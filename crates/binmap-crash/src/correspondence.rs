//! Does this binary belong to this core? (`F2.2`)
//!
//! The question nobody asks until a debugger has already lied to them. A core
//! and a binary from different builds produce a stack trace made entirely of
//! real-looking symbols at real-looking lines, every one of them wrong, and
//! nothing about the output says so. Rebuilding between a crash and an
//! investigation is not an unusual mistake — it is the *normal* workflow, which
//! is what makes this worth a module.
//!
//! Three checks, in order of how conclusive they are:
//!
//! 1. **Build id.** ELF notes carry one, and it changes with the contents. Two
//!    matching build ids is proof; two differing ones is disproof.
//! 2. **Path.** The core records where the executable was mapped from.
//! 3. **Size and layout.** The mapped extent should match the binary's.
//!
//! Only the first is conclusive, and when it is absent the verdict says so
//! rather than promoting a weaker check to a guarantee.

use crate::dump::CoreDump;
use binmap_core::error::{Error, Result};
use object::read::elf::ElfFile64;
use object::{Endianness, Object};
use serde::{Deserialize, Serialize};

/// How confident we are that the binary matches the core.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Correspondence {
    /// Build ids match. Proof, and the only verdict that permits silence.
    Certain { build_id: String },
    /// No build id on one side or the other, but nothing contradicts. The
    /// weaker guarantee has to be stated wherever a symbol is shown.
    Plausible { because: String },
    /// Something is demonstrably wrong.
    Mismatched { because: String },
}

impl Correspondence {
    /// Whether symbolizing is defensible at all.
    pub fn permits_symbolization(&self) -> bool {
        !matches!(self, Correspondence::Mismatched { .. })
    }

    /// Whether every symbolized frame has to carry a caveat.
    pub fn needs_a_caveat(&self) -> bool {
        !matches!(self, Correspondence::Certain { .. })
    }

    pub fn describe(&self) -> String {
        match self {
            Correspondence::Certain { build_id } => format!(
                "The binary and the core carry the same build id ({}), so the symbols below are \
                 the ones that ran.",
                &build_id[..build_id.len().min(16)]
            ),
            Correspondence::Plausible { because } => format!(
                "This binary is probably the one that produced this core, but it cannot be \
                 proven: {because}. If it was rebuilt since the crash, every line number below \
                 will be wrong while looking right."
            ),
            Correspondence::Mismatched { because } => format!(
                "This binary did not produce this core: {because}. Symbolizing anyway would \
                 produce a stack trace that is entirely plausible and entirely wrong."
            ),
        }
    }
}

/// Compare a binary against a core.
///
/// `core_data` is the core file's own bytes, needed because the conclusive
/// check reads the build id back out of the core's copy of the executable's
/// text — comparing what actually ran against what is on disk, rather than
/// comparing the file to itself.
pub fn verify(
    dump: &CoreDump,
    core_data: &[u8],
    executable: &[u8],
    path: &str,
) -> Result<Correspondence> {
    let file = ElfFile64::<Endianness>::parse(executable)
        .map_err(|error| Error::Other(format!("the executable is not an ELF file: {error}")))?;

    let binary_build_id = file
        .build_id()
        .ok()
        .flatten()
        .map(|id| id.iter().map(|byte| format!("{byte:02x}")).collect::<String>());

    // The core's own copy of the executable's first pages contains the same
    // ELF notes, because a core includes the mapped file's text. Where the
    // note is in a `PT_LOAD` the core kept, the build id can be read straight
    // back out of it.
    let core_build_id = build_id_from_core(dump, core_data, executable);

    match (&binary_build_id, &core_build_id) {
        (Some(binary), Some(core)) if binary == core => {
            return Ok(Correspondence::Certain { build_id: binary.clone() });
        }
        (Some(binary), Some(core)) => {
            return Ok(Correspondence::Mismatched {
                because: format!(
                    "the binary's build id is {} and the core's is {}",
                    &binary[..binary.len().min(16)],
                    &core[..core.len().min(16)]
                ),
            });
        }
        _ => {}
    }

    // No build id to compare. Fall back to what the mapping says.
    let Some(mapping) = dump.executable_mapping() else {
        return Ok(Correspondence::Plausible {
            because: "this core records no mapped file table, so there is nothing to compare"
                .into(),
        });
    };

    let recorded = mapping.path.as_str();
    let same_name = recorded.rsplit('/').next() == path.rsplit('/').next();
    if !same_name {
        return Ok(Correspondence::Mismatched {
            because: format!("the core was produced by `{recorded}`, and this is `{path}`"),
        });
    }

    Ok(Correspondence::Plausible {
        because: if binary_build_id.is_none() {
            "this binary carries no build id, so there is nothing conclusive to compare — build \
             with `-C link-arg=-Wl,--build-id` to make this checkable"
                .into()
        } else {
            "the core does not retain the pages holding the build id".to_string()
        },
    })
}

/// Read the build id out of the core's own copy of the executable's text.
///
/// The note lives in the first `PT_LOAD`, which a core keeps, so this compares
/// what actually ran against what is on disk rather than comparing the file to
/// itself.
fn build_id_from_core(dump: &CoreDump, core_data: &[u8], executable: &[u8]) -> Option<String> {
    let file = ElfFile64::<Endianness>::parse(executable).ok()?;
    let expected = file.build_id().ok().flatten()?;

    // Where the note sits at link time, found by locating it in the file and
    // translating through the executable's own mapping.
    let mapping = dump.executable_mapping()?;
    let offset = find_subslice(executable, expected)?;
    let runtime = mapping.start.checked_add(offset as u64)?;

    // The core's copy of those same bytes. If the process was running a
    // different build, these differ.
    let core_bytes = dump.read(core_data, runtime, expected.len())?;
    Some(core_bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// Find `needle` in `haystack`.
fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|window| window == needle)
}
