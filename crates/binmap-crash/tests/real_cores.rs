//! Crash analysis against real core dumps (Phase 2).
//!
//! Every core here was produced by a real process dying under gdb, and the
//! ground truth is known because `corpus/crasher` was written to die in
//! specific ways at specific lines. Three of the bugs these caught were
//! invisible to any synthetic fixture:
//!
//! - a shared library's load bias computed from its *second* segment, because
//!   a filter discarded the zero vaddr that every library has;
//! - a panic detector that missed a real panic, then one that reported a plain
//!   segfault as a panic;
//! - a return-address check that rejected every return into libc.

use binmap_crash::classify::Crash;
use binmap_crash::modules::Modules;
use binmap_crash::symbolize::{Resolved, Symbolizer};
use binmap_crash::unwind::Method;
use binmap_crash::{CoreDump, bias, classify, correspondence, unwind};
use std::path::{Path, PathBuf};

fn corpus() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus/crasher")
}

/// The cores are committed; the binaries are built. Skip rather than fail when
/// the corpus has not been built, so a contributor without it does not see a
/// red suite for a backend they are not touching.
fn load(core: &str, binary: &str) -> Option<(Vec<u8>, Vec<u8>, PathBuf)> {
    let core_path = corpus().join("cores").join(core);
    let binary_path = corpus().join(binary);
    Some((std::fs::read(core_path).ok()?, std::fs::read(&binary_path).ok()?, binary_path))
}

fn segv() -> Option<(Vec<u8>, Vec<u8>, PathBuf)> {
    load("null_write.core", "target/release/crasher")
}

/// The line a crash site should resolve to, read from the marker beside it.
///
/// Hardcoding a line number here went stale the first time the corpus grew a
/// site above it, and the test then failed for a reason that had nothing to do
/// with the unwinder. The source is the ground truth, so the source is what is
/// read.
fn expected_line(site: &str) -> u32 {
    let source = std::fs::read_to_string(corpus().join("src/main.rs"))
        .expect("the crasher's source is beside its cores");
    source
        .lines()
        .enumerate()
        .find(|(_, line)| line.contains(&format!("// site: {site}")))
        .map(|(index, _)| index as u32 + 1)
        .unwrap_or_else(|| panic!("no `// site: {site}` marker in the crasher"))
}

fn panic_core() -> Option<(Vec<u8>, Vec<u8>, PathBuf)> {
    load("panic.core", "cores/crasher-abort")
}

/// Everything, end to end, the way a caller would.
struct Analysed {
    stack: unwind::Stack,
    frames: Vec<Resolved>,
    crash: Crash,
}

fn analyse(core_data: &[u8], binary: &[u8], binary_path: &Path) -> Analysed {
    let dump = CoreDump::parse(core_data).expect("a real core parses");
    let thread = dump.crashing_thread();

    let name = binary_path.file_name().unwrap().to_string_lossy().to_string();
    let modules = Modules::load(&dump, Some((&name, binary)));
    let stack = unwind::walk(&dump, core_data, &modules, &thread.registers).expect("a stack");

    let frames: Vec<Resolved> = stack
        .frames
        .iter()
        .map(|frame| {
            frame
                .module
                .as_deref()
                .zip(frame.link_time_address)
                .and_then(|(module, address)| {
                    Symbolizer::load(Path::new(module), &[address])
                        .ok()
                        .map(|symbolizer| symbolizer.resolve(address))
                })
                .unwrap_or_default()
        })
        .collect();

    let crash = classify::classify(&dump, thread, &frames);
    Analysed { stack, frames, crash }
}

/// Every function named anywhere in a stack, inline frames included.
fn functions(frames: &[Resolved]) -> Vec<String> {
    frames
        .iter()
        .flat_map(|resolved| resolved.locations.iter())
        .filter_map(|location| location.function.clone())
        .collect()
}

// --- parsing ----------------------------------------------------------------

#[test]
fn a_real_core_reports_its_signal_and_process() {
    let Some((core_data, _, _)) = segv() else { return };
    let dump = CoreDump::parse(&core_data).expect("parses");

    let thread = dump.crashing_thread();
    assert_eq!(thread.signal, classify::signal::SIGSEGV, "this core is a segfault");
    assert!(thread.pid > 0);
    assert!(thread.registers.instruction_pointer().is_some());
    assert!(thread.registers.stack_pointer().is_some());
}

#[test]
fn a_real_core_records_where_its_files_were_mapped() {
    let Some((core_data, _, _)) = segv() else { return };
    let dump = CoreDump::parse(&core_data).unwrap();

    assert!(!dump.mappings.is_empty(), "NT_FILE was read");
    assert!(!dump.segments.is_empty(), "PT_LOAD segments were read");
    assert_eq!(dump.executable.as_deref(), Some("crasher"));
    assert!(
        dump.mappings.iter().any(|mapping| mapping.path.contains("libc")),
        "a dynamically linked program maps libc"
    );

    let executable = dump.executable_mapping().expect("the executable is mapped");
    assert_eq!(executable.file_offset, 0, "the first mapping is at offset zero");
}

#[test]
fn a_binary_is_not_a_core_and_says_so() {
    // Passing the wrong file is the commonest mistake here, and reading a
    // binary as a core would invent threads and registers that do not exist.
    let Some((_, binary, _)) = segv() else { return };
    let error = CoreDump::parse(&binary).expect_err("a binary is not a core");
    assert!(error.to_string().contains("not a core dump"), "{error}");
}

#[test]
fn something_that_is_not_elf_at_all_says_so() {
    let error = CoreDump::parse(b"this is not an ELF file").expect_err("not ELF");
    assert!(error.to_string().contains("not an ELF file"), "{error}");
}

// --- correspondence ---------------------------------------------------------

#[test]
fn the_binary_that_produced_the_core_is_recognised_by_its_build_id() {
    let Some((core_data, binary, path)) = segv() else { return };
    let dump = CoreDump::parse(&core_data).unwrap();

    let verdict =
        correspondence::verify(&dump, &core_data, &binary, &path.display().to_string()).unwrap();
    match &verdict {
        correspondence::Correspondence::Certain { .. } => {}
        other => panic!("expected a build-id match, got {other:?}"),
    }
    assert!(verdict.permits_symbolization());
    assert!(!verdict.needs_a_caveat(), "a proven match needs no hedging");
}

#[test]
fn a_different_binary_is_caught_rather_than_symbolized() {
    // The failure this exists for: a rebuild between the crash and the
    // investigation produces a stack of real-looking symbols at real-looking
    // lines, every one of them wrong.
    let Some((core_data, _, _)) = segv() else { return };
    let Some((_, other_binary, other_path)) = panic_core() else { return };
    let dump = CoreDump::parse(&core_data).unwrap();

    let verdict =
        correspondence::verify(&dump, &core_data, &other_binary, &other_path.display().to_string())
            .unwrap();
    match &verdict {
        correspondence::Correspondence::Mismatched { .. } => {}
        other => panic!("a different build must be caught, got {other:?}"),
    }
    assert!(!verdict.permits_symbolization());
    assert!(verdict.describe().contains("entirely wrong"), "{}", verdict.describe());
}

// --- the load bias ----------------------------------------------------------

#[test]
fn the_load_bias_is_corroborated_by_two_derivations() {
    // Getting this wrong does not produce an error, it produces a symbol, a
    // file and a line, all confidently wrong.
    let Some((core_data, binary, _)) = segv() else { return };
    let dump = CoreDump::parse(&core_data).unwrap();

    let bias = bias::derive(&dump, &binary).expect("a bias");
    assert_eq!(bias.derivation, bias::Derivation::Corroborated);
    assert!(!bias.derivation.needs_a_caveat());
    assert!(bias.bias > 0, "a PIE binary is loaded somewhere above zero");

    // And it round-trips.
    let runtime = dump.crashing_thread().registers.instruction_pointer().unwrap();
    let link_time = bias.to_link_time(runtime).expect("inside the executable");
    assert_eq!(bias.to_runtime(link_time), Some(runtime));
    assert!(link_time < bias.bias, "a link-time address is far below a runtime one");
}

#[test]
fn an_address_below_the_bias_is_refused_rather_than_wrapped() {
    // A wrapped subtraction produces an enormous address that looks like a
    // plausible lookup failure rather than a category error.
    let Some((core_data, binary, _)) = segv() else { return };
    let dump = CoreDump::parse(&core_data).unwrap();
    let bias = bias::derive(&dump, &binary).unwrap();

    assert_eq!(bias.to_link_time(0x1000), None, "below the bias is not in the executable");
}

// --- unwinding --------------------------------------------------------------

#[test]
fn the_faulting_frame_is_the_line_that_faulted() {
    // Ground truth is the `// site:` marker in the crasher's own source, which
    // is also what gdb reports for this core.
    let Some((core_data, binary, path)) = segv() else { return };
    let analysed = analyse(&core_data, &binary, &path);

    let innermost = analysed.frames.first().expect("a frame");
    let physical = innermost.physical().expect("a resolved location");
    assert_eq!(physical.function.as_deref(), Some("crasher::null_write"), "{physical:?}");
    assert_eq!(physical.line, Some(expected_line("null_write")));
    assert!(physical.file.as_deref().is_some_and(|file| file.ends_with("main.rs")));
}

#[test]
fn an_inlined_frame_is_recovered_and_marked_as_inlined() {
    // `write_volatile` is `#[inline(always)]`, so it has no machine-level
    // frame. An unwinder that showed only physical frames would lose it, and
    // one that showed it as physical would misrepresent the stack.
    let Some((core_data, binary, path)) = segv() else { return };
    let analysed = analyse(&core_data, &binary, &path);

    let innermost = analysed.frames.first().unwrap();
    assert!(innermost.inline_depth() >= 1, "{innermost:?}");
    assert!(
        innermost
            .locations
            .iter()
            .any(|l| l.function.as_deref().is_some_and(|f| f.contains("write_volatile"))),
        "{innermost:?}"
    );
    assert!(!innermost.physical().unwrap().inlined, "the last location is the physical one");
}

#[test]
fn the_innermost_frame_comes_from_the_registers_and_the_rest_from_cfi() {
    // Frame pointers are absent in optimised code, so CFI is the only thing
    // that can walk this stack at all.
    let Some((core_data, binary, path)) = segv() else { return };
    let analysed = analyse(&core_data, &binary, &path);

    assert_eq!(analysed.stack.frames[0].method, Method::Registers);
    assert!(analysed.stack.frames.len() > 1, "the walk got past the faulting frame");
    assert!(
        analysed.stack.frames[1..].iter().all(|frame| frame.method == Method::Cfi),
        "every later frame came from CFI"
    );
    assert!(!analysed.stack.contains_guesses(), "nothing here was guessed");
}

#[test]
fn the_walk_reaches_main() {
    let Some((core_data, binary, path)) = segv() else { return };
    let analysed = analyse(&core_data, &binary, &path);

    let named = functions(&analysed.frames);
    assert!(named.iter().any(|f| f == "crasher::main"), "got {named:?}");
}

#[test]
fn a_panic_unwinds_through_libc_into_the_users_own_code() {
    // The test that forced per-module CFI. A panicking process aborts through
    // libc, so the innermost frames are libc's — and an unwinder knowing only
    // the executable stops at frame zero, leaving the whole Rust stack
    // unreachable.
    let Some((core_data, binary, path)) = panic_core() else { return };
    let analysed = analyse(&core_data, &binary, &path);

    let named = functions(&analysed.frames);
    assert!(named.iter().any(|f| f.contains("abort")), "through libc: {named:?}");
    assert!(
        named.iter().any(|f| f == "crasher::explicit_panic"),
        "and into the user's code: {named:?}"
    );
    assert!(analysed.stack.frames.len() > 10, "a full stack, not a truncated one");
}

#[test]
fn recursion_appears_once_per_call() {
    // `explicit_panic(3)` calls itself at depths 3, 2, 1 and 0, so the frame
    // appears *four* times — the initial call plus three recursions. The
    // first version of this asserted three and passed against a different
    // build, which is the kind of off-by-one that a stack trace makes look
    // completely reasonable. An unwinder that lost the repetition, or
    // invented extra copies, would be wrong in exactly the same plausible way.
    //
    // This one rather than the `deep_recursion` site, which LLVM turns into a
    // loop whatever shape it is written in. `explicit_panic` returns `!`,
    // which defeats the tail-recursion pass and leaves real frames on the
    // stack.
    let Some((core_data, binary, path)) = panic_core() else { return };
    let analysed = analyse(&core_data, &binary, &path);

    let recursions =
        functions(&analysed.frames).iter().filter(|f| *f == "crasher::explicit_panic").count();
    assert_eq!(recursions, 4, "the initial call at depth 3, then 2, 1 and 0");
}

#[test]
fn the_walk_says_why_it_stopped() {
    // Every stop reason is a real answer about the stack rather than an error.
    let Some((core_data, binary, path)) = segv() else { return };
    let analysed = analyse(&core_data, &binary, &path);

    let reason = &analysed.stack.stopped_because;
    assert!(!reason.describe().is_empty());
    // Whatever it was, the summary tells the reader.
    assert!(!analysed.stack.describe().is_empty());
    if !reason.is_complete() {
        assert!(analysed.stack.describe().contains("incomplete"));
    }
}

#[test]
fn every_frame_knows_which_module_it_is_in() {
    // A link-time address means nothing without it: each module has its own
    // bias, so the same number refers to different code in different files.
    let Some((core_data, binary, path)) = panic_core() else { return };
    let analysed = analyse(&core_data, &binary, &path);

    for frame in &analysed.stack.frames {
        if frame.link_time_address.is_some() {
            assert!(frame.module.is_some(), "{frame:?} has an address but no module");
        }
    }
    let modules: std::collections::BTreeSet<&str> =
        analysed.stack.frames.iter().filter_map(|f| f.module.as_deref()).collect();
    assert!(modules.len() >= 2, "this stack crosses libc and the executable: {modules:?}");
}

// --- classification ---------------------------------------------------------

#[test]
fn a_segfault_is_not_reported_as_a_panic() {
    // The false positive a broadened substring match produced:
    // `std::panicking::catch_unwind` is on the stack of *every* Rust program,
    // installed by `lang_start` before `main` runs.
    let Some((core_data, binary, path)) = segv() else { return };
    let analysed = analyse(&core_data, &binary, &path);

    assert!(!analysed.crash.is_a_panic(), "got {:?}", analysed.crash);
    assert!(
        matches!(analysed.crash, Crash::InvalidMemoryAccess { .. } | Crash::NullDereference),
        "got {:?}",
        analysed.crash
    );
}

#[test]
fn a_panic_is_reported_as_a_panic_rather_than_as_signal_six() {
    // "Terminated by signal 6" tells a reader nothing they did not know.
    let Some((core_data, binary, path)) = panic_core() else { return };
    let analysed = analyse(&core_data, &binary, &path);

    assert!(analysed.crash.is_a_panic(), "got {:?}", analysed.crash);
    assert!(analysed.crash.title().contains("panicked"));
}

#[test]
fn every_classification_says_what_to_look_at() {
    // The first question this kind of crash asks is what a reader actually
    // needs, and it differs per kind.
    for crash in [
        Crash::Panic { message: None },
        Crash::NullDereference,
        Crash::StackOverflow,
        Crash::InvalidMemoryAccess { address: Some(0x40) },
        Crash::ArithmeticFault,
        Crash::IllegalInstruction,
        Crash::BusError,
        Crash::Unclassified { signal: 9 },
    ] {
        assert!(!crash.title().is_empty(), "{crash:?} has no title");
        assert!(!crash.what_to_look_at().is_empty(), "{crash:?} offers no next step");
    }
}

// --- modules ----------------------------------------------------------------

#[test]
fn the_modules_a_process_mapped_are_loaded_with_their_own_biases() {
    let Some((core_data, binary, path)) = segv() else { return };
    let dump = CoreDump::parse(&core_data).unwrap();
    let name = path.file_name().unwrap().to_string_lossy().to_string();
    let modules = Modules::load(&dump, Some((&name, &binary)));

    assert!(modules.loaded.len() >= 2, "at least the executable and libc");
    for module in &modules.loaded {
        assert!(!module.eh_frame.is_empty(), "{} has no CFI", module.path);
        assert!(module.base > 0);
        assert!(module.end > module.base);
    }

    // Each module's bias is its own. Sharing one would place every library
    // frame in the wrong function.
    let biases: std::collections::BTreeSet<u64> =
        modules.loaded.iter().map(|module| module.bias).collect();
    assert!(biases.len() > 1, "different modules are loaded at different places");
}

#[test]
fn an_address_is_attributed_to_the_module_that_owns_it() {
    let Some((core_data, binary, path)) = segv() else { return };
    let dump = CoreDump::parse(&core_data).unwrap();
    let name = path.file_name().unwrap().to_string_lossy().to_string();
    let modules = Modules::load(&dump, Some((&name, &binary)));

    let rip = dump.crashing_thread().registers.instruction_pointer().unwrap();
    let owner = modules.containing(rip).expect("the faulting address is in a module");
    assert!(owner.path.contains("crasher"), "the fault is in our own code: {}", owner.path);

    // And an address in nothing is in nothing.
    assert!(modules.containing(0x10).is_none());
}
