# Binmap — Technical Design

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
│  ┌──────────────────────────┐      ┌──────────────────────────┐    │
│  │  GUI (GPUI)              │      │  headless dev harness    │    │
│  │  gpui-component          │      │  (evals only, unshipped) │    │
│  └────────────┬─────────────┘      └────────────┬─────────────┘    │
└───────────────┼─────────────────────────────────┼──────────────────┘
                └────────────────┬────────────────┘
                                 ▼
                    ┌─────────────────────────┐
                    │   binmap.json        │   ← stable contract
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

**The `binmap.json` boundary is load-bearing**, and more so now that the GUI is the only
shipped surface. With no CLI to keep the engine honest, this artifact is the forcing function:
the GUI and the headless eval harness consume identical data, so an engine that only works when
driven by the UI fails the eval suite immediately. It is also the session export format and the
substrate for evaluation (§10).

---

## 3. Workspace layout

### 3.1 Crates

```
binmap/
├── crates/
│   ├── binmap-core/        Types, errors, config, Finding/Evidence model
│   ├── binmap-binary/      ELF, DWARF, symbols, disassembly
│   ├── binmap-build/       Cargo integration, profile matrix, build orchestration
│   ├── binmap-measure/     Size, time, perf sample ingestion
│   ├── binmap-verify/      The verification harness
│   ├── binmap-analyze/     Analysis engines (one module per analysis)
│   ├── binmap-agent/       Tool registry, native agent loop, model backends
│   ├── binmap-acp/         ACP client — external agents (Claude Code, Codex)
│   ├── binmap-mcp/         MCP server exposing the tool registry to those agents
│   ├── binmap-session/     Session store, artifact serialisation, replay
│   ├── binmap-eval/        Headless harness. Dev-only; never published or shipped.
│   └── binmap-gui/         GPUI application — the only product surface
├── corpus/               Reference crates (submodules) + baselines
├── evals/                Evaluation harness and ground-truth fixtures
└── docs/
```

**Dependency rule:** `binmap-agent` may depend on `binmap-analyze`, never the reverse. An
analysis engine must never call a model. If an analysis wants intelligence, it emits a
structured question and the orchestration layer decides whether to answer it with a model.
This is what keeps P1 (deterministic core) architecturally enforced rather than aspirational.

### 3.2 Keeping the C/C++ door open

Language-specific logic is confined to two traits in `binmap-core`:

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
| ACP client | `agent-client-protocol` | Official Rust crate; pin and negotiate version |
| MCP server | an MCP server crate, or hand-rolled JSON-RPC | Small surface; evaluate before adding a dependency |
| Keyring | `keyring` | API key storage |
| Async | `tokio` | Needed for parallel builds and GUI IPC |
| Serialisation | `serde` + `serde_json` | |
| Schema | `schemars` | Generate JSON Schema for `binmap.json` from Rust types |
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

> **Superseded in detail by `DESIGN-AI.md`.** Summary only here, because the agent design
> grew large enough to need its own document once ACP entered the picture.

Binmap supports two AI modes, and the difference is a difference in *who owns the loop*.

**Mode A — native agent.** Binmap drives the model directly through a user-supplied API key.
It owns the loop: it forces a hypothesis before every tool call, enforces a step and cost
budget, and refuses to construct a finding that cites no evidence. Two API shapes cover the
provider list — an Anthropic-shaped backend for Claude, and an OpenAI-compatible backend for
OpenAI, DeepSeek, Kimi, Z.ai, and local runners. Providers are a data table, not code.

**Mode B — ACP.** Binmap becomes a *client* and an external agent (Claude Code, Codex CLI)
becomes the reasoner. The agent owns its loop, its context strategy, its model, and its
billing. Binmap exposes its analysis tools to that agent through a loopback MCP server
handed over at `session/new`.

**One tool registry, two exposures.** A tool is written once against the `Tool` trait and
surfaced twice: as function-calling declarations for Mode A, and over MCP for Mode B. Writing a
tool twice means the abstraction has broken.

**The grounding invariant survives both modes, by different mechanisms.** In Mode A it is
enforced in the constructor. In Mode B it cannot be — an external agent's loop is not ours to
audit — so it is enforced at the airlock instead: every tool call issues an `EvidenceId`, and
every finding is validated against the evidence actually issued in that session before it
enters the UI. An agent cannot fabricate a citation because it never sees ids it was not given.

**`NullBackend` remains first-class.** "No reasoner" is a selectable option in the UI, and the
full test suite runs against it in CI. A feature that breaks without a model violates P1 and
fails the build.

Tool inventory by phase is unchanged from the table in `DESIGN-AI.md` §4.4. Context strategy —
slice by data dependency, never by address range — is unchanged and detailed in `DESIGN-AI.md`
§7, with the important caveat that in Mode B the external agent manages its own context and
Binmap's slicing applies only to individual tool outputs.

---

## 6. GUI: technology decision

**Choice: GPUI, with the `gpui-component` widget library.**

> Full analysis, including the fork question, threading model, and per-view implementation
> plan, is in **`DESIGN-GUI.md`**. Summary only here.

The whole application is one Rust process. There is no web layer, no IPC, no second language,
and no system webview dependency. `binmap-gui` links the engine crates directly.

The three things that make this viable rather than merely appealing:

- **`gpui-component` already ships the hard widgets.** A virtualized data table for hundreds
  of thousands of rows, a code editor stable at 200K lines with tree-sitter highlighting, and
  a dock layout with resizable panels, tabs, and serializable splits. Those are the three
  components a debugger UI is mostly made of.
- **Custom painting is a first-class API.** The `canvas()` element plus `paint_quad`,
  `paint_path`, and `PathBuilder` cover the treemap and flamegraph directly on the GPU.
- **Distribution improves.** One static binary with no `webkit2gtk` runtime dependency,
  which is the single worst part of shipping a Tauri app on Linux.

The cost is real and stated plainly: GPUI is pre-1.0 with frequent breaking changes, the
ecosystem is forked several ways, and there is no d3 — treemap squarification and flamegraph
layout are written by hand (a few hundred lines each). See `DESIGN-GUI.md` §2 and §6.

Alternatives and why not:

| Option | Verdict |
|---|---|
| Tauri 2 + TS | Previously chosen. Two languages, an IPC boundary, and a webview dependency, in exchange for an ecosystem `gpui-component` largely replaces. |
| `egui` | Fastest to a first window. Poor at rich text and code display; would bottleneck by Phase 2. |
| `iced` | Improving, but no virtualized table or code editor of this maturity. |
| Electron | JS backend means bridging the entire engine. Wrong language. |
| Web app | Violates P4. Must read local binaries and drive local processes. |
| VS Code extension | Attractive distribution, constrains the UI. Secondary surface post-v1 at best. |

### 6.1 GUI/engine boundary

Removing the IPC boundary removes the thing that *structurally* prevented the GUI from
computing analysis. That separation must now be enforced deliberately.

**Rule:** `binmap-gui` depends on `binmap-core` and `binmap-session` only. It reaches analyses
exclusively through a facade trait defined in `binmap-core`:

```rust
#[async_trait]
pub trait Engine: Send + Sync {
    async fn open_project(&self, path: &Path) -> Result<ProjectModel>;
    async fn start_analysis(&self, kind: AnalysisKind, opts: Value) -> Result<RunId>;
    fn subscribe(&self, run: RunId) -> Receiver<EngineEvent>;
    async fn findings(&self, s: SessionId) -> Result<Vec<Finding>>;
    async fn evidence(&self, id: EvidenceId) -> Result<Evidence>;
    async fn verify(&self, id: ProposalId) -> Result<VerificationRun>;
}
```

The Cargo dependency graph enforces this — `binmap-gui` cannot call an analysis engine because
it does not depend on one. `binmap.json` remains the serialisation of everything crossing
this trait, so the CLI, the GUI, and the eval harness still consume identical data.

Analyses run on GPUI's background executor and emit `EngineEvent`s that the UI applies to
entities on the foreground. Findings stream in as discovered; a config sweep takes minutes
and a blank screen for minutes is a broken experience.

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

**Size Explorer.** Zoomable treemap, painted via a GPUI `canvas` element. Grouping switchable between crate,
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

`binmap-verify` is the most reusable component in the system and should be built first.

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

Binmap reads proprietary source and binaries and executes untrusted code. Both directions
matter.

**Data egress.** With a local backend, nothing leaves the machine. With a cloud backend, the
UI must state clearly what will be sent before the first request, and a per-project setting
must be able to forbid cloud backends entirely. A `binmap.toml` in a repo can pin
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

**External agents are subprocesses with capabilities.** An ACP agent can request file reads,
file writes, and terminal commands through the client, and Binmap is the client. Scope
filesystem access to the project root and refuse paths outside it outright rather than
prompting. See `DESIGN-AI.md` §11.

**Secrets.** API keys go to the OS keyring, never to a config file in the repo, never into a
session export.

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
- Full suite runs under `NullBackend`. **This is the CI gate that enforces P1.**
- The suite is driven by `binmap-eval` headlessly, never through the GUI. An engine feature that
  cannot be exercised without the UI is a layering violation and fails review.

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
binmap-core (types, Finding/Evidence)
    │
    ├── binmap-build ──┐
    ├── binmap-measure─┼── binmap-verify ──┐
    └── binmap-session─┘                 │
                                       ├── Phase 0: config sweep  ── SHIP
                                       │
              binmap-binary (symbols) ───┼── Phase 1: size attrib   ── SHIP
                        │              │
              binmap-binary (DWARF) ─────┼── Phase 2: crash analysis── SHIP
                        │              │
                        └──────────────┼── Phase 3: perf attrib   ── SHIP
                                       │
              binmap-agent (Mode A) ─────┼── Phase 1 also: native AI ── SHIP
              binmap-acp + binmap-mcp ─────┼── Phase 2 also: ACP        ── SHIP
              rr ──────────────────────┴── Phase 4: replay          ── SHIP
```

Two things are built before anything else and never rewritten: the `Finding`/`Evidence` model
(§4) and the `binmap.json` schema. Every later component depends on them, so a mistake
there is the one refactor that would genuinely hurt. Spend the first week on them.

The GUI enters at Phase 0 with a minimal shell — project view, Profile Lab, and the Findings
Inspector — and grows one view per phase. With no CLI, it can no longer *follow* a shipped
surface, so the discipline moves: within each phase, the engine is built and proven against the
headless eval harness first, and the UI is built on top of a working engine. The order inside a
phase is unchanged; only the thing that enforces it has changed.

---

## 12. Decisions deliberately deferred

Recorded so they are decided rather than drifted into:

| Decision | Defer until | Note |
|---|---|---|
| Ghidra/decompiler integration | Phase 4 | Heavy dependency. Only if annotated disassembly proves insufficient for the model. |
| C/C++ support | Post-v1 | Seam exists (§3.2). Cost is demangling + build integration. |
| VS Code extension | Post-v1 | Attractive distribution; constrains UI. Secondary surface at best. |
| Shipping a CLI or CI mode | Post-Phase-2 | Removed from v1 (PRD N8). The engine stays a library, so a separate crate can add this later without redesign. Revisit against PRD R13. |
| Binmap as an ACP *agent* | Post-v1 | Would let Zed or JetBrains drive Binmap's analysis. Possibly better distribution than a standalone GUI. Keep the tool registry clean enough to allow it. |
| Hosted trace sharing | Never, without an explicit governance decision | Contradicts P4 as currently written. |
| Fine-tuned model | Post-eval-harness | Cannot evaluate a fine-tune without §10.2 existing first. |
| macOS / Windows | Post-v1 | Phases 0-1 are nearly portable already; 2-4 are not. |
| Plugin API | When the second external contributor asks | Premature abstraction otherwise. |
