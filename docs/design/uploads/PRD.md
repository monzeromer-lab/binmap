# Lodestone — Product Requirements Document

> **Name is a placeholder.** `lodestone` (CLI binary: `lode`) is used throughout so the
> documents read concretely. Substitute freely; see Appendix C for naming criteria.

| Field | Value |
|---|---|
| Status | Draft v0.1 |
| Owner | Solo maintainer |
| Target platform (v1) | Linux x86-64 |
| Target language (v1) | Rust |
| License | Apache-2.0 OR MIT (Rust ecosystem norm) |
| Delivery | Open source, community project |

---

## 1. Summary

Lodestone is a desktop application that understands a Rust program at both the source
level and the machine level at the same time, and uses that dual view to answer three
questions developers currently answer by hand:

1. **Why is this wrong?** — root cause analysis on crashes and misbehaviour in optimized builds.
2. **Why is this slow?** — performance attribution from machine events back to source constructs.
3. **Why is this so big?** — binary size attribution, and what to change to shrink it.

It is a deterministic analysis engine with an optional language-model layer on top. The
engine measures, symbolizes, and verifies. The model explains, hypothesizes, and proposes.
Every claim the model makes is displayed alongside the tool output that supports it.

---

## 2. Problem statement

Rust developers hit a wall the moment they leave debug builds.

**The source-binary gap.** In a release build, variables are "optimized out," functions are
inlined into unrecognizable shapes, and stepping through source lies about what actually
executed. The existing answer is to drop into raw disassembly, which most application
developers cannot read fluently and which no tool connects back to their source.

**Size is unattributed.** `cargo-bloat` and `bloaty` will tell you that a symbol costs 40 KB.
Neither can tell you that it costs 40 KB because a generic function was monomorphized into
twelve near-identical copies, nor what to restructure to collapse them. The measurement
tools stop exactly where the useful advice would start.

**Performance data is a wall of samples.** `perf` and flamegraphs show where cycles go.
Turning "this symbol is hot" into "this is hot because the compiler could not vectorize the
loop, because of this aliasing assumption in your source" is expert work that does not scale.

**Configuration is folklore.** The knowledge that `panic = "abort"` deletes unwinding tables,
that `codegen-units = 1` interacts with LTO, that `opt-level = "z"` sometimes produces
*slower and larger* output than `"s"` on a given crate — this lives in blog posts and tribal
memory rather than in a tool that measures it on your actual code.

Underneath all four is one gap: **no tool holds the source and the binary in view
simultaneously and reasons across the boundary.** Debuggers see the binary. Linters see the
source. Profilers see samples. Nothing bridges them.

---

## 3. Why now

- **`gimli`, `object`, and `addr2line` are mature.** Reading DWARF from Rust is now a
  library call rather than a research project. This was not true five years ago.
- **Language models are good at exactly this bridge.** Mapping between two representations
  of the same artifact — decompiled pseudo-code and source, symbol names and type structure,
  perf events and language constructs — is a translation task, which is what these models do
  best. They are unreliable at arithmetic over addresses, which is why the engine, not the
  model, computes facts.
- **Rust's size problem is now economically material.** WASM payloads, embedded targets, and
  container image sizes all make binary bytes a line item.
- **The alternative is expensive and closed.** Pernosco (the commercial rr front-end) proves
  demand for intelligent replay debugging. It is a paid hosted service. There is no open,
  local equivalent.

---

## 4. Users

### Persona A — Embedded / WASM developer ("bytes are budget")
Ships to a device with 256 KB of flash, or a WASM bundle where every kilobyte is latency.
Currently maintains a hand-tuned `Cargo.toml` copied from `min-sized-rust` and periodically
runs `cargo-bloat` and squints.
**Wants:** a number that goes down, with proof nothing broke.
**Primary surface:** Size Explorer, Profile Lab.

### Persona B — Systems developer debugging a release build ("it only crashes in prod")
Has a core dump from a stripped or lightly-stripped release binary, a stack trace with
inlined frames, and no ability to reproduce locally.
**Wants:** the faulting line, the values that led there, and a hypothesis.
**Primary surface:** Crash Analysis.

### Persona C — Crate maintainer under performance pressure ("a regression landed")
Owns a widely-used crate. A benchmark regressed 12% and bisect points at a 400-line refactor.
**Wants:** attribution of the regression to a specific construct, and a verified fix.
**Primary surface:** Flamegraph, Profile Lab diff mode.

### Explicit non-user (v1)
The reverse engineer working on a binary **without** source. Lodestone's entire thesis is the
source-binary bridge; with no source, it degrades to a worse Ghidra. This may change in v2.

---

## 5. Goals and non-goals

### Goals
- G1. Reduce binary size on a real crate by a measurable amount with automatic verification
  that tests still pass. **This is the v1 headline.**
- G2. Attribute size, cycles, and crashes to *source constructs*, not just symbols.
- G3. Remain fully functional with the model layer disabled.
- G4. Never present a model claim without its supporting evidence visible.
- G5. Run entirely on the user's machine by default. No binary or source leaves the host
  unless the user explicitly configures a cloud backend.
- G6. Ship something useful every 6-8 weeks.

### Non-goals (v1)
Recording these matters more than recording the goals; a solo project dies from scope creep.

- **N1. Not an IDE.** No editing, no LSP, no build-on-save. It reads your code; you edit
  elsewhere.
- **N2. Not a source-only linter.** If Clippy can find it, Lodestone should not try.
- **N3. Not cross-platform in v1.** Linux x86-64 only. `rr` is Linux-x86-64-only and that
  constraint propagates.
- **N4. Not a replacement for GDB/LLDB.** No general-purpose interactive stepping UI.
  Lodestone drives debuggers; it is not one.
- **N5. Not a hosted service.** No accounts, no server, no telemetry — not in v1, and any
  future change requires opt-in.
- **N6. Not C/C++ in v1.** The architecture must not *preclude* it (see DESIGN §3.2), but no
  effort is spent supporting it.
- **N7. No autonomous commits to a user's repository** without the trust tier explicitly
  raised, and never as the default.

---

## 6. Product principles

**P1 — Deterministic core, optional intelligence.**
Every feature must produce value with the model turned off. The model adds explanation,
ranking, and proposal. It never adds facts. Consequence: a bad model output degrades the
product; it cannot break it.

**P2 — Evidence or it didn't happen.**
Every finding carries a chain of evidence, each item tagged with its provenance
(deterministic tool output vs. model inference). The UI renders these differently and always
visibly. A user must never have to guess whether they are reading a measurement or a guess.

**P3 — The verifier is the product.**
The ability to *prove* a change is safe and beneficial is worth more than the ability to
propose it. Build the verifier first; proposals are cheap once verification exists.

**P4 — Local by default.**
Binaries are proprietary. Any design that assumes uploading them is dead on arrival in the
target market.

**P5 — Trust is a dial, not a mode.**
The four optimizer tiers (§7) are one mechanism at four settings, not four features. The
tier is always visible and never silently raised.

---

## 7. The trust ladder

The optimizer's autonomy is a single global setting.

| Tier | Name | Behaviour | Gate |
|---|---|---|---|
| 0 | Observe | Measures, ranks, explains. Read-only. | — |
| 1 | Propose | Generates diffs. Never applies them. | Human review |
| 2 | Tune | Modifies build configuration only. Applies and measures. | Tests pass + significant benchmark win |
| 3 | Autonomous | Applies source patches, verifies, opens a PR. | All of Tier 2 + sanitizers clean + explicit opt-in |

**Default is Tier 1.** Tier 3 requires a config file change, not a UI toggle.

**Ship order is 0 → 2 → 1 → 3.** Tier 2 precedes Tier 1 deliberately: build configuration is
semantically safe, trivially reversible, and produces the most dramatic early wins. Source
patches are riskier and benefit from the verifier being battle-tested first.

---

## 8. Requirements by phase

Each phase ships independently. Each has a CLI before it has a GUI. Estimates assume ~30
hours/week, solo, with a learning curve on binary tooling.

---

### Phase 0 — Config Autotuner
**Timeline:** 5-7 weeks · **Binary expertise required:** minimal

The build-profile search space, measured on the user's actual crate.

| ID | Requirement | Priority |
|---|---|---|
| F0.1 | Detect cargo project/workspace, enumerate targets and current profiles | Must |
| F0.2 | Sweep a configurable matrix: `opt-level` (0/1/2/3/s/z), `lto` (off/thin/fat), `codegen-units`, `panic`, `strip`, `debug`, `overflow-checks` | Must |
| F0.3 | Optional extended sweep: `build-std` + `panic_immediate_abort`, `target-cpu`, `relocation-model` | Should |
| F0.4 | Measure per configuration: binary size (total + per section), wall-clock build time, test pass/fail | Must |
| F0.5 | Measure runtime via user-declared benchmark command, with statistical significance | Must |
| F0.6 | Compute and display the Pareto frontier over (size, runtime, build time) | Must |
| F0.7 | Emit `lodestone.json` artifact — the stable contract consumed by CLI, GUI, and CI | Must |
| F0.8 | Cache builds; resumable sweeps; parallelism cap | Should |
| F0.9 | *(AI)* Explain each configuration's effect and why it mattered on this crate | Should |
| F0.10 | *(AI)* Recommend a profile given a user-stated objective in natural language | Could |

**Acceptance:** On a five-crate reference corpus, `lode tune` finds a configuration that
reduces binary size by ≥25% versus the default release profile, with the test suite passing,
without human intervention.

---

### Phase 1 — Monomorphization & Size Attribution
**Timeline:** 6-9 weeks · **Binary expertise required:** ELF symbol tables, demangling

Where the bytes actually went, and what to do about it.

| ID | Requirement | Priority |
|---|---|---|
| F1.1 | Parse ELF symbol table; demangle Rust v0 and legacy symbols | Must |
| F1.2 | Attribute bytes to crate, module, and generic function | Must |
| F1.3 | Group instantiations of the same generic; report count and aggregate cost | Must |
| F1.4 | Identify size drivers: `core::fmt` machinery, panic strings and locations, unwinding tables, `Drop` glue, vtables, embedded static data | Must |
| F1.5 | Diff two binaries: what grew, what shrank, what appeared | Must |
| F1.6 | Map symbols to source spans via DWARF where available | Must |
| F1.7 | *(AI)* Propose collapse strategies for redundant instantiations (`&dyn`, inner non-generic function, `impl Trait` narrowing) | Should |
| F1.8 | *(AI)* Generate a patch implementing a proposal; verify via §9 harness | Should |
| F1.9 | Detect and report dead code retained by the linker | Could |

**Acceptance:** On a corpus crate with known monomorphization bloat, correctly identifies the
top three generics by aggregate cost, and at least one proposed patch reduces size ≥5% with
tests passing.

---

### Phase 2 — Post-mortem Crash Analysis
**Timeline:** 7-10 weeks · **Binary expertise required:** DWARF, unwinding — **this is the
phase where you learn DWARF properly**

Core dump in, root cause out. Offline and deterministic; no live process.

| ID | Requirement | Priority |
|---|---|---|
| F2.1 | Load ELF core dumps; correlate with binary and source tree | Must |
| F2.2 | Verify binary-source correspondence; refuse or warn loudly on stale builds | Must |
| F2.3 | Unwind the stack, including inlined frames via `DW_TAG_inlined_subroutine` | Must |
| F2.4 | Reconstruct local variables from DWARF location lists, including register and stack-slot residency | Must |
| F2.5 | Recover values reported "optimized out" by tracking them through disassembly where feasible | Should |
| F2.6 | Rust-aware panic handling: identify the panicking monomorphization, decode `panic::Location`, demangle through the panic machinery | Must |
| F2.7 | Synchronized source and disassembly views with address↔line mapping | Must |
| F2.8 | Classify crash type: index out of bounds, unwrap on None/Err, arithmetic overflow, explicit panic, segfault, stack overflow, abort | Should |
| F2.9 | *(AI)* Root cause hypothesis with an evidence chain citing specific tool outputs | Should |
| F2.10 | *(AI)* Propose a fix; verify it against a reproduction where one exists | Could |

**Acceptance:** On 20 crashes with known ground-truth root cause lines, identifies the
correct line in the top-3 candidates for ≥70%, with the deterministic layer alone correctly
symbolizing ≥95% of frames.

---

### Phase 3 — Performance Attribution
**Timeline:** 5-7 weeks · **Depends on Phase 2 symbolization**

| ID | Requirement | Priority |
|---|---|---|
| F3.1 | Drive `perf record`; ingest samples (or `samply` as an alternative source) | Must |
| F3.2 | Attribute samples to source lines using the Phase 2 symbolization layer | Must |
| F3.3 | Inlining-aware flamegraph — logical call structure, not just physical frames | Must |
| F3.4 | Differential profiling between two builds or two commits | Must |
| F3.5 | Surface PMU counters: cache misses, branch mispredicts, IPC, stalls | Should |
| F3.6 | Detect missed vectorization and other codegen quality issues in hot loops | Could |
| F3.7 | *(AI)* Explain why a region is hot in terms of the source construct responsible | Should |
| F3.8 | *(AI)* Propose and verify an optimization patch | Could |

**Acceptance:** On an injected 10%+ performance regression, correctly identifies the
responsible function in the top-3 for ≥70% of cases.

---

### Phase 4 — Replay Debugging (`rr`)
**Timeline:** open-ended · **The original vision; buildable only once 0-3 exist**

| ID | Requirement | Priority |
|---|---|---|
| F4.1 | Record and manage `rr` traces | Must |
| F4.2 | Reverse execution primitives: reverse-continue, reverse-step, reverse-finish | Must |
| F4.3 | Hardware watchpoints with reverse search — "who wrote this value?" | Must |
| F4.4 | Timeline scrubbing across the recorded execution | Must |
| F4.5 | Deterministic re-derivation of any displayed value from the trace | Must |
| F4.6 | *(AI)* Agentic root-cause search over the trace under a step budget | Should |
| F4.7 | Session transcripts are replayable and shareable as artifacts | Should |

**Acceptance:** On 20 injected bugs, locates the root cause line within a 40-step tool budget
for ≥60%, with the full search transcript inspectable.

---

## 9. The verification harness (cross-cutting)

Shared by every phase. Built in Phase 0 and extended thereafter. Any proposed change —
configuration or source — must pass:

1. **Build** succeeds, with warnings diffed against baseline.
2. **Test suite** passes (`cargo test`, or a user-specified command).
3. **Benchmark**, if declared, shows a statistically significant improvement or no
   significant regression. Significance via `hyperfine`/Criterion, not single-run comparison.
4. **Sanitizers**, when `unsafe` blocks are touched: Miri for the safe-Rust reachable
   portion, ASan/UBSan for the rest.
5. **Size** measured and recorded regardless of the objective.

A change that fails any gate is reported as a rejected candidate with the failing gate named.
Rejected candidates are shown in the UI, not silently dropped — near-misses are informative.

---

## 10. Interface requirements

### 10.1 GUI

The GUI is the primary surface. See DESIGN §7 for layout and DESIGN §6 for the technology
decision.

| ID | Requirement | Priority |
|---|---|---|
| U1 | Project view: select crate/workspace, configure targets, see run history | Must |
| U2 | Size Explorer: zoomable treemap by crate/module/generic/section, with diff mode | Must |
| U3 | Profile Lab: Pareto scatter of configurations; click a point for its flags and diff | Must |
| U4 | Findings Inspector: persistent panel showing hypothesis, evidence chain, provenance badges, proposed patch | Must |
| U5 | Source/disassembly split view with synchronized highlighting | Must |
| U6 | Agent Transcript: every tool call and result, collapsible, exportable | Must |
| U7 | Flamegraph view, source-linked (Phase 3) | Must |
| U8 | Timeline scrubber (Phase 4) | Should |
| U9 | Trust tier control, always visible in the chrome | Must |
| U10 | Model backend configuration: local runner or cloud with user-supplied key | Must |
| U11 | Dark theme default; light theme available | Should |
| U12 | Keyboard-first navigation with a command palette | Should |

### 10.2 CLI

Every GUI capability has a CLI equivalent, and the CLI ships first in each phase.
`lode tune`, `lode size`, `lode analyze <core>`, `lode profile`, `lode verify`.
All commands accept `--json` and emit the `lodestone.json` artifact.

### 10.3 CI mode

`lode ci` runs a size and performance budget check against a committed baseline artifact and
exits non-zero on regression. This is the cheapest path to organizational adoption: it gets
the tool into a repo without anyone installing a GUI.

---

## 11. Success metrics

### Adoption (community project — these are the honest ones)
- 12 months: 1,500 GitHub stars, 10 external contributors, 3 crates using `lode ci`
- Package presence: crates.io, and an AUR/Nixpkgs entry
- Cited in at least one conference talk or widely-shared blog post

### Product quality (measured on the reference corpus, tracked per release)
- Phase 0: median size reduction versus default release profile ≥25%
- Phase 1: top-3 accuracy on size-driver identification ≥80%
- Phase 2: root-cause top-3 accuracy ≥70%; frame symbolization accuracy ≥95%
- Phase 4: root-cause location within budget ≥60%

### Trust (the metric that determines survival)
- Rate of findings presented as high-confidence that are wrong: **<5%**
- Every AI-generated finding has ≥1 deterministic evidence item: **100%**, enforced in code,
  not by convention

---

## 12. Competitive landscape

| Tool | Overlap | Why Lodestone differs |
|---|---|---|
| `cargo-bloat`, `twiggy`, `bloaty` | Size measurement | They measure; Lodestone attributes and proposes |
| `cargo-flamegraph`, `samply`, Firefox Profiler | Profiling | No source-binary reasoning, no proposals |
| `perf` | Sampling | Raw; Lodestone is a consumer of it, not a competitor |
| **Pernosco** | Replay debugging | Closest competitor. Commercial, hosted, C++-first. Lodestone is local, open, Rust-first, and adds size/perf |
| `rr` | Replay | A dependency, not a competitor |
| Ghidra, Binary Ninja | Binary analysis | Source-free reverse engineering; opposite thesis |
| GitHub Copilot, Cursor | AI coding | Source-only. No binary, no runtime, no measurement |
| `cargo-llvm-lines` | Monomorphization | Counts IR lines; no attribution to output bytes, no proposals |

**The unoccupied position:** local, open source, source-aware, measurement-grounded, and
spanning correctness *and* performance *and* size in one artifact model.

---

## 13. Risks

| # | Risk | Severity | Mitigation |
|---|---|---|---|
| R1 | **Scope explosion.** Two products (debugger + optimizer) plus a GUI, solo. | Critical | Phase gates. Nothing from phase N+1 starts until phase N has shipped a release. Non-goals in §5 are binding. |
| R2 | **Model confabulation about binaries.** Models describe assembly with total confidence and are frequently wrong. | Critical | P1 and P2. Engine computes facts; model may only reference evidence it was given. Enforce in the type system (DESIGN §5.4). |
| R3 | **AI backlash in the Rust community.** A vocal segment rejects AI tooling outright. | High | Ship as a deterministic tool with an optional layer. `--no-ai` is first-class and documented. Never market as "AI debugger." |
| R4 | **GUI is a second discipline** the maintainer may not have. | High | CLI-first every phase; GUI is a view onto `lodestone.json`. A weak GUI does not block a useful CLI. |
| R5 | **Learning curve on DWARF/unwinding** stalls Phase 2. | High | Phase ordering puts it third, after two shipped releases build momentum and context. |
| R6 | **Solo maintainer burnout / bus factor 1.** | High | Ruthless phase scoping; public roadmap; label good-first-issues from day one; the CLI/artifact split makes contribution surfaces small. |
| R7 | **Benchmark noise** produces false optimization wins. | Medium | Statistical significance mandatory. Never accept a single-run comparison. Document the noise floor per machine. |
| R8 | **Local models too weak** for the harder reasoning; cloud models can't be used on proprietary binaries. | Medium | Design prompts against the weakest supported model. Ensure Tier 0/2 need no model at all. |
| R9 | **`rr` platform lock** limits the addressable audience. | Medium | Accepted. Phases 0-3 do not require `rr`. |
| R10 | **Upstream tool churn** (`perf`, `bloaty`, `rr` output formats). | Low | Version-pin, parse defensively, integration-test against pinned versions in CI. |

---

## 14. Open questions

1. **Local model floor.** What is the smallest model that produces useful monomorphization
   proposals? This determines the hardware requirement and should be answered empirically in
   Phase 1, not guessed.
2. **Evaluation corpus provenance.** Real-world bugs (higher fidelity, hard to collect) vs.
   mutation-injected bugs (easy to scale, possibly unrepresentative). Likely both; ratio TBD.
3. **Does the GUI ship in Phase 0 or Phase 1?** Phase 0 is CLI-shaped; the treemap and Pareto
   views are what make it compelling. Leaning toward a minimal GUI in Phase 0 covering U1-U4.
4. **Company or pure community project?** Affects licensing posture, CLA, and whether a
   hosted trace-sharing service is ever on the table. Does not need answering before Phase 2,
   but the license choice made now is effectively permanent.
5. **C/C++ support timing.** The architecture keeps the door open. Opening it costs
   significant effort in demangling, build system integration, and language-specific
   heuristics.

---

## Appendix A — Reference corpus

Five crates, fixed at project start, benchmarked every release. Chosen to exercise different
failure modes:

1. An embedded/`no_std` crate — the size-constrained case
2. A generics-heavy library (serde-adjacent) — the monomorphization case
3. A CLI application with a deep dependency tree — the realistic-application case
4. A WASM target — the payload case
5. A multi-crate workspace — the build-orchestration case

Baseline numbers recorded before any code is written.

---

## Appendix B — First 30 days

1. Choose a name; write the one-paragraph scope statement into the README **before** code.
2. Repository setup: dual license, CI on Linux x86-64, `cargo-deny`, `rustfmt`, Clippy denies.
3. Define the `lodestone.json` schema. This is the contract between engine, CLI, and GUI —
   getting it right early prevents a painful refactor.
4. Build the Phase 0 core with no AI whatsoever: sweep, build, measure, report.
5. Lock in the reference corpus and record baselines.
6. Only then add the explanation layer.

The 30-day success condition: one command, run on a real crate, cuts its binary
substantially, with proof the tests still pass.

---

## Appendix C — Naming criteria

- Not already on crates.io, and no conflicting GitHub organisation
- Pronounceable by non-native English speakers
- Short CLI verb (≤5 characters) available
- Does not contain "AI", "GPT", "Copilot", or "smart" — see R3
- Suggests navigation, sight, or depth rather than intelligence

Candidates: `lodestone`/`lode`, `sonde`, `plumb`, `strata`, `bathyscope`.
