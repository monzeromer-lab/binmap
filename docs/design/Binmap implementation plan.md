# Binmap — implementation plan

A source-aware artifact analysis tool for Rust, TypeScript and C#. One GPUI desktop application, Linux x86-64 first.

> Revision 2 · 7 September 2026 · Status: pre-alpha, nothing has shipped · Sources: PRD §8–§12, DESIGN §3–§11, DESIGN-GUI §1–§10, DESIGN-AI §12, DESIGN-POLYGLOT §4–§6, TOOLING · Interface inventory in §4 is taken from the built design, Binmap v3

## 1. Scope and conventions

Nine phase blocks, every requirement in each, the order they are built in, and what has to be true before the next block starts. Estimates assume roughly thirty hours a week, solo, and carry a learning curve on both binary tooling and GPUI. They are longer than the earlier estimates because every phase now carries its own interface work instead of deferring it.

### 1.1 What ships

One GUI binary. **N8 is a stated non-goal: not a command-line tool.** There is no shipped CLI and no CI mode. The engine stays a library, and a development-only headless harness — `binmap-eval` — drives the eval suite without being packaged, documented or supported as a user surface. Three consequences run through this plan:

- Ordering discipline lives inside each phase: **engine first, proven against the headless harness, then interface.** The interface never leads a phase, and engine work never blocks on it.
- The interface is on the critical path from week one, which is a discipline the maintainer does not yet have. This is risk R4 and it is rated Critical.
- Keyboard access is the only accommodation left for users who live in terminals, and that is most of the audience — which is why `U12` is a Must rather than a Should.

### 1.2 Identifier conventions

Identifiers are the PRD's, unchanged. `F` is an engine requirement, `A` a model-layer requirement and `U` an interface requirement, each numbered by phase (`F0.1`, `A1.2`, `U2.3`). Two cross-cutting sets are numbered globally: `U1`–`U13` for the interface as a whole (PRD §11) and `AI.1`–`AI.12` for the model layer (PRD §10). Every one of them appears in §4 or §5.

## 2. Stack and workspace

### 2.1 The interface stack

**GPUI, with `gpui-component` via the `gpui-kit` umbrella.** This supersedes the earlier Tauri 2 recommendation, which rested on the claim that the web ecosystem had mature widgets for data-dense code interfaces that Rust frameworks lacked. For this particular set of widgets that is no longer true, and that is what changes the answer.

The whole application becomes one Rust process: no web layer, no IPC boundary, no second language, and no system webview. Three things make it viable rather than merely appealing — a virtualized data table built for very large row counts, a code editor stable at 200K lines with tree-sitter highlighting, and a dock layout with resizable panels and serializable splits. Those are exactly the three hardest widgets in the product. Custom painting through `canvas()`, `paint_quad` and `PathBuilder` covers the treemap and flamegraph on the GPU, and distribution becomes one static binary with no `webkit2gtk` dependency.

The costs are real and stated plainly: GPUI is pre-1.0 with frequent breaking changes, the ecosystem is forked several ways, there is no charting library to fall back on so squarification and flamegraph layout are written by hand, and the contributor pool is smaller than React's. The last point is partly offset by the fact that these contributors are systems programmers who would rather not write TypeScript.

### 2.2 Fork choice and version policy

**Start on Zed mainline `gpui` plus `gpui-kit`.** The kit exists to pin a matching framework release and re-export every layer, so the application depends on one crate and never on GPUI directly. The forks' selling point is features Zed rejects, such as custom shaders, which Binmap does not need; and switching later is a `[patch.crates-io]` entry rather than a code change. Keep that escape hatch documented and take it only on hitting a mainline wall.

Four rules make the pre-1.0 dependency a scheduled cost instead of a recurring surprise:

1. Pin exact versions, commit the lockfile, and prefer the kit so the framework and widget versions move as a pair.
2. All GPUI usage lives in `binmap-gui`, and every custom widget in `binmap-gui::widgets`, so an API change touches one module.
3. One deliberate upgrade per phase, at the start of the phase, never mid-feature. Upgrading the framework and debugging DWARF in the same week is how a solo project loses a month.
4. Screenshot-test the custom-painted views; framework rendering changes are otherwise silent.

### 2.3 Crates

| Crate | Responsibility | First needed |
|---|---|---|
| binmap-core | Types, errors, config, the Finding and Evidence model, the backend traits, the engine facade | 0 |
| binmap-verify | The verification harness. The most reusable component in the system; built first | 0 |
| binmap-build | Cargo integration, the profile matrix, build orchestration and caching | 0 |
| binmap-measure | Size, timing, statistical significance, sample ingestion | 0 |
| binmap-session | Session store, artifact serialisation, replay, redaction on export | 0 |
| binmap-eval | The headless harness. Development-only; never published or shipped | 0 |
| binmap-gui | The GPUI application. The only product surface, and the only crate that imports GPUI | 0 |
| binmap-analyze | Analysis engines, one module per analysis. Never calls a model | 1 |
| binmap-binary | ELF and symbols in Phase 1; DWARF, disassembly and core dumps in Phase 2 | 1 |
| binmap-agent | Tool registry, the native agent loop, model backends, the finding gate | 1 |
| binmap-bridge-node | A small Node helper for bundler introspection. Source maps stay native Rust | 1.5 |
| binmap-acp | ACP client for external agents | 2 |
| binmap-mcp | MCP server exposing the tool registry to those agents over loopback | 2 |
| binmap-bridge-dotnet | ClrMD, the ILC size accounting file, Portable PDB. No Rust equivalent exists | 2.5 |

Supporting trees: `corpus/` reference projects and baselines, `evals/` ground-truth fixtures, `docs/`. Replay work in Phase 4 extends `binmap-binary` rather than adding a crate.

### 2.4 Two dependency rules

**The model may not lead an analysis.** `binmap-agent` may depend on `binmap-analyze`, never the reverse. An analysis that wants intelligence emits a structured question, and the orchestration layer decides whether to answer it with a model.

**The interface may not compute.** Dropping the IPC boundary removed the thing that structurally prevented a view from calling an analysis, so the separation is now enforced by the dependency graph instead: `binmap-gui` depends on `binmap-core` and `binmap-session` only, and reaches analyses through a facade trait defined in core. It cannot call an engine because it does not depend on one. Analyses run on the background executor and emit events the interface applies to entities on the foreground.

### 2.5 Backend traits and the capability model

Artifact-specific logic is confined to traits in `binmap-core`: an artifact reader, a symbolizer, a build system and a measurement source. Roughly eighty per cent of the interface is shared across backends; the exceptions are the native-only views, Disassembly and Replay, and the web-only view, Load time, and they are the last things built in each family. Capabilities are declared per target and read by the interface — a view a target cannot support is absent from the nav rail rather than present and empty, and a blocked action states its reason instead of disappearing. Phase 0.5 exists to build this.

### 2.6 Data model

The whole product reduces to `Finding`. Every analysis produces findings, every view renders them, and the artifact is a list of them. A finding carries a kind, a location, an impact, a confidence, a provenance and a non-empty evidence list; evidence carries the tool, its arguments, a content digest of the output, and the output itself. `AI.7` requires the interface to distinguish three cases, not two:

| Provenance | Means | Ceiling |
|---|---|---|
| ● Measured | A tool ran. Command, arguments and output digest recorded | Certain |
| ◈ Derived | A named rule of ours concluded it from measured inputs | High |
| ◆ Inferred, natively | Our own loop, which forces a hypothesis and enforces the gate in the constructor | Probable |
| ◆ Inferred, externally | An ACP agent's claim, validated at the airlock. A weaker guarantee, and labelled as one | Probable |

Those words and the four confidence words — Certain, High, Probable, Speculative — are fixed vocabulary, never paraphrased in code, in the interface, or in this document.

## 3. Cross-cutting infrastructure

Seven pieces are built in Phase 0 and extended by every phase afterwards. Each later phase multiplies an error here, which is what makes them the most expensive things to get wrong.

- **The verification harness.** Five gates, detailed in §6, applied to every proposal in every phase.
- **The evidence store.** Every tool invocation writes a record with an identifier *before* its result is returned. Nothing else can mint one — the whole trust boundary rests on that ordering (`A1.2`).
- **The tool registry.** One registry, two exposures: natively to the Mode A loop, and over MCP to external agents (`A2.2`, `AI.5`). A tool written twice means the abstraction has broken. Each declares whether it has side effects, so the trust tier can gate it whoever called it.
- **The session artifact.** `binmap.json` is the stable contract carrying findings, evidence, gate results and target metadata, versioned from the first release, with export and import and a redaction pass on export (`F0.7`, `U13`).
- **The headless harness.** Every phase's acceptance criterion is measured through `binmap-eval`, never through the interface (`A1.3`).
- **The environment probe.** Feature-detects every external tool and kernel setting, reports what is missing with the exact command that fixes it, and never assumes a capability it has not confirmed (`U0.4`, `U10`).
- **The streaming event model.** Findings appear as they are discovered, not in a batch at the end, and every long run is cancellable and keeps what it has measured (`U0.5`). A repaint is an explicit notification; forgetting it produces an interface that silently stops updating, and that is the most common framework bug.

## 4. The interface, as designed

The design is built and reviewable: every surface below exists in the Binmap v3 design file. This table is the contract between that design and the requirements, and the phase column is where each surface is implemented.

| Surface | As designed | Requirements | Phase |
|---|---|---|---|
| First run | Four steps: no project open with recents, environment review, target and benchmark configuration, reasoner choice | U0.1 U0.4 AI.1 | 0 |
| Title bar | Crate, commit and dirty flag; trust tier control; active reasoner with egress badge; palette; export; window controls | U9 AI.9 | 0 |
| Nav rail | Target, Size, Tune, Failure, Perf, Load, Replay, Agent, Environment, with finding counts. Entries absent when the target lacks the capability | U0.1 | 0, 0.5 |
| Target view | Targets grouped by language, each with its capability line in plain words; run history; restored-session notice | U1 F0.1 F0.7 | 0, 0.5 |
| Profile Lab | Pareto scatter with the frontier derived, not flagged; configuration table including rejected candidates; selected flags, gates and apply gated by tier | U3 U0.2 F0.6 | 0 |
| Size Explorer | Metric row, treemap with grouping control and drill-down, unit table, diff mode; raw, gzip and brotli switch on compressed targets | U2 U1.1 U1.2 F1.5 | 1, 1.5 |
| Failure view | Stack with inlined frames marked; source pane with reconstructed values annotated and marked when derived; disassembly pane; pane switch; binary-to-source verification badge | U5 U2.1 U2.2 U2.3 | 2 |
| Perf view | Inlining-aware flamegraph, hot symbol table, attributed source, PMU metrics, difference against a baseline commit | U7 U3.1 F3.3 F3.4 | 3 |
| Load view | Web only and opt-in: throttle profiles, phase breakdown with interquartile spread, bytes-to-milliseconds per chunk, the noise floor beside every number, gate switch off by default | POLYGLOT §7.3 | 1.5 |
| Replay view | Two-lane track with keyframes and watchpoint hits, reverse-execution controls, values re-derived per position with find-the-write, watchpoint list stating the four-register limit | U8 U4.1 F4.1–F4.5 | 4 |
| Agent panel | Transcript of hypothesis, tool call, result and revision; step budget; cost meter; gate tally with the rejected claim named; external-origin badge on ACP claims | U6 U1.3 AI.9 AI.10 | 1, 2 |
| Findings Inspector | Always present. Hypothesis, Evidence and Proposal tabs; each evidence row carries tool, arguments, digest and verbatim output; proposal diff, verify, apply | U4 U0.3 AI.6 AI.7 | 0 |
| Environment panel | Grouped probes per target family and for replay, each with its exact fix and a re-check action | U10 U0.4 | 0 |
| Command palette | Analyses, symbols and session actions, with shortcuts. Keyboard reaches every action | U12 (Must) | 0 |
| Reasoner picker | Local runners, keyed providers and ACP agents in one list, newest models first, unavailable models disabled with the reason, install command shown when an agent is missing | AI.1 AI.2 AI.3 AI.4 | 1, 2 |
| Provider keys | Keys held in the OS keyring and shown only as fingerprints; a stored key cannot be read back; per-provider connection test | AI.8 | 1 |
| Tier and apply dialogs | The four tiers with what each may do; applying above the current tier asks for the raise, names the gates and states what it will write | U9 A2.4 | 0, 2 |

### 4.1 Gaps between the design and the requirements

Six requirements have no screen in the design yet. Each is scheduled rather than assumed.

- `U13` Import. Export with redaction is designed; loading a `binmap.json` back has no entry point. Phase 0, with the export path.
- `U11` Light theme. Tokens exist for it; the application has no switch. Phase 0, alongside the theme system.
- `AI.8` The per-project cloud prohibition. The keyring half is designed; an enforceable project setting is not. Phase 1, with the provider table.
- `A2.5` `AI.12` Registry-based agent discovery. The design lists adapters with install commands, which is the hardcoded version the requirement rejects. Phase 2.
- `F0.8` Resumable sweeps. Cancellation is designed and retains results; resuming a cancelled sweep has no surface. Phase 0.
- `U2` Zoom. The treemap drills into a region; continuous zoom and pan are not designed. Phase 1, when squarification is written.

Two places where the design goes beyond the requirements, both kept: the Load view's explicit refusal to be a default gate, and the environment panel's re-check action, which makes a missing tool recoverable without restarting the application.

## 5. Phase plan

Nine blocks, each shipping a usable application. Nothing from the next block starts until the current one has shipped. Interface weeks are inside each estimate, not additional to it.

### Phase 0 — Config autotuner and the application shell

8–10 weeks · binary expertise: minimal · shell ~3 weeks, after the engine passes the headless harness

The build-profile search space, measured on the user's actual crate. Deliberately no custom painting: use the built-in chart, prove the facade trait and the streaming event model, and learn the framework on easy views.

- `F0.1` Detect the cargo project or workspace; enumerate targets and profiles.
- `F0.2` Sweep a configurable matrix over opt-level, lto, codegen-units, panic, strip, debug and overflow-checks. `F0.3` Optional extended sweep: build-std with panic_immediate_abort, and target-cpu.
- `F0.4` Per configuration, measure total and per-section size, build time and test outcome. `F0.5` Measure runtime through the user's benchmark command, with the machine's noise floor established first.
- `F0.6` Derive the Pareto frontier over size, runtime and build time.
- `F0.7` Session persistence with artifact export and import. `F0.8` Build caching, resumable and cancellable sweeps, a parallelism cap, and a separate target directory so the user's own cache is never disturbed.
- `U0.1` Application shell: window, theme, dock layout, project view. `U0.2` Profile Lab with per-point flags and apply-to-Cargo.toml gated by tier.
- `U0.3` Findings Inspector with provenance badges. `U0.4` Environment panel with exact fixes. `U0.5` Streaming progress; nothing blocks, everything cancellable.
- `U9` `U11` `U12` `U13` Trust tier control, both themes, the command palette, and session export with redaction plus import. The model layer is a null backend only: the registry and evidence store exist and nothing calls a model.

**Acceptance.** On a five-crate reference corpus, a user opens the application, selects a crate, runs a sweep, and sees a configuration reducing size by at least 25% against default release with tests passing — without touching a terminal. **Exit criteria:** the artifact schema is versioned, the five gates run, and the headless harness reproduces every number the interface shows.

### Phase 0.5 — Capability model and the multi-target shell

2–3 weeks · a refactor, taken early on purpose

- Two traits become four; locations go artifact-neutral; Finding and Evidence lose their native-specific assumptions.
- Per-target capability declarations, read by the nav rail, the empty states and the blocked-action copy.
- Target groups in the project view, each target stating its capabilities in plain words. Doing this after Phase 1 costs considerably more than doing it now.

### Phase 1 — Size attribution, monomorphization, and the first model

9–12 weeks · binary expertise: ELF symbol tables, v0 demangling · treemap ~3 weeks, Agent panel ~2 weeks

- `F1.1` Parse the ELF symbol table, with a v0 demangling parser of our own. `F1.2` Attribute bytes to crate, module and generic function, cross-checked against an independent tool to a stated tolerance.
- `F1.3` Group instantiations of the same generic; report count and aggregate cost. `F1.4` Identify the recurring size drivers: formatting machinery, panic strings, unwinding tables, Drop glue, vtables, static data.
- `F1.5` Diff two binaries, including generic-argument renames. `F1.6` Map symbols to source spans through DWARF.
- `A1.1` The Mode A loop against one local, OpenAI-compatible provider. `A1.2` Tool registry, evidence store and finding gate. `A1.3` The eval harness driving the loop headlessly.
- `A1.4` Remaining providers as table entries. `A1.5` Propose collapse strategies, generate a patch, and verify it through §6.
- `U1.1` Virtualized symbol table first, then `U1.2` the treemap with diff mode — the table is a genuine fallback if squarification and hit testing run long. `U1.3` Agent panel with reasoner picker, transcript and cost, built against Mode A only, its event enum shaped to match ACP so Phase 2 reuses the same transcript.

**Acceptance.** The top three generics by aggregate cost are identified correctly on a corpus crate with known bloat, at least one proposed patch reduces size by 5% or more with tests passing, and the grounding rate is 1.0.

### Phase 1.5 — The TypeScript backend

5–7 weeks · no DWARF, no unwinding, no disassembly · reuses every view Phase 1 built

- Bundler metadata through the Node bridge where it exists, source-map attribution in native Rust as the always-available fallback, with the attribution tier reported alongside the result.
- Size in three numbers — raw, gzip, brotli — with the compression settings recorded and exposed, because a claim under brotli 11 served at quality 4 is a wrong number delivered confidently.
- Per-chunk attribution with the initial-load path marked, and the headline number defaulting to initial-load bytes rather than the sum of everything.
- Deterministic findings with no model involvement: duplicate dependencies, wholesale locale imports, barrel imports that defeat tree shaking, polyfills traced back to browserslist entries.
- Bundler sweep over target level, minifier, tree shaking, code splitting, source-map mode and browserslist, flagging where the raw and compressed rankings disagree.
- Load-time metrics, opt-in and never a default gate. Remaining model providers land here too, as table entries.

### Phase 2 — Crash analysis and external agents

11–14 weeks · this is where DWARF is learned properly · debugger shell ~4 weeks, permission prompts ~1 week

- `F2.1` Load ELF core dumps and correlate with binary and source. `F2.2` Verify correspondence and refuse or warn loudly on a stale build. Compute the load bias once and subtract it before every lookup.
- `F2.3` Unwind including inlined frames. `F2.4` Reconstruct locals from location lists, including piecewise locations. `F2.5` Recover optimized-out values through the disassembly, marked derived, with the recovery path shown.
- `F2.6` Rust-aware panic handling: the panicking monomorphization and the decoded panic location. `F2.8` Classify the crash type.
- `A2.1` ACP client: initialize, session creation, prompting, update streaming. `A2.2` The MCP server exposing the tool registry. `A2.3` Claude Code and Codex CLI supported and tested.
- `A2.4` Permission requests mapped to trust tiers, filesystem scoped to the project root. `A2.5` Registry-based agent discovery and install.
- `U2.1` Stack pane with nested inline frames, source pane, disassembly pane. `U2.2` Scroll sync through the line table. `U2.3` Reconstructed values inline at the faulting line, marked when derived.

**Acceptance.** On 20 crashes with known ground truth the correct line is in the top three for at least 70%, with the deterministic layer alone symbolizing at least 95% of frames. **Exit criteria:** the airlock rejects an external claim citing evidence never issued, and the rejection rate is measured — a high rate means the tool descriptions are unclear, not that the agent is bad.

### Phase 2.5 — C# NativeAOT

4–6 weeks · a native-backend variant · spike DWARF quality from the compiler before committing

- Size from the compiler's own accounting file rather than the symbol table: full managed names, no demangling.
- The dependency-graph reader, which answers why an item survived trimming — a question no native backend can answer. Ship it in the first C# release; it is the differentiator, not a nice-to-have.
- The compiler configuration sweep: trimming mode, optimization preference, invariant globalization, resource-key substitution, event-source and stack-trace support, reflection metadata. A rich and genuinely unmeasured search space.
- Value-type generic instantiation grouping, which applies here for the same reason it applies to Rust.
- The .NET bridge, spawned on demand and dying with the session. Both fragile inputs sit behind a capability probe that degrades to plain symbol attribution and reports that it did.

### Phase 3 — Performance attribution

7–9 weeks · depends on Phase 2 symbolization · flamegraph ~2 weeks

- `F3.1` Drive sampling and ingest samples. `F3.2` Attribute them to source lines through the same symbolization layer the crash analyzer uses.
- `F3.3` Inlining-aware flamegraph showing logical call structure, with inline frames visually distinct. `U3.1` is that view, source-linked.
- `F3.4` Differential profiling between builds or commits. `F3.5` PMU counters: cache misses, branch mispredicts, IPC, stalls.
- V8 profiles for web targets, mapped through the source map, grouped by package, your-code-only on by default. Perf-specific tools, transcript replay and cost budgeting land in the model layer here.

**Acceptance.** On an injected regression of 10% or more, the responsible function is identified in the top three for at least 70% of cases.

### Phase 3.5 — C# CoreCLR

6–8 weeks

- Deployment sizing at assembly and type level, since a JIT deployment has no single compiled artifact: measure what actually ships.
- Trimming analysis reporting what the linker could not remove, naming the reflection site responsible.
- The AOT-migration question answered by measurement: framework-dependent, self-contained, trimmed, ReadyToRun, single-file and NativeAOT as points in one configuration space, each built, measured and tested.
- Managed traces reduced to a weighted call tree and attributed through the Portable PDB, so the flamegraph view works unchanged.

### Phase 4 — Replay debugging

open-ended · Rust and native only · timeline ~3 weeks · buildable only once 0 through 3 exist

- `F4.1` Record and manage traces, with the recording constraints stated before a user's first failed attempt rather than after.
- `F4.2` Reverse execution — reverse-continue, reverse-step, reverse-finish — driven over the debugger's remote protocol. `F4.3` Hardware watchpoints with reverse search, answering who wrote this value; four are available on x86-64 and the interface says so rather than silently single-stepping.
- `F4.4` Timeline scrubbing with every pane wired to the replay position, which is `U4.1` and `U8`. `F4.5` Every displayed value re-derived from the trace on each replay rather than remembered.
- `A4.1` Step-budgeted root-cause search over a trace, with the full transcript inspectable.

**Acceptance.** On 20 injected bugs, the root-cause line is located within a 40-step budget for at least 60%.

## 6. The verification harness

Built in Phase 0, extended thereafter, applied to every proposal whoever produced it. A candidate failing any gate is reported as rejected with the failing gate named, and stays visible — a near miss is informative.

| Gate | Passes when | Applies |
|---|---|---|
| Builds | The build succeeds, warnings diffed against the baseline | Always |
| TestsPass | The suite passes, using the user's command where declared | Always |
| BenchmarkNotWorse | A statistically significant win, or no significant regression, against the measured noise floor | Where declared |
| SizeNotWorse | Size measured and recorded whatever the objective was | Always |
| MiriClean | Sanitizers clean over the reachable portion | When unsafe is touched |

Two rules the gates depend on. The noise floor is measured once per machine by timing one unchanged binary repeatedly, and a result inside it is reported inconclusive, never coloured as a win. And the sanitizer gap is stated in the interface: a clean run on code with substantial foreign-function calls is a much weaker statement than it sounds, and a tool implying otherwise is misleading its user about safety.

## 7. The model layer

Two modes that differ in who owns the loop. **Mode A** is our own controlled loop: we decide what the model sees, force a hypothesis before every tool call, enforce a step budget, and refuse to construct a finding that cites no evidence. **Mode B** is an inversion — the external agent is the reasoner, with its own harness, context strategy, model and billing. What we keep in Mode B is narrower but still decisive: we own the tools and we own the airlock. The honest version is that Mode B's grounding guarantee is weaker, and the interface says so rather than presenting the two as equivalent.

**Mode A before Mode B, deliberately.** Building the native loop first forces the tool registry and evidence store to be correct while we control both ends; ACP then becomes a second exposure of a working system rather than an attempt to design the trust boundary and the protocol integration at once.

| Phase | Model-layer scope |
|---|---|
| 0 | Null backend only. Registry and evidence store exist; nothing calls a model |
| 1 | Mode A, one local provider, one analysis. Finding gate live. Eval harness live |
| 1.5 | Remaining providers as table entries — cheap once the two API shapes exist |
| 2 | Mode B: ACP client, MCP server, Claude Code and Codex, permission mapping to tiers |
| 3 | Perf-specific tools, transcript replay, cost budgeting |
| 4 | Replay-search tools and the step-budgeted root-cause search over a trace |

Prompts are authored against the weakest supported local model (`AI.11`), because a prompt tuned to a frontier model degrades unpredictably below it. Tool descriptions matter more in Mode B than Mode A: in our own loop we write the system prompt, but an external agent arrives knowing nothing, and the description string is its entire briefing.

## 8. Testing and evaluation

Two separate problems. The deterministic layer needs conventional testing: unit tests, snapshot fixtures per external tool version, property tests on address-to-line round trips, and a reference corpus of five projects — an embedded crate, a generics-heavy library, an application with a deep dependency tree, a WASM target, and a multi-crate workspace. Two rules make it enforceable: **the full suite runs under the null backend, and that is the CI gate that keeps the deterministic core deterministic**; and the suite is driven headlessly, never through the interface — an engine feature that cannot be exercised without the interface is a layering violation and fails review.

Interface testing uses the framework's own test macro and a test context that simulates platform input: layout algorithms as plain unit tests, view state against a fake engine returning scripted events, interaction assertions on selection and Inspector state, and tolerance-based screenshot comparison for the custom-painted views.

| Model-layer metric | Read as |
|---|---|
| Top-1 and top-3 accuracy | Against ground truth, per analysis kind |
| Steps to answer | Loop steps in Mode A; tool calls observed at the MCP server in Mode B |
| Grounding rate | Must be 1.0 by construction. Lower is a bug in the gate, not a model failure |
| Rejection rate | Mode B only. High means unclear tool descriptions |
| Confident-wrong rate | The trust metric, and the one to optimise against |
| Cost | Mode A only; external agents bill separately |

## 9. Schedule

| Phase | Scope | Weeks | Cumulative |
|---|---|---|---|
| 0 | Config sweep, shell, Profile Lab, Findings Inspector | 8–10 | 8–10 |
| 0.5 | Capability model and multi-target shell | 2–3 | 10–13 |
| 1 | Size attribution, monomorphization, treemap, Mode A | 9–12 | 19–25 |
| 1.5 | TypeScript backend: source maps, bundle attribution, bundler sweep | 5–7 | 24–32 |
| 2 | Crash analysis and ACP | 11–14 | 35–46 |
| 2.5 | C# NativeAOT and the compiler config sweep | 4–6 | 39–52 |
| 3 | Performance attribution, then V8 profiles | 7–9 | 46–61 |
| 3.5 | C# CoreCLR: deployment sizing, trimming, AOT migration | 6–8 | 52–69 |
| 4 | Replay debugging, native only | open | — |

Fifty-two to sixty-nine weeks through Phase 3.5 at thirty hours a week, with Phase 4 open-ended. Something useful ships every six to eight weeks inside those blocks, and a phase that has not shipped does not release the next one.

## 10. Risks

| Risk | Severity | Mitigation |
|---|---|---|
| R1 Scope explosion — two products plus an application, solo | Critical | Phase gates. Nothing from the next phase starts until this one ships. The non-goals are binding |
| R2 Model confabulation about binaries | Critical | The engine computes facts; findings must cite issued evidence. Enforced in the constructor and at the airlock |
| R4 The interface is on the critical path from week one, and it is a discipline the maintainer lacks — removing the CLI removed the fallback that used to protect against this | Critical | Engine-first inside every phase; the built-in chart before any custom painting; the symbol table as a fallback for the treemap; one framework upgrade per phase, at its start |
| R3 Rejection of model tooling by the target community | High | A deterministic tool with an optional layer. No reasoner is a listed choice, not a hidden flag. Never marketed as an AI debugger |
| R5 The DWARF learning curve stalls Phase 2 | High | Phase ordering puts it third, after two shipped releases |
| R6 Solo maintainer burnout; bus factor one | High | Ruthless phase scoping, a public roadmap, and good-first-issues from day one |
| T1 Fragile external formats — compiler accounting files, bundler stats, DWARF from unfamiliar producers | High | Snapshot fixtures per version, capability probes at environment time, and clean degradation to a weaker attribution tier that is reported as such |
| P4 Platform assumption — Linux x86-64 is required only by Phase 4, yet a C# and TypeScript audience is largely on Windows and macOS | Medium | The most under-examined decision in the plan. Revisit before Phase 1.5 ships to non-Rust users; the framework supports macOS today |

## 11. Open questions

1. **Does a core-dump launcher narrow N8 acceptably?** Crash analysis is the use case most likely to start in a terminal. A minimal command that opens the application on a given core file is a launcher, not a command-line product, and may be worth the exception.
2. **Do external agents cope with a large domain-specific tool set** they have never encountered? Their training is oriented to file editing and shell commands. The rejection rate in Phase 2 is the measurement that answers this.
3. **Does the code editor component expose enough decoration API** for inline variable values at the faulting line? Phase 2's design depends on it.
4. **How large can the virtualized table go in practice?** Documented for very large row counts; confirm on a real symbol table before committing the Phase 1 design.
5. **Accessibility.** The framework's screen-reader story is weaker than the web's. Decide whether that is an accepted v1 limitation and say so publicly rather than leaving it unstated.

## 12. Non-goals

Recording these matters more than recording the goals, because a solo project dies from scope creep. `N1` not an editor or a language server; `N2` not a source-only linter, since anything the existing linter finds is not this tool's job; `N3` not cross-platform in v1; `N4` not a replacement for an interactive debugger, which it drives rather than replaces; `N5` not a hosted service, with no accounts, no server and no telemetry; `N6` not C/C++ in v1, though the architecture must not preclude it; `N7` no autonomous commits without the trust tier deliberately raised; `N8` not a command-line tool.

Languages beyond Rust, TypeScript and C# are not on the roadmap, which is `N6` generalised. The trait seams mean none is precluded, and that is the only accommodation they get: adding a language is a phase of work, not a weekend, and scope is the risk most likely to end this project.
