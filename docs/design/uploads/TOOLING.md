# Lodestone — Toolchain Study

> Deep dive on every external tool and crate the project depends on: what it does, what it
> costs, what breaks, and what to use instead. Companion to `DESIGN.md` §3.3.

---

## 1. Link in, or shell out?

Two integration styles, with a rule for choosing.

**Link in (a Rust crate)** when the output is structured data you will query repeatedly and
interactively — symbol tables, DWARF, disassembly. Parsing is in-process, typed, and fast
enough for a UI.

**Shell out (an external binary)** when the tool represents years of specialised work, its
output is a report rather than a queryable structure, and it runs once per analysis —
`bloaty`, `hyperfine`, `perf`, `rr`.

**The trap:** shelling out to something you query in a loop. Calling `addr2line` as a
subprocess once per stack frame is fine; calling it once per sample across a million perf
samples is not. Symbolization must be in-process, which is why `addr2line` appears as a crate
below and not as a binary.

Every shelled-out tool gets: a version check at startup, a wrapper module that is the only
place its output is parsed, and a snapshot test against captured real output.

---

## 2. Binary analysis layer

### 2.1 `object` — container parsing

The foundation. Reads ELF (and Mach-O, PE, and wasm, which matters only for future
portability). Gives sections, segments, symbols, and relocations behind a unified API, plus a
lower-level `object::elf` module when you need raw structures.

You need it for:
- Section sizes and flags — the basis of the `.text` / `.rodata` / `.eh_frame` breakdown
- The symbol table (`.symtab`, or `.dynsym` when stripped)
- Program headers, which is how you read core dumps (§2.6)
- Build ID, for matching a binary to its debug info

**Watch for:** symbol sizes in ELF are frequently zero or wrong for compiler-generated
symbols. Deriving size from the next symbol's address is a common workaround and is itself
frequently wrong across section boundaries and with alignment padding. `bloaty` handles this
better than a naive implementation, which is the cross-check argument in §4.1.

### 2.2 `gimli` — DWARF

The whole of Phase 2 rests here. It is a low-level library: it will not hand you "the value of
`x` at line 42." It hands you the pieces from which that is derived.

**Sections you will actually touch:**

| Section | What it holds | Used for |
|---|---|---|
| `.debug_info` | The DIE tree: types, variables, functions, scopes | Everything |
| `.debug_abbrev` | DIE shape declarations | Decoding `.debug_info` |
| `.debug_line` | The line-number program | Address ↔ source line |
| `.debug_str`, `.debug_line_str` | String tables | Names |
| `.debug_loclists` | Location lists — where a variable lives, per PC range | **Recovering "optimized out" values** |
| `.debug_rnglists` | Address ranges for scopes | Scope resolution |
| `.debug_addr` | Address pool (split DWARF) | Indirect addresses |
| `.eh_frame` / `.debug_frame` | Call frame information | Unwinding |

**The three hard parts, in the order you will hit them:**

1. **The line program is a state machine, not a table.** You execute it to produce rows. One
   address maps to multiple lines and one line maps to multiple non-contiguous address ranges
   after optimization. This is not an error — it is the fact the product exists to surface.

2. **Location lists are expressions, not addresses.** A variable's location varies by PC and
   is expressed as a DWARF expression that must be *evaluated*: "in register `rbx`", "at
   `CFA-24`", "in `rax`'s low 32 bits composed with a stack piece". `gimli::Evaluation` runs
   these. Piecewise locations (`DW_OP_piece`) are common at `-O2` and are where naive
   implementations quietly produce wrong values. Wrong values are worse than absent ones.

3. **Inlining is a tree.** `DW_TAG_inlined_subroutine` entries carry `DW_AT_call_file` and
   `DW_AT_call_line`, letting you reconstruct the logical call chain that the physical stack
   destroyed. `addr2line` does this for you (§2.3) — use it rather than reimplementing.

**Split debug info.** `-Csplit-debuginfo` produces `.dwo` or `.dwp` files, and this is
increasingly common. Handle it from the start rather than bolting it on: detect
`DW_AT_GNU_dwo_name` / DWARF 5 split units and locate the companion file. Discovering this
mid-Phase-2 is a bad week.

**Learning path.** Read `gimli`'s `examples/simple.rs`, then `examples/dwarfdump.rs`. The
latter is a complete DWARF dumper and is the best available tutorial for the crate.

### 2.3 `addr2line` — the API that matters most

Built on `gimli`. Its `Context::find_frames(addr)` returns an iterator over frames at an
address, **including the inlined ones**, each with function name, file, and line.

This single call is the backbone of Phase 2 and Phase 3. It is also the thing you must not
reimplement — inline frame reconstruction is subtle and this crate is used by Rust's own
backtrace machinery.

Build the `Context` once per binary and reuse it. Construction parses and indexes DWARF and
is expensive; lookups are cheap. Cache it keyed by build ID.

### 2.4 `rustc-demangle` — symbol names

Handles both the legacy hash-suffixed scheme and the v0 scheme.

**v0 is the important one**, because it is structured rather than lossy. It encodes the crate,
the path, and crucially the *generic arguments* and disambiguators. That structure is what
makes monomorphization grouping possible: two instantiations of the same generic share a path
prefix and differ in their type arguments, which is exactly the grouping key Phase 1 needs.

Legacy symbols do not carry this. If a crate is built with legacy mangling, Phase 1's grouping
degrades to heuristic prefix matching. Detect the scheme and say so in the UI rather than
silently producing worse results.

Enable v0 explicitly: `-Csymbol-mangling-version=v0`.

### 2.5 Disassembly — `iced-x86`

**Recommended, and the reason is specific.**

| Option | Assessment |
|---|---|
| **`iced-x86`** | Pure Rust, x86/x64 only, very fast. Decoder plus formatters for Intel, AT&T, masm, and nasm syntax. **Critically: `InstructionInfoFactory` reports the registers and memory each instruction reads and writes.** |
| `capstone` | Bindings to a mature C library, many architectures. Adds a C dependency; instruction-info detail is weaker. |
| `yaxpeax-x86` | Pure Rust, precise semantics, good design. Smaller ecosystem, less formatter flexibility. |

The `InstructionInfoFactory` point decides it. `DESIGN.md` §5.4 requires slicing by data
dependency — "which instructions could have written this register?" — and that requires
per-instruction read/write sets. Getting them from a decoder rather than hand-maintaining
tables is the difference between a weekend and a quarter.

The multi-architecture argument for `capstone` does not apply: `rr` is x86-64 only, so
Phase 4 pins the architecture regardless.

### 2.6 Core dumps

There is no good crate for Linux ELF core dumps. The `minidump` crate handles Breakpad's
format, which is not what the kernel writes. You will build this on `object`.

Structure: an ELF file of type `ET_CORE`, whose `PT_LOAD` segments carry memory contents and
whose `PT_NOTE` segment carries the metadata. The notes you need:

| Note | Contents |
|---|---|
| `NT_PRSTATUS` | General-purpose registers, per thread. **This is the register file.** |
| `NT_PRPSINFO` | Process info, command line |
| `NT_FILE` | Mapped files with addresses — how you locate the binary and shared objects |
| `NT_X86_XSTATE` | Extended registers (SSE/AVX), if you ever need them |
| `NT_AUXV` | Auxiliary vector, including the load bias for PIE binaries |

**The PIE trap.** Rust binaries are position-independent by default. Every address in the core
is a runtime address; every address in DWARF is a link-time address. You must compute the load
bias from `NT_FILE` or `NT_AUXV` and subtract it before any DWARF lookup. Getting this wrong
produces plausible-looking but entirely wrong symbolization, which is the worst failure mode
available. Assert on it: symbolize a known address and verify against `addr2line` on the
command line during development.

**Operational notes for the docs:** core dumps are often disabled (`ulimit -c 0`), routed
through `core_pattern` to a handler, or captured by `systemd-coredump` and retrievable with
`coredumpctl`. The tool should detect and explain these rather than reporting "no core found."

### 2.7 Unwinding

Two paths:

- **`gimli`'s CFI support** — `UnwindContext` and `UnwindTable` evaluate `.eh_frame` rules
  directly. Full control, more work.
- **`framehop`** — a crate purpose-built for unwinding, combining CFI, frame pointers, and
  prologue analysis, used by `samply`. Handles the cases where `.eh_frame` is absent or
  wrong.

**Recommendation: `framehop`.** Unwinding correctness across optimized frames, signal frames,
and JIT-less-but-hand-written assembly is a deep pit, and this crate has already fallen into
it. Reserve direct `gimli` CFI work for cases `framehop` cannot express.

---

## 3. Build orchestration

### 3.1 `cargo_metadata`

The workspace model: packages, targets, features, dependency graph, target directory. This is
how `lode` discovers what it is looking at.

### 3.2 Sweeping profiles without touching files

**This is the most useful practical detail in the document.** A config sweep must not mutate
the user's `Cargo.toml` — that is destructive, racy, and terrifying to a user watching their
repo change.

Cargo reads profile settings from environment variables:

```
CARGO_PROFILE_RELEASE_OPT_LEVEL=z
CARGO_PROFILE_RELEASE_LTO=fat
CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1
CARGO_PROFILE_RELEASE_PANIC=abort
CARGO_PROFILE_RELEASE_STRIP=symbols
CARGO_PROFILE_RELEASE_DEBUG=line-tables-only
```

The pattern is `CARGO_PROFILE_<PROFILE>_<SETTING>`, uppercased with hyphens as underscores.
There is also `cargo --config profile.release.lto='"fat"'` for the same effect via CLI.

Consequences: a sweep is a loop over environment maps, the working tree is never modified,
runs are trivially parallelisable across distinct target directories, and cancellation leaves
nothing to clean up. Use a separate `CARGO_TARGET_DIR` per configuration to avoid rebuild
thrashing, at the cost of disk — offer a flag to trade one for the other.

**Verify the exact variable names against your toolchain version** before building the sweep
matrix; this is cheap to check and the naming has edge cases (`opt-level` → `OPT_LEVEL`).

### 3.3 Nightly-only axes

Several of the highest-value size levers need nightly:

- `-Z build-std=std,panic_abort` with `-Z build-std-features=panic_immediate_abort` — removes
  the panic formatting machinery entirely. Dramatic on small binaries. Requires the `rust-src`
  component.
- `-Z sanitizer=address|memory|thread|leak`
- Miri

Detect the toolchain and mark nightly-only axes clearly in the UI rather than failing
mysteriously on stable. A stable-only sweep is still valuable.

### 3.4 `-Csymbol-mangling-version=v0`

Not a size lever, but Phase 1 depends on it (§2.4). If the project is not already using v0,
the tool should offer to enable it for analysis builds and explain why.

---

## 4. Measurement

### 4.1 Size

| Tool | Role |
|---|---|
| **Own implementation** (`object` + `rustc-demangle`) | Primary. Attribution requires generic grouping, which no existing tool does. |
| **`bloaty`** | Cross-check and validation. Mature symbol/section attribution with a built-in diff mode. |
| **`cargo-bloat`** | Reference for expected output; largely superseded by your own attribution |
| **`cargo-llvm-lines`** | Counts LLVM IR lines per generic. A *pre-build* proxy for monomorphization cost — useful for ranking before spending build time. |
| **`twiggy`** | wasm-specific. Needed for the WASM corpus crate. |

**Why own it rather than parse `bloaty`:** the product's differentiator is attributing bytes to
a *generic origin* across instantiations, and to categories like panic machinery, `Drop` glue,
vtables, and formatting. That requires v0 demangling and Rust-specific classification. No
general-purpose tool does it, and parsing `bloaty` text to then re-derive structure is worse
than reading the symbol table directly.

**Why keep `bloaty` anyway:** it is a correctness oracle. If your total attributed bytes
disagree with `bloaty`'s by more than a small margin, your attribution is wrong. Make that a
test.

### 4.2 Timing

**`hyperfine`** for command-level benchmarks. Supports warmup runs, minimum run counts, shell
spawn correction, and JSON export. Parse the JSON, never the human-readable table.

**Criterion** for in-repo microbenchmarks. Writes structured estimates including confidence
intervals into `target/criterion`. When a project already has Criterion benches, use them
rather than imposing a new harness.

**The statistical rule, restated because it decides the product's credibility:** never claim
an improvement from a single run. Require non-overlapping confidence intervals or an explicit
significance test. Measure the machine's noise floor once, store it, and refuse to report
improvements beneath it. An optimizer that reports noise as wins is worse than no optimizer,
because it costs the user trust *and* time.

### 4.3 Profiling

**`perf`** is the default source. Two persistent problems:

1. **Permissions.** `kernel.perf_event_paranoid` typically must be ≤ 1, and often ≤ -1 for
   full functionality. Detect the current value and print the exact command to change it
   rather than failing opaquely.

2. **Frame pointers.** Historically Rust omitted frame pointers, making `perf`'s default
   call-graph unwinding useless and forcing `--call-graph dwarf` (large captures, slower) or
   `-Cforce-frame-pointers=yes` (a small runtime cost, much better captures). Recent Rust
   versions changed the default for `std` on some targets. **Verify the behaviour for the
   user's actual toolchain and target rather than assuming**, and surface the answer, since it
   changes the recommended capture command.

**`samply`** is the better ecosystem bet. It is Rust, produces the Firefox Profiler format,
and its unwinding uses `framehop` — the same crate recommended in §2.7. Its supporting crates
(`fxprof-processed-profile`, and the symbolication stack around it) are libraries you can
depend on rather than text you must parse. Prefer this path and keep `perf` as an alternative
input format.

**Avoid `valgrind`/`callgrind` for the default path.** Excellent instruction-count precision,
but the slowdown makes it unusable for interactive work. Offer it as an opt-in mode for
noise-free instruction counting, where it genuinely shines.

---

## 5. Debugger control — a correction

An earlier note in this project's history suggested driving LLDB through its Python API. **For
a Rust application that is the wrong choice**, and the recommendation is withdrawn.

| Approach | Assessment |
|---|---|
| **DAP (Debug Adapter Protocol)** | **Recommended.** JSON over stdio, well-specified, stable. `lldb-dap` and CodeLLDB both speak it. It is designed exactly for a GUI driving a debugger, which is what Lodestone is. |
| **GDB/MI** | Solid fallback, well-documented, parseable. Necessary for `rr`, which speaks GDB's protocol. Uglier than DAP but battle-tested. |
| LLDB Python API | Rich, but embedding Python in a Rust application to get at it is a poor trade. Rust bindings to the C++ API are thin and unmaintained. |
| `ptrace` directly | You would be writing a debugger. `DESIGN.md` N4 says do not. |

**For `rr` specifically:** `rr replay -s <port>` exposes a GDB remote stub, so the practical
control path is GDB/MI, or the remote serial protocol directly. Reverse execution is exposed
through GDB's `reverse-continue`, `reverse-step`, and `reverse-finish`, plus hardware
watchpoints. The agent tools in `DESIGN.md` §5.2 map onto these almost one-to-one.

**`rr` operational constraints, worth knowing before Phase 4 planning:** it requires specific
CPU features and performance counter access, is Linux x86-64 only, needs
`kernel.perf_event_paranoid` ≤ 1, and has known incompatibilities in some virtualized
environments. Recording slows execution meaningfully. None of this is disqualifying, but it
should appear in the docs before a user's first failed recording rather than after.

---

## 6. Verification tooling

| Tool | Role | Constraint |
|---|---|---|
| `cargo test` | Baseline gate | User-overridable command |
| **Miri** | UB detection in safe-reachable Rust | Nightly. Slow. Cannot cross FFI — which is exactly where the interesting `unsafe` often lives. |
| ASan / UBSan / TSan | UB and race detection including FFI | Nightly `-Zsanitizer`. Cannot combine several at once. |
| `cargo-deny` | License and advisory checks, **and dependency bans** | The bans feature enforces `DESIGN-GUI.md` §4's architectural boundary. |
| `cargo-mutants` | Generates the eval corpus's injected bugs | Slow on large crates; scope it to target modules |

The Miri/FFI gap deserves stating in the UI. "Miri passed" on a crate with substantial FFI is
a much weaker statement than it sounds, and a tool that implies otherwise is misleading its
user about safety.

---

## 7. Explicitly not used in v1

Recorded so they are decided rather than drifted into.

| Tool | Why not | Revisit |
|---|---|---|
| Ghidra / Binary Ninja | Heavy dependency; decompiled pseudo-code is only needed if annotated disassembly proves insufficient for the model | Phase 4, if evals show a gap |
| `angr`, KLEE | Symbolic execution is a research-grade time sink and does not serve the v1 questions | Not planned |
| Frida, DynamoRIO, Pin | Dynamic instrumentation overlaps with what `rr` gives more cheaply | If a non-`rr` platform is targeted |
| `valgrind` | Too slow for the default path | Opt-in instruction counting |
| AFL++, `cargo-fuzz` | Reproduction search is a Phase 4+ concern | Heisenbug reproduction feature |
| `minidump` crate | Wrong format for Linux kernel cores | If Windows support is added |

---

## 8. Fragility assessment

Where the project is most likely to break, and what to do about it.

| Dependency | Fragility | Why | Mitigation |
|---|---|---|---|
| **GPUI + `gpui-component`** | **High** | Pre-1.0, frequent breaking changes, forked ecosystem — and now the only product surface | Pin exact; isolate in `lode-gui`; scheduled upgrades (`DESIGN-GUI.md` §7); engine stays independently testable |
| **ACP + agent adapters** | Medium | Young protocol, versioned; adapters are npm packages the user installs | Pin and negotiate at `initialize`; Mode A works standalone so ACP breakage degrades rather than blocks |
| **`rr`** | **High** | Environment-sensitive; hard failures on some hardware and VMs | Phase 4 only; preflight check with clear diagnostics |
| DWARF handling (your code) | High | Optimized-build edge cases are endless | Golden binaries; cross-check against `addr2line` and `readelf` |
| `perf` output parsing | Medium | Format varies by kernel and version | Prefer `samply`'s library path; snapshot tests |
| `bloaty` | Medium | External C++ binary; its Rust demangling may lag | Cross-check role only, never load-bearing |
| Cargo profile env vars | Medium | Naming has edge cases; behaviour can shift | Integration test asserting each axis actually changes the output |
| `gimli`, `object`, `addr2line` | **Low** | Mature, coordinated, widely depended upon | Normal version ranges |
| `iced-x86` | Low | Stable, self-contained | Normal |
| `hyperfine` | Low | Stable JSON output | Snapshot test |

The pattern worth noticing: **the two highest-risk dependencies are both optional to the
*engine*.** GPUI is the presentation layer only — the engine is a library, fully exercised
headlessly by `lode-eval`, so a bad GPUI upgrade cannot stall analysis work. `rr` is Phase 4
only; Phases 0-3 never touch it.

The caveat, added since the GUI became the sole product surface: GPUI is no longer optional to
the *product*, only to the engine. A broken GPUI upgrade now blocks a release even though it
does not block development. That is not an accident of the plan — it is why the phase ordering is
shaped the way it is, and it is worth preserving under pressure to reorder.

---

## 9. Preflight

The Environment panel (PRD U10) checks the environment on first run and reports clearly. A
devtool that fails with a subprocess error message is a devtool people uninstall. The same
checks are available headlessly via `lode-eval doctor` for CI.

Rendered in the panel as a grouped, always-visible status list:

```
Environment

Toolchain
  ✓ rustc 1.xx.x (stable)
  ✓ cargo
  ⚠ nightly not installed — build-std and Miri axes unavailable
  ✓ rust-src component

Measurement
  ✓ bloaty 1.x            /usr/bin/bloaty
  ✓ hyperfine 1.x         /usr/bin/hyperfine
  ⚠ perf not found        install linux-tools-$(uname -r)

Replay (Phase 4)
  ✗ rr not found
  ⚠ kernel.perf_event_paranoid = 2 (rr and perf need ≤ 1)
      sudo sysctl kernel.perf_event_paranoid=1

Project
  ✓ workspace: 4 crates
  ⚠ symbol-mangling-version is legacy — v0 improves generic attribution
  ✓ core dumps enabled (ulimit -c unlimited)

Reasoner
  ✓ local runner reachable at 127.0.0.1:11434
  ○ no API keys configured (not required)
  ✓ claude-code-acp found          npx @zed-industries/claude-code-acp
  ⚠ codex-acp not installed        install from the ACP Registry
  ✓ project policy allows cloud models
```

Every warning names the exact fix, and every fix that can be applied in-app has a button next
to it. Every check maps to a feature that will otherwise fail later, at a worse moment.

---

## 10. Reading list

Ordered by when it becomes relevant, not by importance.

**Phase 0**
- `min-sized-rust` (johnthagen) — effectively the specification for the Phase 0 sweep matrix
- The Cargo Book's profile chapter — the authoritative list of axes
- *The Rust Performance Book* (Nicholas Nethercote)

**Phase 1**
- The Rust symbol mangling v0 RFC — the grammar you are grouping by
- `cargo-llvm-lines` source — a compact worked example of monomorphization accounting

**Phase 2**
- Eli Bendersky's DWARF series — the clearest introduction available
- TartanLlama, "Writing a Linux Debugger" — C++, but the concepts transfer directly
- `gimli`'s `examples/dwarfdump.rs` — the best documentation the crate has
- The DWARF 5 standard — as a reference to consult, not to read through

**Phase 3**
- Brendan Gregg on `perf` and flamegraphs
- `samply` and `framehop` sources

**Phase 4**
- The `rr` wiki, especially the pages on how recording works and its hardware requirements
- The `rr` technical paper, for the determinism model
- Pernosco's writing on omniscient debugging — the closest thing to a design precedent
