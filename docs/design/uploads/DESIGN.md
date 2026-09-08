# Lodestone — Technical Design

> Companion to `PRD.md`. This document covers architecture, data model, the agent design,
> the GUI, and the testing strategy. It describes the target architecture; not all of it
> exists in any given phase.

---

## 1. Design thesis

One sentence: **a deterministic analysis engine that produces evidence, and a language model
that may only reason over evidence the engine produced.**

Everything else follows from that. The model never computes an address, never reports a size,
never claims a value. It receives structured facts and produces hypotheses, rankings, and
explanations that cite those facts. This is not a stylistic preference — it is what makes the
tool trustworthy enough to be worth installing, and what lets it work at all with the model
disabled.

The second structural idea: **correctness debugging and performance debugging are the same
loop with a different oracle.**

```
   ┌─────────────┐
   │ Observation │  crash, slow benchmark, large binary
   └──────┬──────┘
          ▼
   ┌─────────────┐
   │ Hypothesis  │  model proposes, or heuristic proposes
   └──────┬──────┘
          ▼
   ┌─────────────┐
   │ Experiment  │  a tool call that could refute it
   └──────┬──────┘
          ▼
   ┌─────────────┐
   │  Evidence   │  deterministic tool output
   └──────┬──────┘
          ▼
   ┌─────────────┐
   │  Verdict    │  oracle: is it wrong? slow? big?
   └─────────────┘
```

The oracle differs per analysis. The loop, the evidence store, the tool registry, and the
verifier are shared. This is why the debugger and the optimizer belong in one codebase rather
than two.

---

## 2. System architecture

```
┌────────────────────────────────────────────────────────────────────┐
│                          PRESENTATION                              │
│  ┌──────────────────┐    ┌──────────────┐    ┌──────────────────┐  │
│  │  GUI (Tauri)     │    │  CLI (clap)  │    │  CI mode         │  │
│  │  TS + web views  │    │              │    │  budget gates    │  │
│  └────────┬─────────┘    └──────┬───────┘    └────────┬─────────┘  │
└───────────┼─────────────────────┼─────────────────────┼────────────┘
            └─────────────────────┼─────────────────────┘
                                  ▼
                    ┌─────────────────────────┐
                    │   lodestone.json        │   ← stable contract
                    │   Session / Findings    │
                    └────────────┬────────────┘
                                 ▼
┌────────────────────────────────────────────────────────────────────┐
│                        ORCHESTRATION                               │
│  ┌────────────────┐  ┌─────────────────┐  ┌────────────────────┐   │
│  │ Agent loop     │  │ Tool registry   │  │ Verifier           │   │
│  │ (optional)     │  │ + budgets       │  │ (always on)        │   │
│  └────────────────┘  └─────────────────┘  └────────────────────┘   │
└────────────────────────────────────────────────────────────────────┘
                                 ▼
┌────────────────────────────────────────────────────────────────────┐
│                      ANALYSIS ENGINES                              │
│  config-sweep │ size-attribution │ crash-analysis │ perf-attrib    │
│                        │ replay-search                             │
└────────────────────────────────────────────────────────────────────┘
                                 ▼
┌────────────────────────────────────────────────────────────────────┐
│                        FACT PROVIDERS                              │
│  ┌────────────┬───────────┬────────────┬───────────┬────────────┐  │
│  │ Binary     │ Build     │ Measure    │ Runtime   │ External   │  │
│  │ object     │ cargo     │ bloaty     │ LLDB API  │ processes  │  │
│  │ gimli      │ rustc     │ hyperfine  │ rr        │            │  │
│  │ addr2line  │ profiles  │ perf       │ core dump │            │  │
│  │ iced-x86   │           │ criterion  │           │            │  │
│  └────────────┴───────────┴────────────┴───────────┴────────────┘  │
└────────────────────────────────────────────────────────────────────┘
```

**The `lodestone.json` boundary is load-bearing.** Every presentation surface consumes it and
nothing else. This is what allows a weak or absent GUI to not block a useful CLI, what makes
the tool scriptable, and what makes evaluation possible (see §10).

---

## 3. Workspace layout

### 3.1 Crates

```
lodestone/
├── crates/
│   ├── lode-core/        Types, errors, config, Finding/Evidence model
│   ├── lode-binary/      ELF, DWARF, symbols, disassembly
│   ├── lode-build/       Cargo integration, profile matrix, build orchestration
│   ├── lode-measure/     Size, time, perf sample ingestion
│   ├── lode-verify/      The verification harness
│   ├── lode-analyze/     Analysis engines (one module per analysis)
│   ├── lode-agent/       Model abstraction, tool registry, agent loop
│   ├── lode-session/     Session store, artifact serialisation, replay
│   ├── lode-cli/         clap front-end
│   └── lode-gui/         Tauri backend + web frontend
├── corpus/               Reference crates (submodules) + baselines
├── evals/                Evaluation harness and ground-truth fixtures
└── docs/
```

**Dependency rule:** `lode-agent` may depend on `lode-analyze`, never the reverse. An
analysis engine must never call a model. If an analysis wants intelligence, it emits a
structured question and the orchestration layer decides whether to answer it with a model.
This is what keeps P1 (deterministic core) architecturally enforced rather than aspirational.

### 3.2 Keeping the C/C++ door open

Language-specific logic is confined to two traits in `lode-core`:

```rust
/// Symbol name → structured meaning.
pub trait SymbolInterpreter: Send + Sync {
    fn demangle(&self, raw: &str) -> Option<DemangledSymbol>;
    /// Group instantiations that share a generic origin.
    fn generic_origin(&self, sym: &DemangledSymbol) -> Option<GenericOrigin>;
    fn classify(&self, sym: &DemangledSymbol) -> SymbolRole;  // Drop glue, vtable, fmt, ...
}

/// Build system integration.
pub trait BuildSystem: Send + Sync {
    fn discover(&self, root: &Path) -> Result<ProjectModel>;
    fn profile_axes(&self) -> Vec<ProfileAxis>;
    fn build(&self, cfg: &BuildConfig) -> Result<BuildOutput>;
    fn test_command(&self) -> Command;
}
```

v1 ships `RustSymbols` and `CargoBuild`. Adding C++ means adding an Itanium ABI demangler and
a CMake/compile-commands integration, with no change to the engines. Do not write these in
v1; do keep the seam.

### 3.3 Dependencies

| Concern | Choice | Note |
|---|---|---|
| ELF/Mach-O | `object` | Same maintainers as `gimli` |
| DWARF | `gimli` | Study `examples/dwarfdump.rs` before writing anything |
| Address→line | `addr2line` | Wraps `gimli`; handles inline frames |
| Demangling | `rustc-demangle` | v0 and legacy |
| Disassembly | `iced-x86` | x86-only, fast, excellent formatter control |
| CLI | `clap` (derive) | |
| Async | `tokio` | Needed for parallel builds and GUI IPC |
| Serialisation | `serde` + `serde_json` | |
| Schema | `schemars` | Generate JSON Schema for `lodestone.json` from Rust types |
| Session store | `rusqlite` | Single-file, no server, good for transcripts |
| Errors | `thiserror` (libs), `anyhow` (binaries) | |
| Tracing | `tracing` + `tracing-subscriber` | |
| Process control | `std::process` / `tokio::process` | |
| Testing | `insta` (snapshots), `proptest` | |

**Shelled-out tools:** `bloaty`, `hyperfine`, `perf`, `rr`, `cargo`, `rustc`, `lldb`. Each
represents years of work not worth reimplementing. Wrap each in a module that version-pins,
parses defensively, and has a snapshot test against a captured output sample.

**Never vendored:** a decompiler. Phase 4 may shell out to Ghidra headless if the P-Code view
turns out to be necessary, but that is an explicit later decision, not a v1 dependency.

---

## 4. Data model

The whole product reduces to `Finding`. Every analysis produces findings; every UI renders
findings; every eval scores findings.

```rust
pub struct Finding {
    pub id: FindingId,
    pub kind: FindingKind,
    pub title: String,              // one line, imperative or declarative
    pub detail: String,             // prose; may be model-generated
    pub location: Option<Location>,
    pub impact: Impact,
    pub confidence: Confidence,
    pub evidence: Vec<EvidenceId>,  // never empty — enforced at construction
    pub proposal: Option<Proposal>,
    pub provenance: Provenance,
}

pub enum FindingKind {
    SizeDriver { bytes: u64, share: f32 },
    MonomorphizationBloat { generic: String, instantiations: u32, bytes: u64 },
    ConfigOpportunity { axis: String, from: String, to: String },
    CrashCause { class: CrashClass },
    PerfHotspot { samples: u64, share: f32 },
    Regression { metric: Metric, delta: f64, significance: f64 },
    DeadCode { bytes: u64 },
    StaleArtifact { detail: String },
}

pub enum Location {
    Source { file: PathBuf, line: u32, col: Option<u32>, span: Option<Range<u32>> },
    Address { addr: u64, len: Option<u64>, section: String },
    Symbol { name: String, demangled: String },
    /// The interesting case: both views of the same thing.
    Bridged { source: Box<Location>, address: Box<Location>, via: MappingMethod },
}

pub enum MappingMethod {
    DwarfLineTable,
    InlineFrame { depth: u32 },
    SymbolTable,
    /// The model guessed. Rendered differently, and always suspect.
    Inferred,
}
```

### 4.1 Evidence and provenance

This is the mechanism that implements P2, and it is the most important type in the codebase.

```rust
pub struct Evidence {
    pub id: EvidenceId,
    pub summary: String,
    pub provenance: Provenance,
    pub payload: EvidencePayload,   // raw tool output, retained verbatim
}

pub enum Provenance {
    /// A tool ran. Reproducible. This is a fact.
    Deterministic {
        tool: String,
        version: String,
        args: Vec<String>,
        output_digest: [u8; 32],
    },
    /// A model said so. This is a claim.
    Model {
        backend: String,
        model_id: String,
        prompt_digest: [u8; 32],
        cited: Vec<EvidenceId>,   // must be non-empty
    },
    /// A rule in our own code. Reproducible, but our logic.
    Heuristic { rule: String },
}
```

**Construction invariant, enforced in the constructor rather than by convention:**

```rust
impl Finding {
    pub fn from_model(/* ... */) -> Result<Self, GroundingError> {
        if cited.is_empty() {
            return Err(GroundingError::UngroundedModelClaim);
        }
        // every cited id must resolve in the session's evidence store
        // ...
    }
}
```

A model-provenance finding that cites nothing cannot be constructed. This turns P2 from a
guideline into a compile-and-run-time guarantee, which is the difference between a principle
that survives and one that erodes under deadline pressure.

### 4.2 Confidence

```rust
pub enum Confidence {
    /// Directly measured. "This symbol is 40,112 bytes."
    Certain,
    /// Deterministic derivation with assumptions. "This maps to line 42."
    High,
    /// Model inference with strong evidence.
    Probable,
    /// Model inference with weak or partial evidence.
    Speculative,
}
```

The UI must render `Certain`/`High` and `Probable`/`Speculative` with visually distinct
treatments. The PRD's headline trust metric — <5% of high-confidence findings wrong — is
measured against this enum, so the boundary between `High` and `Probable` is a product
decision, not an implementation detail. **Rule: anything with `Provenance::Model` caps at
`Probable`.**

### 4.3 Session

```rust
pub struct Session {
    pub id: SessionId,
    pub project: ProjectFingerprint,   // repo, commit, dirty flag, toolchain
    pub started: DateTime<Utc>,
    pub trust_tier: TrustTier,
    pub findings: Vec<Finding>,
    pub evidence: EvidenceStore,
    pub transcript: Vec<TranscriptEntry>,  // every tool call, in order
    pub verdicts: Vec<VerificationRun>,
}
```

Sessions persist to SQLite. Two consequences worth designing for from the start: a session
can be exported and attached to a bug report, and the eval harness (§10) is just a program
that replays sessions and scores their findings.

---

## 5. The agent layer

### 5.1 Model abstraction

```rust
#[async_trait]
pub trait ModelBackend: Send + Sync {
    fn id(&self) -> &str;
    fn context_window(&self) -> usize;
    fn supports_tools(&self) -> bool;
    async fn complete(&self, req: CompletionRequest) -> Result<CompletionResponse>;
}
```

Implementations: `LocalBackend` (llama.cpp/Ollama-compatible HTTP), `AnthropicBackend`,
`OpenAiBackend`, `NullBackend`.

`NullBackend` is not a stub. It is the backend used when `--no-ai` is set, and **the full
test suite runs against it in CI**. If a feature breaks under `NullBackend`, that feature
violates P1 and the build fails. This is how the deterministic-core principle stays true over
time.

Prompts are authored against the *weakest* supported local model. If a prompt needs a
frontier model to work, the feature is scoped wrong.

### 5.2 Tool registry

Tools are the model's only access to reality.

```rust
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn schema(&self) -> serde_json::Value;   // JSON Schema for arguments
    fn cost(&self) -> ToolCost;              // Cheap | Moderate | Expensive
    async fn invoke(&self, args: Value, ctx: &mut SessionCtx) -> Result<ToolOutput>;
}
```

Every invocation appends an `Evidence` record with `Provenance::Deterministic` before the
result is returned to the model. There is no path by which a model observes a fact that is
not simultaneously recorded as citable evidence.

**Tool inventory by phase:**

| Phase | Tools |
|---|---|
| 0 | `project_info`, `build_with_config`, `measure_size`, `run_tests`, `benchmark`, `section_breakdown` |
| 1 | `list_symbols`, `symbol_bytes`, `demangle`, `group_monomorphizations`, `read_source_span`, `diff_binaries`, `find_symbol_source` |
| 2 | `load_core`, `unwind_stack`, `frame_locals`, `resolve_address`, `disassemble_range`, `read_memory`, `dwarf_query`, `inline_chain` |
| 3 | `perf_record`, `hot_symbols`, `attribute_samples`, `pmu_counters`, `diff_profiles` |
| 4 | `rr_record`, `run_to`, `reverse_continue`, `watch_address`, `reverse_search`, `read_var_at_time` |

**Design notes on tool outputs:**

- `disassemble_range` returns *annotated* disassembly by default: instructions interleaved
  with source lines from the line table, symbolized call targets, and resolved constants.
  Raw bytes are a separate, explicitly-requested mode.
- `read_memory` returns hex only for the specific range requested, capped at 256 bytes, with
  an interpretation attempt (does this look like a pointer, a UTF-8 string, a known struct
  layout from DWARF?). The model is bad at reading hex and good at reading interpretations.
- Every tool output carries a `truncated: bool` and a hint for narrowing. Silent truncation
  is how agents end up confidently wrong.

### 5.3 The loop

```rust
pub struct AgentConfig {
    pub max_steps: u32,          // hard budget, default 25
    pub max_tokens: u64,
    pub max_wall_time: Duration,
    pub require_hypothesis: bool, // default true
}
```

Each step:

1. **Hypothesis required.** The model must state what it believes and what would refute it
   before it may call a tool. This is enforced structurally — a response without a hypothesis
   block is rejected and retried once, then the step is aborted. It sharply reduces aimless
   tool-call wandering, which is the dominant failure mode of agentic debugging.
2. **Tool selection**, with the remaining budget visible to the model.
3. **Invoke**, record evidence, append to transcript.
4. **Revise or conclude.**

On budget exhaustion the agent must emit its best current hypothesis **explicitly marked
`Speculative`**, along with what it would investigate next. A partial answer with an honest
label is useful; a fabricated confident answer is worse than nothing.

### 5.4 Context strategy

A disassembled function is enormous and a binary is unthinkably larger. The rule is: **slice
by data dependency, never by address range.**

Concretely, when investigating a value:
- Backward slice: which instructions could have written this register or stack slot?
- Forward slice: what consumed it?
- Include the DWARF type layout, not raw bytes.
- Include the source span for the enclosing function and any inlined frames.
- Exclude everything else, and say so — an explicit "47 instructions omitted, call
  `disassemble_range` to see them" is safer than a silent cut.

For source, use tree-sitter for semantic chunking rather than line windows, so a chunk is a
function or an impl block rather than an arbitrary 100 lines.

---

## 6. GUI: technology decision

**Choice: Tauri 2, with a TypeScript frontend.**

Rationale:

- The core views are data-dense and text-heavy — treemaps, flamegraphs, synchronized
  source/disassembly panes, virtualized tables of tens of thousands of symbols. The web
  platform has mature, battle-tested solutions for all of these (CodeMirror 6 or Monaco for
  code, d3 for treemap and flamegraph layout, TanStack Virtual for large tables). Rebuilding
  a competent code viewer in an immediate-mode GUI is months of work that produces something
  worse.
- The backend is Rust, so `lode-gui` calls the existing crates directly. No FFI boundary, no
  re-serialisation of foreign types, no second implementation of anything.
- Binary size and memory footprint are a fraction of Electron's — which matters for
  credibility when the product's headline feature is making binaries smaller.

Alternatives and why not:

| Option | Verdict |
|---|---|
| `egui` | Fastest to a first window. Poor at rich text, code display, and complex layout. Would become the bottleneck by Phase 2. |
| `iced` | Improving, but the widget ecosystem for virtualized tables and code views is not there. |
| Electron | JS backend means reimplementing or IPC-bridging the entire engine. Wrong language. |
| Web app | Violates P4. The tool must read local binaries and drive local processes. |
| VS Code extension | Attractive distribution, but constrains the UI severely and couples the project to one editor. Reconsider as a *secondary* surface post-v1. |

**Frontend stack:** TypeScript, a lightweight framework (Svelte or React — pick one and do
not revisit), CodeMirror 6, d3 for layout computation only (rendering via canvas for the
large views), TanStack Virtual.

**Rendering rule:** anything that can show >5,000 elements renders to canvas, not DOM. A
treemap of a real binary has tens of thousands of leaves; DOM will not survive it.

### 6.1 GUI/engine boundary

The frontend never computes analysis. It requests, receives `lodestone.json`-shaped data, and
renders. Tauri commands are thin:

```rust
#[tauri::command] async fn open_project(path: String) -> Result<ProjectModel>;
#[tauri::command] async fn start_analysis(kind: AnalysisKind, opts: Value) -> Result<RunId>;
#[tauri::command] async fn get_findings(session: SessionId) -> Result<Vec<Finding>>;
#[tauri::command] async fn get_evidence(id: EvidenceId) -> Result<Evidence>;
#[tauri::command] async fn verify_proposal(id: ProposalId) -> Result<VerificationRun>;
#[tauri::command] async fn set_trust_tier(tier: TrustTier) -> Result<()>;
```

Long-running work streams progress over Tauri events (`analysis:progress`,
`analysis:finding`, `agent:step`). Findings stream in as they are discovered rather than
appearing in a batch at the end — a config sweep takes minutes and a blank screen for minutes
is a broken experience.

---

## 7. GUI: interface design

### 7.1 Frame

```
┌──────────────────────────────────────────────────────────────────────┐
│ ▸ my-crate @ a3f9c21   [Trust: Propose ▾]  [Model: local/qwen ▾]  ⚙  │
├────────┬─────────────────────────────────────────┬───────────────────┤
│        │                                         │                   │
│  Nav   │            Main view                    │  Findings         │
│        │                                         │  Inspector        │
│ Size   │   (treemap / pareto / source+disasm     │                   │
│ Tune   │    / flamegraph / timeline)             │  ┌─────────────┐  │
│ Crash  │                                         │  │ Hypothesis  │  │
│ Perf   │                                         │  ├─────────────┤  │
│ Agent  │                                         │  │ Evidence    │  │
│        │                                         │  │  ● measured │  │
│        │                                         │  │  ● measured │  │
│        │                                         │  │  ◆ inferred │  │
│        │                                         │  ├─────────────┤  │
│        │                                         │  │ Proposal    │  │
│        │                                         │  │ [ Verify ]  │  │
│        │                                         │  └─────────────┘  │
├────────┴─────────────────────────────────────────┴───────────────────┤
│ ⚡ Sweeping configs  17/36  ·  best so far: 1.2 MB (−38%)  ·  2m14s   │
└──────────────────────────────────────────────────────────────────────┘
```

**The Findings Inspector is always present.** This is the single most important UI decision
in the product. It is the physical manifestation of P2: a claim and its evidence occupy the
same screen, always, and you cannot read one without seeing the other.

Provenance is carried by a consistent visual language used everywhere:

- **●  Measured** — deterministic tool output. Solid marker.
- **◈  Derived** — our own heuristic. Half-filled marker.
- **◆  Inferred** — model output. Outlined marker, distinct hue.

Clicking any evidence item opens the verbatim tool output, including the exact command line
and its digest. No black boxes.

### 7.2 Views

**Size Explorer.** Zoomable treemap, canvas-rendered. Grouping switchable between crate,
module, generic origin, and ELF section. Colour encodes category (your code, dependencies,
`core::fmt`, panic machinery, unwinding tables, `Drop` glue, vtables, static data). A diff
mode overlays two builds with growth in warm colours and shrinkage in cool. Selecting a
region populates the Inspector with the findings for that region.

**Profile Lab.** Scatter plot of every swept configuration: size on one axis, runtime on the
other, build time encoded as point size. The Pareto frontier is drawn as a line. Clicking a
point shows its flags, the diff from the current profile, and a one-click "apply to
Cargo.toml" that is gated by the trust tier. Failed configurations appear greyed with the
failing gate named — a config that broke the tests is informative.

**Crash Analysis.** Three panes. Left: the stack, with inlined frames nested and visually
marked as such, since "this frame did not physically exist" is exactly the confusion the tool
exists to resolve. Centre: source with reconstructed variable values shown inline at the
faulting line. Right: annotated disassembly, scroll-synchronized with the source pane via the
line table. Values recovered from registers rather than read directly are marked as derived,
with the recovery path viewable.

**Flamegraph.** Standard flamegraph with inline frames expanded into their logical structure.
Clicking a frame jumps the source pane. Differential mode colours by delta against a
baseline.

**Agent Transcript.** Every step as a card: hypothesis, tool call with arguments, result,
revision. Collapsible, exportable, and replayable. This view is not a debug affordance — it
is a trust affordance. Users who can watch the reasoning will forgive it being wrong; users
shown only a conclusion will not.

**Timeline (Phase 4).** A scrubber across recorded execution. Dragging moves the replay
position; the source, disassembly, and variable panes all update. Watchpoint hits mark the
track.

### 7.3 Interaction principles

- **Nothing blocks.** Every analysis is cancellable and streams partial results.
- **Every number is clickable** and leads to how it was obtained.
- **Trust tier is never raised silently.** Raising it is a deliberate act with a confirmation
  that states what changes.
- **Keyboard-first**, with a command palette. The audience lives in terminals.
- **Empty states teach.** A first-run Size Explorer explains what a treemap of a binary
  means. This is many users' first exposure to binary-level thinking.

---

## 8. The verification harness

`lode-verify` is the most reusable component in the system and should be built first.

```rust
pub struct VerificationRequest {
    pub baseline: BuildRef,
    pub candidate: BuildRef,
    pub gates: Vec<Gate>,
    pub objective: Objective,   // MinimizeSize | MinimizeTime | Pareto
}

pub enum Gate {
    Builds,
    TestsPass { command: Option<String> },
    NoNewWarnings,
    BenchmarkNotWorse { significance: f64 },
    BenchmarkImproves { min_delta: f64, significance: f64 },
    SizeNotWorse,
    MiriClean,      // when unsafe is touched
    SanitizersClean { san: Vec<Sanitizer> },
}

pub struct VerificationRun {
    pub outcome: Outcome,               // Accepted | Rejected { gate: Gate } | Inconclusive
    pub measurements: Measurements,
    pub gate_results: Vec<GateResult>,
}
```

**Statistical discipline is non-negotiable.** A single-run comparison is not evidence. Use
`hyperfine` with a warm-up and a minimum run count, report confidence intervals, and require
non-overlapping intervals or an explicit significance test before declaring an improvement.
Record the machine's measured noise floor once and refuse to claim improvements below it.
Benchmark noise producing false wins is how an optimization tool destroys its own credibility
(PRD R7).

**Rejected candidates are retained**, surfaced in the UI with the failing gate named. A
configuration that shaved 40% but broke three tests is a useful thing for a human to see.

---

## 9. Security and privacy

Lodestone reads proprietary source and binaries and executes untrusted code. Both directions
matter.

**Data egress.** With a local backend, nothing leaves the machine. With a cloud backend, the
UI must state clearly what will be sent before the first request, and a per-project setting
must be able to forbid cloud backends entirely. A `lodestone.toml` in a repo can pin
`allow_cloud_models = false`, so an organization can make the decision once.

**Execution.** Building and running a project executes arbitrary code by design — that is
what a build is. Do not pretend otherwise; document it. The `rr` and `perf` paths require
specific `ptrace_scope` and `perf_event_paranoid` settings; detect and explain rather than
silently failing.

**Injection through analyzed content.** Symbol names, panic strings, and source comments end
up in model prompts and can contain adversarial text. Treat all analyzed content as untrusted
data: fence it in the prompt, never let it be interpreted as instruction, and never let a
model-proposed action execute without passing the verifier. The verifier is the security
boundary as well as the quality boundary.

**Secrets.** API keys go to the OS keyring, never to a config file in the repo.

**Session exports** may contain source snippets and symbol names. Warn on export and offer a
redaction pass.

---

## 10. Testing and evaluation

Two separate problems: the deterministic layer needs conventional testing; the model layer
needs measurement.

### 10.1 Deterministic layer

- Unit tests per crate.
- **Snapshot tests (`insta`) against pinned external tool outputs.** Capture real `bloaty`,
  `perf`, and `readelf` output into fixtures. This catches upstream format churn (R10)
  without needing those tools installed in CI.
- **Golden binaries.** Commit small pre-built binaries with known DWARF characteristics —
  one with heavy inlining, one stripped, one with optimized-out locals, one `no_std`. Test
  symbolization against known-correct answers.
- Property tests on address↔line mapping round-trips.
- Full suite runs under `NullBackend` (§5.1). **This is the CI gate that enforces P1.**

### 10.2 Model layer: the eval harness

Without this you cannot tell whether a prompt change helped, and you will make the product
worse while believing you improved it.

```
evals/
├── fixtures/
│   ├── crashes/       binary + core + source + ground_truth.toml
│   ├── size/          crate + known top-N size drivers
│   ├── perf/          crate + injected regression + ground truth
│   └── replay/        rr trace + ground-truth root cause line
└── harness/           runs sessions, scores findings, reports
```

Each fixture declares ground truth:

```toml
[ground_truth]
root_cause_file = "src/parser.rs"
root_cause_line = 142
class = "IndexOutOfBounds"
acceptable_lines = [141, 142, 143]   # tolerance band
```

Metrics tracked per release, per model backend:

| Metric | Definition |
|---|---|
| Top-1 / Top-3 accuracy | Is ground truth the first / among first three findings? |
| Steps to answer | Tool calls consumed before the correct finding |
| Grounding rate | Fraction of model findings with ≥1 deterministic evidence item (must be 1.0) |
| Confident-wrong rate | High-confidence findings that are incorrect (the trust metric) |
| Cost | Tokens and wall-clock, per backend |

**Fixture sourcing.** Injected bugs scale and give exact ground truth; use `cargo-mutants` and
hand-written injections. Real bugs are more representative; mine RustSec advisories and
closed issues from large crates where the fixing commit identifies the line. Use both, and
report the two populations separately, because injected-bug performance systematically
overstates real-world performance.

**Run evals in CI on every prompt change.** A prompt is code.

---

## 11. Build order

Derived from the PRD phases, expressed as dependencies:

```
lode-core (types, Finding/Evidence)
    │
    ├── lode-build ──┐
    ├── lode-measure─┼── lode-verify ──┐
    └── lode-session─┘                 │
                                       ├── Phase 0: config sweep  ── SHIP
                                       │
              lode-binary (symbols) ───┼── Phase 1: size attrib   ── SHIP
                        │              │
              lode-binary (DWARF) ─────┼── Phase 2: crash analysis── SHIP
                        │              │
                        └──────────────┼── Phase 3: perf attrib   ── SHIP
                                       │
              lode-agent + rr ─────────┴── Phase 4: replay        ── SHIP
```

Two things are built before anything else and never rewritten: the `Finding`/`Evidence` model
(§4) and the `lodestone.json` schema. Every later component depends on them, so a mistake
there is the one refactor that would genuinely hurt. Spend the first week on them.

The GUI enters at Phase 0 with a minimal shell — project view, Profile Lab, and the Findings
Inspector — and grows one view per phase. It never leads; it always follows a working CLI.

---

## 12. Decisions deliberately deferred

Recorded so they are decided rather than drifted into:

| Decision | Defer until | Note |
|---|---|---|
| Ghidra/decompiler integration | Phase 4 | Heavy dependency. Only if annotated disassembly proves insufficient for the model. |
| C/C++ support | Post-v1 | Seam exists (§3.2). Cost is demangling + build integration. |
| VS Code extension | Post-v1 | Attractive distribution; constrains UI. Secondary surface at best. |
| Hosted trace sharing | Never, without an explicit governance decision | Contradicts P4 as currently written. |
| Fine-tuned model | Post-eval-harness | Cannot evaluate a fine-tune without §10.2 existing first. |
| macOS / Windows | Post-v1 | Phases 0-1 are nearly portable already; 2-4 are not. |
| Plugin API | When the second external contributor asks | Premature abstraction otherwise. |
