# Phase 0 audit — raw findings

162 findings across 12 dimensions. Severities: major 76, minor 42, blocker 33, nit 11

Adversarial refutation was still running when this snapshot was taken, so these are
UNVERIFIED except where a regression test in the tree proves one.

## blocker (33)

- **Export redacts evidence but keeps the old digest, so importing an exported artifact refuses every evidence record and drops every finding as ungrounded**
  - `crates/binmap-session/src/store/mod.rs`:83
  - requirement: F0.7 (PRD line 201; plan §4 line 151 "session persistence with artifact export and import"; U13)
  - The crate doc (lib.rs:11-14) states the artifact is "something a user sends to a colleague or attaches to a bug report". As implemented, redaction and import re-validation are mutually exclusive: the colleague opens the file and sees zero findings and a refusal notice claiming their evidence was tampered with. Export is therefore only usable as an inert JSON dump, never as an importable session — which is exactly the round trip F0.7 asks for.

- **No production code path ever writes a SessionArtifact; SessionStore::save and load have zero callers outside their own unit tests**
  - `crates/binmap/src/main.rs`:42
  - requirement: F0.7 (plan §4 line 151); Phase 0 exit criterion "the artifact schema is versioned" — versioned but never written
  - F0.7 is satisfied only as a library. A user who runs a sweep and closes the app loses the run: findings, evidence, gate reports and sweep state all live in `Inner`'s mutexes and are dropped with the process. The Target view is specified to show "run history; restored-session notice" (plan line 109) — the engine has nothing for it to restore from, so this is an engine gap, not a GUI gap.

- **U13 import has no entry point at all: SessionStore::read and load are called only from binmap-session's own tests**
  - `crates/binmap-session/src/store/mod.rs`:62
  - requirement: U13 (plan line 128: "Import. Export with redaction is designed; loading a `binmap.json` back has no entry point. Phase 0, with the export path.")
  - The plan explicitly scheduled the import entry point for Phase 0 alongside export. Nothing — not the app, not the headless harness that is supposed to reproduce every number the interface shows — can load a binmap.json. The Phase 0 exit criterion that "the headless harness reproduces every number the interface shows" cannot be checked against a saved session because neither side can produce or consume one.

- **The Builds and TestsPass gates run a fixed default-release command with no candidate flags and no --target-dir, so every configuration is gated on the same default build inside the user's own target directory**
  - `crates/binmap-verify/src/lib.rs`:248
  - requirement: F0.8 "a separate target directory so the user's own cache is never disturbed" (plan §4 line 151); Phase 0 acceptance "...with tests passing"
  - Three consequences at once. (1) F0.8's separate-target-directory promise is broken for the gate commands: the sweep does disturb the user's cache, repeatedly, on every configuration. (2) The Builds and TestsPass verdicts are about default release, not about the candidate — a configuration whose flags do not compile, or whose `panic=abort`/`build-std` breaks the suite, still passes both gates. (3) The Phase 0 acceptance criterion selects the smallest configuration where `report.passed()` (binmap-eval/src/main.rs:246), so "25% smaller with tests passing" is currently 25% smaller with the *default build's* tests passing. Answering the audit's efficiency question directly: no, configurations are not built twice — but only because the second build is the wrong build, cached because it is identical for all N configurations.

- **SweepState lives only in an in-memory BTreeMap and is never written anywhere, so no sweep can be resumed after the process exits**
  - `crates/binmap-build/src/engine/mod.rs`:50
  - requirement: F0.8 "resumable ... sweeps" (plan §4 line 151; plan line 132: "Resumable sweeps. Cancellation is designed and retains results; resuming a cancelled sweep has no surface. Phase 0.")
  - Resume works only within one process lifetime, which is the case that matters least. The whole point of F0.8's resumability — cancelling an afternoon's ninety-six builds, quitting, and picking it up tomorrow — is unimplemented, and the doc comment on SweepState asserts the opposite of what the code does. The `RunRecord` carrier in the artifact (session/artifact.rs:72-84) and `adopt_run` were built for exactly this and are left unconnected at both ends.

- **There is no CI configuration anywhere, so §8's mandated CI gate ("the full suite runs under the null backend") does not exist — and neither does a null backend**
  - `Cargo.toml`:1
  - requirement: plan §8: "the full suite runs under the null backend, and that is the CI gate that keeps the deterministic core deterministic"; DESIGN.md:337
  - The one rule §8 names as the thing that makes the test suite enforceable is entirely unimplemented. 144 tests that no machine other than the author's ever runs are 144 tests that will rot silently; and with no null backend there is nothing to gate on when the model layer arrives.

- **Nothing in the test suite measures Phase 0's acceptance criterion or the exit criterion — binmap-eval has zero tests and is never invoked by `cargo test`**
  - `crates/binmap-eval/src/main.rs`:234
  - requirement: Phase 0 acceptance criterion; exit criterion "the headless harness reproduces every number the interface shows"; A1.3
  - The acceptance criterion is only checkable by a human remembering to type `binmap-eval acceptance` by hand. A regression that takes the best reduction from 30% to 5% turns the whole suite green. Likewise the exit criterion — no test anywhere compares a number produced by the engine against the same number as AppState would render it, so "the harness reproduces every number the interface shows" is asserted by nobody.

- **The reference corpus is one trivial no-dependency binary instead of the five projects §8 specifies, and no test references it at all**
  - `corpus/tiny/Cargo.toml`:1
  - requirement: plan §8: "a reference corpus of five projects — an embedded crate, a generics-heavy library, an application with a deep dependency tree, a WASM target, and a multi-crate workspace"
  - Four of the five required kinds are missing, and the one present matches none of them — a dependency-free binary exercises neither deep dependency graphs, nor generic instantiation, nor build-std/WASM, nor multi-crate target enumeration. The acceptance criterion is written against a corpus, so as it stands it cannot be evaluated as written even by hand.

- **Every gate runs one configuration-independent GatePlan, so `cargo test` never runs under the candidate profile — and every sweep test uses `shell("true")`, so no test can see it**
  - `crates/binmap-build/src/sweep/mod.rs`:415
  - requirement: Phase 0 acceptance: "...at least 25% smaller than default release **with tests passing**"; F0.5
  - This is the "sweep silently measuring the wrong thing" case, and the suite is blind to it by construction. binmap-eval's acceptance path filters on `measured.report.passed()`, so a configuration can be reported as passing tests when the tests were compiled and run under the default profile — panic=abort, strip, opt-level=z were never exercised by the suite that supposedly cleared them. A test with a fake build system that records the env each gate command saw would catch it; none exists.

- **F0.3's build-std axis can never succeed: the build shells out to plain `cargo`, so `-Zbuild-std` is rejected on any stable default toolchain, and the nightly probe is never consulted before the axis is expanded**
  - `crates/binmap-build/src/cargo.rs`:61
  - requirement: F0.3; TOOLING §3.3 ("Detect the toolchain and mark nightly-only axes clearly in the UI rather than failing mysteriously on stable")
  - This is exactly the failure mode TOOLING §3.3 and the probe's own doc comment (environment/mod.rs:122-124, "the failure mode otherwise is ninety-six builds that all fail the same way an hour in") were written to prevent. Sweep::measure_one (sweep/mod.rs:390-397) records a failed build as `built: false` with no distinguishing reason, so the user watching a build-std sweep sees every extended point silently rejected rather than being told nightly is not selected.

- **`-Zbuild-std=core,alloc,panic_immediate_abort` treats a build-std *feature* as a crate name; `-Zbuild-std-features` appears nowhere and `--target` is never passed**
  - `crates/binmap-core/src/config.rs`:211
  - requirement: F0.3; TOOLING §3.3
  - The single highest-value size lever in F0.3 is spelled in a way cargo cannot act on, so even a correctly-selected nightly toolchain would not remove the panic formatting machinery. The axis would appear to run and produce a binary the same size as the non-build-std points, which is worse than failing: it would be recorded as measured evidence that build-std does not help.

- **BuildConfiguration::name() truncates values to six alphanumerics, so distinct target-cpu and build-std points collapse to the same name — which is the target directory, the resume key and the frontier point id**
  - `crates/binmap-core/src/configuration.rs`:210
  - requirement: F0.3, F0.8 (per-configuration caching and resume)
  - Two different extended configurations build into the same directory and overwrite each other's artifact, and `SweepState::remaining()` marks the second as already measured. The sweep would report a size for `x86-64-v4` that was actually produced by `x86-64-v2` — a wrong number delivered with full confidence and full evidence citations. The existing test is precisely the one that should have caught it and is scoped to the matrix where the bug cannot appear.

- **The gates run a fixed cargo invocation with no per-configuration environment, so "builds and tests pass" is asserted about default release rather than about the swept configuration**
  - `crates/binmap-build/src/sweep/mod.rs`:415
  - requirement: Phase 0 acceptance ("a configuration at least 25% smaller than default release **with tests passing**"); F0.8 (our target directory, never the user's)
  - Every configuration passes or fails the gates identically because they all gate the same default build — the acceptance criterion's "with tests passing" carries no information about the winning configuration. It is also the specific case that would bite: the default matrix sweeps `panic = abort` (config.rs:124), which the libtest harness cannot link, so the one axis most likely to be rejected by a real `cargo test` is never actually tested. Separately, the gate build writes into the user's target/, contradicting TrustTier::Observe's stated contract (config.rs:15-18).

- **The reference corpus has one project, not the five the acceptance criterion and §8 both name**
  - `corpus/tiny/Cargo.toml`:1
  - requirement: Phase 0 Acceptance (plan §5, line 156) and §8, line 277
  - The 35.4% result I measured is one data point on the easiest possible project: a single-package, single-target, zero-dependency binary whose own source comment says "Every configuration in the matrix should still build and still pass the test below". The four missing projects are precisely the ones that would exercise the code paths tiny cannot: --package selection on a workspace, a WASM target family, a deep dependency tree where lto=fat actually costs minutes, and a generics-heavy library where the frontier is not a single point. Claiming the acceptance passes on 1/5 of the corpus is claiming it passes on the sample that cannot fail.

- **Builds, NoNewWarnings and TestsPass run a fixed default-release build for every candidate, so "with tests passing" is never established for the configuration that wins**
  - `crates/binmap-build/src/sweep/mod.rs`:415
  - requirement: Phase 0 Acceptance ("with tests passing") and §6 gate table (TestsPass: "The suite passes")
  - This is the single most damaging defect to the acceptance claim. The winner my run selected — `opt-level=s lto=fat codegen-units=1 panic=abort strip=symbols` — had its size measured under those flags and its tests run without them. The harness prints "gates: All gates passed" for a configuration whose test suite was never executed. It also explains the otherwise implausible "96 passed every gate": the Builds/TestsPass columns are 96 copies of one cached default-release result, so the gates cannot discriminate between candidates at all. Every rejection the design promises to show ("a near miss is informative") is unreachable, because nothing candidate-specific can ever fail.

- **No MeasurementSource implementation exists, so BenchmarkNotWorse is always skipped, the noise floor is never measured, and F0.5 is unreachable**
  - `crates/binmap-build/src/engine/mod.rs`:58
  - requirement: Exit criteria ("the five gates run"); F0.5; §6's noise-floor rule
  - One of the six gates cannot run on any project on any machine, even with hyperfine installed — there is no code path from a declared benchmark to a BenchmarkVerdict. The exit criterion "the gates run" is therefore false as stated. It also strands the whole §6 noise-floor doctrine: `NoiseFloor`, `timing::compare`, `BenchmarkVerdict::Inconclusive` and the "never coloured as a win" rule are all written, tested and unreachable. The designed Profile Lab needs these numbers directly (see the frontier finding).

- **The acceptance number is produced solely by binmap-eval; the shipped binary has no sweep path, so "without touching a terminal" is unmet**
  - `crates/binmap/src/main.rs`:52
  - requirement: Phase 0 Acceptance ("a user opens the application, selects a crate, runs a sweep, and sees a configuration ... without touching a terminal")
  - Recorded because the audit dimension is the acceptance sentence taken literally, not to re-report the known GUI gap: the missing views are the next piece of work. The point is what that implies about the claim. Four of the sentence's six clauses ("opens the application", "selects a crate", "runs a sweep", "sees a configuration") currently resolve to the dev harness, so what has been demonstrated is that the *engine* finds a 25%+ configuration on one crate — a real and non-trivial result — not that the acceptance criterion passes. Until the Tune view exists, the honest status line for Phase 0 is "engine measured, criterion not yet exercised", and the criterion's last clause is the reason.

- **`Finding::new` is documented as "the only way to make a finding" but every field is `pub` and the struct derives `Deserialize`, so any crate can build an ungrounded finding by struct literal or from JSON**
  - `crates/binmap-core/src/finding.rs`:196
  - requirement: R2 (plan §10 risk table, plan §2.6); finding.rs module doc "the constructor is the thing that makes 'non-empty' and 'actually issued' true rather than merely intended (R2)"
  - The whole trust boundary is stated as an invariant the constructor enforces. It is enforced only against callers who choose to use the constructor. Phase 1's model layer and Phase 2's ACP airlock are both built on the assumption that a `Finding` value in hand has been through the gate; today a `Finding` value proves nothing. The fix is small now (private fields + a `#[serde(try_from = ...)]` or an unvalidated `RawFinding` that import must convert) and expensive after five crates hold `Finding` values.

- **The provenance ceiling is applied once at construction and the fields stay public, so clone-then-mutate raises a finding to Certain and empties its evidence**
  - `crates/binmap-core/src/finding.rs`:314
  - requirement: Plan §2.6 ceiling table; finding.rs:58-60 "A finding claiming more than its provenance allows is clamped by Finding::new"
  - An external agent's `Probable` guess becomes a `Certain` measured-looking claim with a two-line edit anywhere downstream, and the interface renders confidence verbatim. This is exactly the confabulation R2 exists to prevent, and it needs no malice — a view that normalizes or a future `Finding::with_confidence` helper does it by accident.

- **`EvidenceStore::adopt` mints identifiers outside `begin`, and the digest covers only `output` — not the id, not the invocation — so a forged record enters a live store and passes the airlock**
  - `crates/binmap-core/src/evidence.rs`:220
  - requirement: Plan §3 "Every tool invocation writes a record with an identifier before its result is returned. Nothing else can mint one" (A1.2); evidence.rs:24-30 "There is no public constructor... an identifier that exists was issued by a store"
  - This defeats the airlock rather than going around it: the forged id is genuinely issued by the store, so `store.issued()` says yes and `Finding::new` accepts it. The Evidence tab then shows a command line that was never run beside output that really was produced by something else — the single worst failure mode for a tool whose pitch is "you can retype this command by hand". Two independent fixes are needed: make `adopt` a distinct namespace or private to import, and digest the invocation and id along with the output.

- **The Builds and TestsPass gates run a fixed default-release command, not the candidate configuration, so every candidate in a sweep is gated on the baseline build**
  - `crates/binmap-build/src/sweep/mod.rs`:415
  - requirement: plan §6 ("A candidate failing any gate is reported as rejected with the failing gate named") and F0.4
  - Builds and TestsPass produce identical results for all 96 configurations because they compile and test the unmodified default profile. A configuration that does not compile (build-std, an unsupported flag combination) still shows `Builds: pass`; because its artifact is absent, `size_bytes` is None and `gate_size` (verify/lib.rs:317-322) rejects it as `Rejected by SizeNotWorse — size was not measured`, naming the wrong gate. The design's own z-buildstd case — panic=abort making three `#[should_panic]` tests fail — is undetectable, since the test gate never runs under the candidate's profile. The gate commands also carry no `--target-dir`, so they build and test into the user's own `target/`, contradicting the invariant cargo.rs:19-22 and its test `a_sweep_never_touches_the_users_target_directory` (cargo.rs:239) assert.

- **baseline_warnings is never populated outside tests, so NoNewWarnings is judged against zero — the exact comparison its own doc forbids**
  - `crates/binmap-verify/src/lib.rs`:284
  - requirement: plan §6 table row Builds ("warnings diffed against the baseline"); Phase 0 acceptance
  - Any real crate that already emits warnings fails NoNewWarnings on every candidate, since the gate build is the same build as the baseline and emits the same N > 0 warnings against a stored baseline of 0. `rejected_by()` then returns NoNewWarnings for the whole matrix, `report.passed()` is false everywhere, `point()` marks every point ineligible (sweep/mod.rs:97-99), the frontier is empty, and the sweep reports "0 passed every gate". Phase 0's acceptance run cannot succeed on any corpus crate with a pre-existing warning.

- **The sweep never attaches a BenchmarkVerdict to the candidate and only writes back a Regressed one, so BenchmarkNotWorse always reports "skipped — no benchmark is declared" and Inconclusive is unreachable**
  - `crates/binmap-build/src/sweep/mod.rs`:479
  - requirement: plan §6 ("a result inside it is reported inconclusive, never coloured as a win"); design literal g("BenchmarkNotWorse { significance: 0.05 }", "inconclusive", "+0.6% n.s.", "inside the noise floor (0.9%)") at docs/design/Binmap v3.dc.html:1912
  - A configuration whose timing difference is inside the machine's noise floor gets `runtime_nanos` recorded, which `point()` feeds to the Pareto comparison as a real objective (sweep/mod.rs:91-93, pareto.rs:85), so it can win the frontier and be presented as the recommendation — while its gate row says only "skipped: no benchmark is declared". That is precisely a noise-floor result coloured as a win. The Inconclusive variant, its `GateResult::Inconclusive`, `report.inconclusive()` and the "Passed, with N inconclusive" summary are all dead in the shipping path; the designed inconclusive cell can never be rendered.

- **F0.5 is unmet end to end: nothing implements MeasurementSource, so hyperfine is never invoked and runtime is never measured**
  - `crates/binmap-core/src/traits.rs`:110
  - requirement: F0.5
  - Every runtime path in the codebase is conditional on `self.benchmark` being `Some`, and nothing can ever make it `Some`. `Sweep::run` step 3 (sweep/mod.rs:235) is dead, `measure_baseline`'s noise-floor block (308-324) is dead, `benchmark_survivors` (438) is dead, and `timing::compare` is exercised only by unit tests. So runtime is never measured, the BenchmarkNotWorse gate always reports "no benchmark is declared", the Pareto frontier is really two-dimensional, and TOOLING §4.2's "parse hyperfine's JSON" is satisfied only by a parser nothing calls. The wiring from ProjectConfig.benchmark to a hyperfine-backed source is the missing piece; the pieces on either side of it are written.

- **F0.4's per-section size is measured and then thrown away — MeasuredConfiguration keeps only the total**
  - `crates/binmap-build/src/sweep/mod.rs`:399
  - requirement: F0.4
  - F0.4 is literally "measure total AND per-section size" per configuration. The section table survives only as free-form text inside one evidence record (size.rs:28-33), which no view can render and no comparison can use: a user cannot see that opt-level=z bought its win out of .text while .eh_frame stayed, and the sweep cannot diff sections between two configurations. It also strands the bloaty oracle — `bloaty::disagreement` (bloaty.rs:57) takes an `&ArtifactSize` with sections, so no swept artifact can ever be cross-checked, only the unit test's own test binary.

- **The per-configuration test outcome is produced by a build and test run under the default profile, not under the configuration being measured**
  - `crates/binmap-build/src/sweep/mod.rs`:415
  - requirement: F0.4 ("test outcome" per configuration); Phase 0 acceptance ("with tests passing")
  - Every configuration receives the same TestsPass verdict — the default release profile's — so panic=abort, opt-level=z, build-std with panic_immediate_abort and overflow-checks=false are never actually tested, and `report.passed()` (which decides frontier eligibility at sweep/mod.rs:97-99 and the acceptance number in eval/main.rs:246) certifies something that was not run. The comment at sweep/mod.rs:411-413 states the opposite — "the build gate re-runs the build under the same arguments, which cargo answers from its own cache" — but the arguments differ and the target directory differs, so it is a full extra default-profile build per configuration, written into the user's own `target/` that F0.8 promises not to disturb.

- **GatePlan::with_env is never called, so every configuration is gated by a plain `cargo build --release` in the user's own target directory rather than by the configuration under test**
  - `crates/binmap-build/src/sweep/mod.rs`:415
  - requirement: F0.2 / F0.4 (per-configuration test outcome); binmap-build crate contract, crates/binmap-build/src/lib.rs:5-8 ("nothing writes to the user's target directory")
  - Every one of the 96 rows gets its Builds/TestsPass/NoNewWarnings verdict from the same default-release build, so the gate column is identical for all configurations and says nothing about the configuration it is printed beside. The acceptance criterion ("at least 25% smaller with tests passing") is measured against tests that never ran under the winning configuration — a wrong number delivered confidently. It also writes into the user's own `target/` on every candidate, breaking the F0.8 promise the crate docs make. The comment at sweep/mod.rs:412-414 asserting the gate "re-runs the build under the same arguments" is false.

- **No type in the workspace implements MeasurementSource, so the benchmark is always None and F0.5 (runtime + noise floor) never executes**
  - `crates/binmap-core/src/traits.rs`:110
  - requirement: F0.5 — "Measure runtime through the user's benchmark command, with the machine's noise floor established first"
  - The whole runtime axis is inert: `SweepState.noise_floor` is always None, `MeasuredConfiguration.runtime_nanos` is hard-coded None at sweep/mod.rs:431, `benchmark_survivors` returns immediately at sweep/mod.rs:444, and Gate::BenchmarkNotWorse is permanently skipped ("no benchmark is declared", verify/lib.rs:336-338). The §6 rule the binmap-measure crate exists to enforce — never colour a result inside the noise floor as a win — is unenforceable because no noise floor is ever measured.

- **The Engine trait exposes no per-configuration measurements, so the Profile Lab's table, scatter and noise-floor line have no source the interface may read**
  - `crates/binmap-core/src/facade.rs`:149
  - requirement: Profile Lab (plan §4, phase 0) — U3, U0.2, F0.6; design configColumns FLAGS/SIZE/Δ SIZE/RUNTIME/BUILD/GATES, paretoPoints {x: rtNum, y: sizeKb, size: buildN, frontier, failed}, selConfig.gates, "noise floor on this machine: 0.9%"
  - This is the whole of Phase 0's headline view. No column but Δ SIZE can be filled, the scatter has no axes, the Selected-configuration panel has no gate rows, and the frontier cannot be distinguished from dominated points — unless a Phase-0 GUI either breaks the §2.4 dependency boundary or re-parses opaque JSON it is documented as not understanding.

- **hyperfine is wrapped but never called: no MeasurementSource implementation exists in the workspace, so runtime is never measured**
  - `crates/binmap-build/src/engine/mod.rs`:58
  - requirement: TOOLING §4.2 (hyperfine for command-level benchmarks, JSON parsed never the table); plan §5 F0.5
  - F0.5 ("measure runtime through the user's benchmark command, with the machine's noise floor established first") cannot execute. `Sweep::measure_baseline` skips the noise floor (`if let Some(source) = self.benchmark`, sweep/mod.rs:308), `benchmark_survivors` returns immediately (sweep/mod.rs:444), `runtime_nanos` is always None, and `Gate::BenchmarkNotWorse` is permanently skipped with "no benchmark is declared". The designed Profile Lab makes runtime the X axis of the Pareto scatter and gives every configuration row an `rt`/`rtDelta` column (Binmap v3.dc.html:1539, 1719-1724); the engine can never populate either. The hyperfine wrapper is well-built and entirely dead.

- **The verification gates run without the candidate configuration's profile environment, so every gate verdict is about default release**
  - `crates/binmap-build/src/sweep/mod.rs`:415
  - requirement: TOOLING §3.2 (profile env vars are the primary mechanism for putting a configuration into effect)
  - The Builds and TestsPass gates for `olz-lfat-cgu1-pabort` actually compile and test plain `cargo build/test --release` in the user's own target directory. Every configuration therefore passes or fails identically, on a build that is not the one measured — and the acceptance criterion ("25% smaller with tests passing") is asserted against a test run of a different binary. The comment immediately above at sweep/mod.rs:412-414 claims the gate "re-runs the build under the same arguments, which cargo answers from its own cache", which is not true: the arguments differ and the target directory differs, so it is a second full build of the wrong thing.

- **The gate harness verifies a fixed build/test command, never the candidate configuration, so every configuration in a 96-point sweep gets identical gate rows**
  - `crates/binmap-build/src/sweep/mod.rs`:415
  - requirement: Phase 0 acceptance (§ acceptance: "a configuration at least 25% smaller than default release with tests passing"); Gate::TestsPass, Gate::Builds (crates/binmap-core/src/gate.rs:29-35)
  - The winning configuration is never actually tested under its own profile settings, so the headline claim "25% smaller with tests passing" is unsupported — panic=abort or build-std configurations that break the suite would still pass. Worse, the failure attribution is wrong in the interface: a configuration that did not compile shows "Builds: Passed" with a fabricated duration and is blamed on SizeNotWorse. It also runs the user's whole `cargo test --release` once per configuration in the user's own target directory, contradicting `a_sweep_never_touches_the_users_target_directory` (crates/binmap-build/src/cargo.rs:236).

- **An exported session artifact loses every finding and every evidence record when it is imported, because export redacts the output but keeps the old digest**
  - `crates/binmap-session/src/store/mod.rs`:95
  - requirement: U13 (export/import a session a colleague can read); binmap-session/src/lib.rs module doc: "Export redacts… A session artifact is something a user sends to a colleague"
  - The whole product is the finding list, and the file a user attaches to a bug report arrives at the other end empty with a notice that all its evidence is tampered. The existing test `an_exported_artifact_admits_that_its_evidence_was_altered` asserts the digest mismatch but never asserts the findings survive, so this passes CI while U13 is non-functional.

## major (76)

- **Cancellation is checked only between configurations, and ToolRunner blocks on Command::output() with no child kill, so a cancel waits out the whole build and test suite**
  - `crates/binmap-build/src/sweep/mod.rs`:346
  - requirement: F0.8 "cancellable sweeps" (plan §4 line 151)
  - Pressing cancel on a real project means waiting through one full LTO build plus one full `cargo test --release` before anything happens — minutes to tens of minutes. For a GUI whose designed affordance is a cancel button on a running sweep, a cancel that appears to do nothing for ten minutes reads as a hang, and the user's only recourse is to kill the app, which (given the previous finding) also discards the run.

- **A resume silently skips the noise floor forever if the first attempt was cancelled after the baseline size was recorded, disabling every runtime measurement in the run**
  - `crates/binmap-build/src/sweep/mod.rs`:210
  - requirement: F0.8 resumable sweeps; F0.5 noise floor (sweep/mod.rs:8-9 "The noise floor is measured before any timing comparison")
  - The resumed sweep completes, sets `complete = true`, emits a Finished summary and produces a frontier — with every `runtime_nanos` left None and the BenchmarkNotWorse gate never consulted. Nothing in the state or the events says timing was skipped, so the user gets a silently size-only frontier from a run that looks identical to a full one. This is the failure mode the module doc-comment's own rule about unmeasured axes exists to prevent, arrived at through the resume path instead.

- **RunId minting restarts at run-0001 each process, so the first new sweep after adopting a persisted run silently resumes that run's state against a different target**
  - `crates/binmap-build/src/engine/mod.rs`:153
  - requirement: F0.8 resumable sweeps (plan §4 line 151)
  - Once F0.7 persistence is wired up as intended — load the artifact, `adopt_run` the stored SweepState, then let the user start a fresh sweep — the fresh sweep inherits the adopted run's `planned` matrix, its already-`measured` results and its `baseline_bytes`, while being handed a possibly different `Target`. The user selects crate B, runs a sweep, and sees crate A's measurements with `state.target` still naming crate A. The id space needs to be seeded from the adopted runs, or minted as something not derivable from a counter that resets.

- **Every parallel worker's Builds gate runs cargo in the same shared target directory, so the cap's workers serialize on cargo's build-directory lock and the gate's reported duration is lock-wait time**
  - `crates/binmap-build/src/sweep/mod.rs`:415
  - requirement: F0.8 "a parallelism cap" and "build caching" (plan §4 line 151)
  - The cap itself is applied correctly (sweep/mod.rs:340, `with_parallelism` clamps to >=1 at sweep/mod.rs:52), but the throughput it is supposed to buy is given back at the gate step, where N workers queue behind one lock in the user's own target dir. Separately, because that build is a cargo no-op after the first, rustc emits no diagnostics and `count_warnings` (verify/lib.rs:398-400) returns 0, so the NoNewWarnings gate passes vacuously for every configuration after the first — a green gate that checked nothing.

- **The redaction pass covers only evidence records and target.root; absolute artifact paths inside the serialized sweep state and finding text are exported verbatim**
  - `crates/binmap-session/src/store/mod.rs`:80
  - requirement: F0.7 / U13 "a redaction pass on export" (plan §4 line 95 and line 154)
  - The redaction pass reports "Nothing needed redacting" or an undercount while the file still contains the user's home directory and account name in every measured configuration's artifact path. redact.rs:1-11 frames the pass as a courtesy rather than a security boundary, which is fine — but the RedactionReport shown to the user is then wrong about its own coverage, and the user attaches the file to a bug report on that assurance.

- **`names_are_distinct_so_cached_builds_do_not_collide` only exercises the default matrix, where the two axes that can actually collide are empty**
  - `crates/binmap-core/src/configuration.rs`:292
  - requirement: F0.8 (per-configuration caching); the test's own stated purpose
  - The test names itself after the invariant it fails to check. With a target-cpu axis enabled, two configurations share one target directory — the second measures the first's binary — and `remaining()` treats the second as already done, so a resumed sweep silently skips it. Adding one case with `target_cpu: vec!["x86-64-v3".into(), "x86-64-v4".into()]` turns the test red today.

- **The bloaty cross-check — the one test TOOLING §4.1 explicitly demands — silently returns on any machine without bloaty, and bloaty is absent here**
  - `crates/binmap-measure/src/bloaty.rs`:112
  - requirement: TOOLING §4.1, quoted verbatim in the test's own doc comment: "If your total attributed bytes disagree with bloaty's by more than a small margin, your attribution is wrong. Make that a test."
  - The single test that would catch our section attribution being wrong passes by doing nothing on the machine it was written on. It should either drive `disagreement()` against a checked-in bloaty CSV fixture plus a checked-in object file, or be `#[ignore]`d with a named CI job that installs bloaty — a green `ok` for a test that ran no assertion is worse than a red one.

- **`hyperfine::measure()` has no successful-path test on any machine: the only integration test asserts the missing-tool branch and skips when hyperfine is installed**
  - `crates/binmap-measure/src/hyperfine.rs`:157
  - requirement: F0.6 (runtime as a sweep objective); TOOLING §4.2
  - The runtime objective is the axis the Pareto frontier is supposed to trade against, and the code that produces its samples is untested end to end on every configuration of the developer's machine. A wrong flag name or a swapped export path would ship green.

- **The environment probe is tested only against whatever machine runs the suite; two of its tests loop over a set that is empty on a fully-provisioned machine and pass vacuously**
  - `crates/binmap-build/src/environment/mod.rs`:326
  - requirement: U0.4, U10, TOOLING §9: "Never assume a capability that has not been confirmed" and "every warning names the exact fix"
  - The behaviour that matters — what the panel says when rustup is missing, when `cargo miri` exits non-zero, when core_pattern is piped, when perf_event_paranoid reads garbage — is decided by the developer's laptop rather than by the test. Every branch of `nightly()`, `sanitizer()`, `git()` and `core_dumps()` other than the one this machine happens to take is unreachable from the suite.

- **There are no property tests of any kind, and the address-to-line round trip §8 names has neither an implementation nor a test**
  - `crates/binmap-core/src/traits.rs`:99
  - requirement: plan §8: "property tests on address-to-line round trips"
  - Address-to-line is the mapping every later phase's source attribution rests on. Right now the trait method is a promise with nothing behind it, so the §8 requirement is not merely untested — the code it would test does not exist, and no test records that gap.

- **No snapshot fixtures for any external tool version: the bloaty and hyperfine parsers are tested against hand-typed literals that record no version**
  - `crates/binmap-measure/src/bloaty.rs`:88
  - requirement: plan §8: "snapshot fixtures per external tool version"
  - The literals are a guess at what bloaty and hyperfine emit, not a capture of what they emitted, and nothing ties them to a version. When bloaty 1.2 changes its CSV header or hyperfine renames `times`, the parsers keep passing against the frozen guess while failing against reality — which is precisely the coupling §8 asks fixtures to make visible.

- **`cargo deny check bans` fails on every internal path dependency before it ever evaluates the boundary bans, because `wildcards = "deny"` is set without `allow-wildcard-paths` and the workspace path deps carry no `version`**
  - `deny.toml`:14
  - requirement: §2.4 ("The interface may not compute", enforced by the dependency graph); TOOLING.md:330 ("The bans feature enforces DESIGN-GUI.md §4's architectural boundary")
  - The single artifact the plan relies on to keep §2.4 true cannot report a green result. A developer who runs the documented `cargo deny check bans` gets a wall of errors about internal path deps that have nothing to do with the boundary, and the cheap response is to stop running it or to delete `wildcards = "deny"` — either way the ban that actually matters stops being checked. Fix is one of: add `version = "0.1.0"` to the workspace path deps in Cargo.toml:28-33, or set `allow-wildcard-paths = true` and `publish = false` on the non-published members.

- **Nothing in the repository ever runs `cargo deny check bans`; the boundary is enforced by a comment**
  - `deny.toml`:7
  - requirement: PRD.md:442 ("Repository setup: dual license, CI on Linux x86-64, cargo-deny, rustfmt, Clippy denies"); DESIGN-GUI.md:189 ("Enforced in CI with cargo-deny's dependency bans, not by convention")
  - §2.4 is called load-bearing precisely because dropping the IPC boundary removed the structural barrier. With no CI, the replacement barrier is an unexecuted config file plus a comment in crates/binmap-gui/Cargo.toml:15 that says "enforced by review". The graph is correct today by the author's discipline, not by any mechanism, and the first violation will be found by a human or not at all.

- **deny.toml encodes the "interface may not compute" rule but not the "all GPUI usage lives in binmap-gui" rule — no ban on gpui-kit exists**
  - `deny.toml`:22
  - requirement: §2.2 rule 2 ("All GPUI usage lives in binmap-gui, and every custom widget in binmap-gui::widgets, so an API change touches one module"); DESIGN-GUI §7.2 ("Isolate the surface... No other crate imports it")
  - Both §2.2 rule 2 and §2.4 are described as load-bearing, and deny.toml is presented as the thing that makes them structural. Only one of the two is expressed. A contributor adding gpui-kit to binmap-core to share a colour type would pass every check the repo has, and the pre-1.0 upgrade cost §2.2 was written to bound would then be spread across crates.

- **F0.1 says "enumerate targets and profiles"; discovery enumerates targets only and no code anywhere reads a project's profiles**
  - `crates/binmap-build/src/project.rs`:56
  - requirement: F0.1 (PRD §, plan §4 line 147: "Detect the cargo project or workspace; enumerate targets and profiles.")
  - Half of a Must requirement is absent. A Project/Target view cannot show the user what their current release profile already sets, so the user cannot see what the sweep is departing from, and the engine cannot tell a project that already has `lto = "fat"` from one that does not — every reduction is stated against a baseline whose settings the tool never read.

- **The whole engine hardcodes the `release` profile, so a project whose shipping profile is a custom one cannot be swept and a proposal writes to the wrong profile**
  - `crates/binmap-core/src/configuration.rs`:86
  - requirement: F0.1 ("enumerate targets and profiles"), TOOLING §3.2
  - Projects that ship from `[profile.dist]` or `[profile.release-lto]` (the common pattern once anyone tunes a profile) are measured on a profile they do not ship, and `propose_configuration` (engine/mod.rs:289-306) writes the winning settings into `[profile.release]` — a diff that changes a profile the user does not build with. The parameterisation is one string away and its absence is what makes finding 1 unfixable without touching every call site.

- **Nothing in the repository can turn F0.3 on: the default matrix leaves both extended axes empty and there is no way to change the matrix of an open engine**
  - `crates/binmap-build/src/engine/mod.rs`:166
  - requirement: F0.2 ("Sweep a *configurable* matrix"), F0.3
  - F0.3 exists only as types and unit-tested helpers; no path through the shipped binary or the headless harness reaches `-Zbuild-std` or `-Ctarget-cpu`, which is why the three defects above have gone unobserved. It also means F0.2's configurability is untested end to end: the exit criterion says the headless harness reproduces every number the interface shows, and the harness cannot even express a non-default matrix.

- **Setting RUSTFLAGS for the target-cpu axis silently discards the project's own build.rustflags, so a target-cpu point differs from the baseline by more than target-cpu**
  - `crates/binmap-build/src/cargo.rs`:100
  - requirement: F0.3 (target-cpu axis)
  - The comparison the frontier is built on stops being a one-variable change: a target-cpu point is the project built *without* its own configured flags. On a project that sets `-Ctarget-feature=+crt-static` or `--cfg` gates in config.toml, that point either fails to build or measures a binary the user cannot ship, and either way the size delta is attributed to target-cpu. The fix is to append to the inherited flags (or use CARGO_ENCODED_RUSTFLAGS after reading the existing ones), not to replace them.

- **TOOLING §8's mitigation — an integration test asserting each axis actually changes the output — does not exist, and the accessor added for it has no caller**
  - `crates/binmap-build/src/cargo.rs`:105
  - requirement: TOOLING §8 fragility table, row "Cargo profile env vars" ("Integration test asserting each axis actually changes the output"); TOOLING §3.2 ("Verify the exact variable names against your toolchain version")
  - The named mitigation for a Medium-fragility dependency is absent, and its absence is what let the four F0.3 defects above ship. A test of this shape would have caught the -Z rejection on stable, the build-std spelling, and any future rename of a CARGO_PROFILE_* variable — all of which fail silently today, because a failed build is recorded as `built: false` rather than surfaced.

- **TOOLING §3.4's `-Csymbol-mangling-version=v0` is neither probed nor offered anywhere**
  - `crates/binmap-build/src/environment/mod.rs`:27
  - requirement: TOOLING §3.4, §2.4, §9
  - Phase 1's monomorphization grouping degrades to heuristic prefix matching on legacy symbols, and TOOLING §2.4 is explicit that the tool must detect the scheme and say so rather than silently producing worse results. The probe panel is built and shipping in Phase 0; adding the row later means Phase 1 starts on binaries already swept and cached under legacy mangling.

- **The gate build and test have no --target-dir, so they write into the user's own target/release that F0.8 promises never to disturb**
  - `crates/binmap-eval/src/main.rs`:103
  - requirement: F0.8 ("a separate target directory so the user's own cache is never disturbed")
  - F0.8's promise is stated as structural in cargo.rs's module doc ("a sweep of ninety-six configurations leaves the manifest exactly as it found it") and the ProjectConfig field comment, but the gate path defeats it. A sweep on a real project will invalidate the user's release cache and, worse, race: 8 concurrent workers all shell out to `cargo build --release` in the same directory, serialising on cargo's build lock. That also means the build durations the Builds gate records (verify/lib.rs:265, `format!("{:?}", output.duration)`) are lock-wait times, not build times.

- **MiriClean is unreachable twice over: the plan is never given a sanitizer command and no candidate is ever marked as touching unsafe**
  - `crates/binmap-verify/src/lib.rs`:97
  - requirement: Exit criteria ("the five gates run"); §6 gate table (MiriClean)
  - For a Phase 0 configuration sweep, "no unsafe touched" is arguably the right verdict — the design agrees, showing MiriClean skipped on every config (Binmap v3.dc.html:1716). But it is right by accident, not by decision: the field is hardcoded false rather than computed, and the sanitizer command is never wired, so the moment Phase 1 proposes a source patch the gate will still silently skip. The §6 FFI caveat machinery (verify/lib.rs:378-384) has likewise never executed.

- **With runtime never measured and build time suppressed at any parallelism above 1, F0.6's Pareto frontier degenerates to argmin(size)**
  - `crates/binmap-build/src/sweep/mod.rs`:419
  - requirement: F0.6 ("Derive the Pareto frontier over size, runtime and build time"); U0.2 / the Tune view contract
  - This is the clearest case of the engine failing to provide what a designed view needs. The Profile Lab's scatter has runtime on x, size on y and build time as circle area; today the engine can fill exactly one of the three at default settings, so the plot has nothing to plot against and the frontier the panel is built to show is one dot. It also makes F0.6 vacuous — a one-objective "Pareto frontier" is a minimum, and the derivation the design is careful to describe as "nothing else beat it on every objective at once" is beating nothing on one objective.

- **VerificationReport::summary() prints "All gates passed" when two of six gates were skipped, contradicting the rule stated in the same file**
  - `crates/binmap-core/src/gate.rs`:238
  - requirement: §6 ("reported as rejected with the failing gate named"); gate.rs:106-107's own stated invariant; exit criteria ("the five gates run")
  - The tool's whole thesis is that it does not overstate what it measured, and this is the one line where it does. Combined with the two structurally-skipped gates and the baseline-not-candidate gating, a user reading "All gates passed" on the winning configuration is being told six checks succeeded when two never ran, two ran against the wrong binary, and one (SizeNotWorse) passes by definition. The right output is "Passed, with BenchmarkNotWorse and MiriClean skipped", parallel to the existing inconclusive branch.

- **SessionArtifact::with_gates is never called, so the versioned artifact ships with an empty gates list**
  - `crates/binmap-session/src/artifact.rs`:116
  - requirement: Plan §2, line 95 ("binmap.json is the stable contract carrying findings, evidence, gate results and target metadata"); Exit criteria ("the artifact schema is versioned")
  - The schema is genuinely versioned — SCHEMA_VERSION = 1 (artifact.rs:13) with a forward-refusal at artifact.rs:190-195, and store/tests.rs:137 covers it — so that clause of the exit criterion holds for the container. But the container is being shipped with one of its four declared payloads empty. A colleague importing a binmap.json gets findings and evidence and no gate verdicts, unless they also happen to be running the same engine build that can parse the run blob — which is exactly the coupling the RunRecord design was introduced to avoid.

- **TargetMetadata hardcodes commit: None and dirty: false, discarding the reproducibility information the environment probe already computes**
  - `crates/binmap-session/src/artifact.rs`:40
  - requirement: Exit criteria ("the headless harness reproduces every number the interface shows"); F0.7
  - Every exported artifact silently claims `dirty: false` — the serde default and the hardcoded value coincide, so a session measured on a dirty tree is indistinguishable from one measured on a clean tag. That is worse than omitting the field. It is also the one piece of metadata the reproduction exit criterion actually depends on: without the commit, "reproduces every number" has no defined input.

- **Nothing compares a harness number to an interface number, and the two entry points duplicate the gate plan rather than sharing one, so they can diverge without any test noticing**
  - `crates/binmap-eval/src/main.rs`:103
  - requirement: Exit criteria ("the headless harness reproduces every number the interface shows")
  - The criterion is vacuously true today — the interface shows nothing — but it is also currently untestable by construction, and that is the part worth fixing before the views land. Making it genuine needs three things that do not exist: one shared engine-configuration constructor so the app and the harness cannot be measuring different standards; a corpus-driven test that runs a sweep and asserts on the numbers; and an artifact written by the acceptance path, so "the number the interface shows" and "the number the harness produced" are the same serialised value rather than two independent computations that happen to agree. The duplicated GatePlan is the concrete divergence risk: change the test command in one file and the harness silently certifies a standard the app does not apply.

- **`Finding::revalidate` re-checks grounding but never re-applies the provenance ceiling, so an edited binmap.json restores an externally-inferred finding at Certain**
  - `crates/binmap-core/src/finding.rs`:322
  - requirement: Plan §2.6 ceiling table (Inferred → Probable); finding.rs:320-321 "Import is not construction, so it needs its own gate"
  - The comment says import gets its own gate; the gate it gets is half the constructor's. A session file — the artifact the plan calls the stable contract, and the thing U13 has colleagues mail to each other — can restore an agent's guess wearing the badge of a measurement, with grounding that checks out.

- **Session import revalidates findings but adopts `gates` verbatim; a gate outcome citing evidence that was never issued survives untouched**
  - `crates/binmap-session/src/artifact.rs`:201
  - requirement: Phase 0 exit criteria "the artifact schema is versioned, the gates run"; artifact.rs:187-188 "Import is not construction, so it runs the same checks construction ran. A file is not a more trustworthy source than an agent"
  - A restored session shows the Profile Lab's gate tally — "TestsPass · the suite passes" — from a file, with no run behind it and no citation that resolves. The doc comment two lines above the function promises the opposite of what the function does, which is worse than an unchecked import that admits it, because a reviewer reading the module concludes the check is there.

- **Nothing anywhere validates that a `GateOutcome`'s evidence ids were issued — the harness copies caller-supplied ids straight into the verdict**
  - `crates/binmap-verify/src/lib.rs`:331
  - requirement: R2 (plan §10); plan §3 evidence store — the airlock question is `store.issued(&id)`
  - The airlock guards `Finding` and only `Finding`. Gate verdicts are the strongest claims the tool makes — they are what "tests passing" in the Phase 0 acceptance criterion means — and they carry citations no one checks. When the Phase 2 ACP airlock arrives, an external agent proposing a candidate supplies `measurement_evidence` directly, and this is the hole it walks through.

- **Several gate verdicts are produced with an empty evidence list, including a `Failed` that rejects a candidate**
  - `crates/binmap-verify/src/lib.rs`:279
  - requirement: R2; plan §6 gate table; gate.rs:129 "One gate's verdict, with the evidence behind it"
  - `NoNewWarnings` failing is a rejection the user must be able to argue with, and the Profile Lab's rejection row will have nothing to open. The struct's own doc line claims the evidence is there. A finding may not exist without evidence; a gate verdict — which is a stronger claim — may.

- **A hand-edited binmap.json injects a fully-accepted finding: the only barrier is a SHA-256 the file itself documents how to compute**
  - `crates/binmap-session/src/artifact.rs`:189
  - requirement: F0.7 / U13 session artifact with import; artifact.rs:187-188 "A file is not a more trustworthy source than an agent"
  - The digest is a corruption check, not an integrity check, and the code presents it as the latter — the existing test (crates/binmap-session/src/store/tests.rs:76-94) only exercises the naive edit that forgets to recompute it. Since U13's whole point is opening a file a colleague sent, the threat model is precisely an artifact from someone else's machine. Either the artifact should be signed, or the doc comment and the restored-session notice should say plainly that an imported session's numbers are unverified until re-run.

- **Every redacted export produces a file whose import drops all evidence as tampered and therefore all findings as ungrounded**
  - `crates/binmap-session/src/store/mod.rs`:95
  - requirement: U13 (plan §5 Phase 0: "session export with redaction plus import"); plan §3 "the stable contract carrying findings, evidence, gate results and target metadata... with export and import"
  - The two halves of U13 cancel out. A user redacts a session, mails it, and the recipient opens a file with zero findings and a refusal notice — the feature's only real use case is the one it cannot serve. The redaction is a known, self-inflicted alteration and should be recorded as such (re-digest, plus a `redacted: true` marker on each altered record so the recipient still sees the change), not laundered through the tamper detector.

- **`Finding::new` failures are discarded with `if let Ok(...)` in the sweep, so an ungrounded finding vanishes silently instead of being reported**
  - `crates/binmap-build/src/sweep/mod.rs`:364
  - requirement: R2; plan §6 "A candidate failing a gate is rejected with the failing gate named, and stays visible"; error.rs:28-30
  - Silently dropping is the one behaviour the gate vocabulary is designed to prevent — a rejected candidate is supposed to stay in the table with its reason. Worse, the drop is invisible: the sweep summary at mod.rs:271 counts `state.measured.len()`, so the tally and the visible rows disagree with no explanation, and the Phase 0 exit criterion (the headless harness reproduces every number the interface shows) is measured through the same swallowing path.

- **A benchmark regression flips a gate to `Failed` in place but the already-streamed finding is never re-emitted, so the interface keeps a claim the gate record contradicts**
  - `crates/binmap-build/src/sweep/mod.rs`:486
  - requirement: R2; plan §3 streaming event model; plan §6 gate reporting
  - The user sees a `Configuration` finding reading "measured" for a candidate the harness has since rejected on runtime, and the Inspector's gate tab beside it says Failed — the interface contradicting itself is the fastest way to lose the trust the whole design is built to earn. The uncited verdict compounds it: the one gate whose decision came from statistics cites only the size record.

- **`perf_event_paranoid` produces a user-visible measured value with no evidence record at all, and `Probe` has nowhere to cite a record even when one exists**
  - `crates/binmap-build/src/environment/mod.rs`:184
  - requirement: Plan §3 "Every tool invocation writes a record with an identifier before its result is returned"; U0.4/U10 environment probe; F0.5 vocabulary (FindingKind::EnvironmentProbe exists at crates/binmap-core/src/finding.rs:113)
  - The environment panel is a wall of assertions about the user's machine with nothing behind any of them. `FindingKind::EnvironmentProbe` exists and `view_for_kind` routes it to `View::Environment` (crates/binmap-gui/src/state/mod.rs:364), so the design clearly intends probes to become grounded findings; the engine gives the view a `Probe` that cannot become one. The kernel-setting probe is the case that matters most, because it is the one whose "Apply" button (mod.rs:217) asks the user to run sudo on the strength of an unevidenced reading.

- **GatePlan::significance is printed beside the gate but never reaches the decision — timing::compare hardcodes p > 0.05**
  - `crates/binmap-measure/src/timing.rs`:181
  - requirement: gate.rs:141-143 "A threshold that decides a verdict belongs on screen next to the verdict"; plan §6 BenchmarkNotWorse
  - The one number the design puts on screen to justify the verdict is not the number that produced the verdict. A user who tightens the threshold to 0.01 sees the label change and the verdicts stay identical — the interface would be reporting a decision rule the engine does not apply.

- **substantial_ffi is a flag nobody sets, so the MiriClean FFI caveat can never appear**
  - `crates/binmap-verify/src/lib.rs`:384
  - requirement: plan §6 ("the sanitizer gap is stated in the interface: a clean run on code with substantial foreign-function calls is a much weaker statement than it sounds, and a tool implying otherwise is misleading its user about safety")
  - `report.caveats()` (gate.rs:227-229) is always empty in production, so the sanitizer gap the plan singles out as a safety-misleading failure is never stated to the user. MiriClean reports `Skipped — no unsafe touched` and `passed()` returns true for a project that is half FFI. The caveat machinery, the caveat field and `caveats()` are exercised only by tests, which makes the gap look covered in the test suite while being unreachable in the product.

- **The environment probe discovers miri but nothing ever calls sanitizing_with, so plan.miri is always None**
  - `crates/binmap-build/src/environment/mod.rs`:221
  - requirement: plan §6 MiriClean ("Sanitizers clean over the reachable portion", applies when unsafe is touched)
  - Even if `touches_unsafe` were determined, the gate would take the `plan.miri.is_none()` branch (verify/lib.rs:362-367) and report "no sanitizer is available on this machine" on a machine where the probe just found miri and told the user it was available. The Environment panel and the gate would contradict each other.

- **count_warnings counts cargo's per-package "generated N warnings" summary line as a warning, inflating BuildOutcome::warnings by one per package**
  - `crates/binmap-build/src/cargo.rs`:156
  - requirement: plan §6 Builds ("warnings diffed against the baseline")
  - `BuildOutcome::warnings` is wrong by +1 for every workspace member that emitted any warning, so any consumer that diffs it (the design's "0 new" cell, a future baseline population) reports phantom new warnings when the set of warning-emitting packages changes without the warning count changing. The two duplicate implementations differ in exposure to the bug purely by which flags the caller happened to pass, so fixing one leaves the other wrong.

- **SizeNotWorse always passes when size was measured, but the built design renders it as the failing gate on a size increase**
  - `crates/binmap-verify/src/lib.rs`:330
  - requirement: design literals at Binmap v3.dc.html:1733-1736 and :1937 vs plan §6 SizeNotWorse
  - The Profile Lab's Gates column renders `Badge tone=fail` with `r.failedGate` (design line 1744); with the current engine that badge can never read SizeNotWorse, and a configuration that grows the binary by 134 KB is handed to the view as `pass`. Either the design's fail state and size budget need an engine that can produce them, or the design's literals need correcting — but a view built against the design as it stands will render a state the engine cannot reach.

- **VerificationReport::passed() returns true for an Inconclusive gate, and passed() is the only aggregate the sweep and the frontier consult**
  - `crates/binmap-core/src/gate.rs`:210
  - requirement: plan §6 ("a result inside it is reported inconclusive, never coloured as a win")
  - The engine offers a view no way to ask "did this pass on measurements, or only because nothing could be measured" other than re-deriving it from `inconclusive()`, and the engine's own summary sentence claims a candidate "passed every gate" when a gate could not tell. A frontier point whose only advantage is a runtime difference inside the noise floor is eligible, wins, and is emitted as a FrontierPoint finding whose `Impact::is_a_win()` returns true — the noise-floor result coloured as a win, in the engine, before any view exists.

- **Pareto dominance treats a missing axis one way for the dominator and another for the dominated, so the frontier depends on which point happened to lack a measurement**
  - `crates/binmap-measure/src/pareto.rs`:88
  - requirement: F0.6
  - This is reachable as soon as F0.5 is wired: `benchmark_survivors` skips a configuration whose artifact has gone (sweep/mod.rs:465-469) or whose benchmark run errors (line 470), leaving `runtime_nanos: None` while its neighbours have one. Those configurations are then silently eliminated by any smaller timed point — including, possibly, the fastest configuration in the sweep — while the reverse gap is treated as incomparable. The frontier a user is shown becomes a function of which measurements happened to fail, and the derived finding still says "Nothing else measured in this sweep beat it on every objective at once" (sweep/mod.rs:530).

- **The frontier compares runtime medians with strict `<`, so a sub-noise difference decides frontier membership**
  - `crates/binmap-measure/src/pareto.rs`:71
  - requirement: F0.6; TOOLING §4.2 ("refuse to report improvements beneath" the measured floor); plan §6
  - Two configurations whose runtimes differ by 1 ns — well inside a 0.9% machine floor — are ranked as a genuine trade-off: one is declared non-dominated and gets a `FrontierPoint` finding with `Provenance::Derived { rule: "pareto-dominance" }` and `Confidence::High` (sweep/mod.rs:543-547). The gate layer is scrupulous about not calling noise a win and the frontier layer, which is what the Profile Lab actually plots, is not. The same applies to build time, for which no floor is measured at all.

- **The benchmark verdict is dropped unless it is a regression, so BenchmarkNotWorse still reads "no benchmark is declared" after the benchmark ran**
  - `crates/binmap-build/src/sweep/mod.rs`:472
  - requirement: F0.5; plan §6 gate table (BenchmarkNotWorse); PRD R7
  - A configuration that measurably got faster shows a gate row saying the benchmark was skipped because none is declared, and — worse for the product's stated purpose — a difference inside the noise floor produces no `GateResult::Inconclusive` either, so `VerificationReport::inconclusive()` (gate.rs:216) is always empty and the "passed, but N gates could not tell" summary (gate.rs:236-243) can never fire from a real sweep. The one behaviour the design calls the difference between a measurement tool and a demo ("inconclusive · +0.6% n.s.") is unreachable, and the regression patch-up at 486-487 mutates a `GateOutcome` whose `detail` was written by a different code path, leaving `configured`/`evidence` describing the skip.

- **Runtime measurements carry no evidence: MeasurementSource::samples returns durations with no EvidenceId, and the samples themselves are not retained**
  - `crates/binmap-core/src/traits.rs`:113
  - requirement: Phase 0 exit criterion ("the headless harness reproduces every number the interface shows"); F0.5
  - Findings are refused without evidence (`Finding::new` returns `Error::Ungrounded` when `draft.evidence.is_empty()`, finding.rs:293-296), which is the project's central discipline — yet every timing number would enter the state ungrounded, and a rejected-by-benchmark configuration would cite a build and a size measurement but nothing about the timing that rejected it. Because the samples and the p-value are discarded, the noise floor, spread and significance the interface is designed to show beside each runtime cannot be recomputed or replayed from the session artifact.

- **The noise floor is measured per sweep, is not persisted, carries no timestamp, and is unreachable through the Engine facade**
  - `crates/binmap-build/src/sweep/mod.rs`:308
  - requirement: plan §6 ("measured once per machine by timing one unchanged binary repeatedly"); TOOLING §4.2 ("measure the machine's noise floor once, store it")
  - The plan and TOOLING both say once per machine and stored; the code says once per sweep and forgotten. The built design states it as a durable machine property — "noise floor on this machine: 0.9% · measured once, 3 Sep" (Binmap v3.dc.html:131) — which needs both persistence and a timestamp the type does not have. And even within a session a view holding `Arc<dyn Engine>` cannot obtain the floor, so "the noise floor beside every number" has no path to the interface at all.

- **No sweep test exercises the benchmark, the noise floor, or the regression patch-up — every test passes benchmark: None**
  - `crates/binmap-build/src/sweep/tests.rs`:155
  - requirement: F0.5; Phase 0 exit criterion
  - The most statistically delicate code in the product — noise floor establishment, sample comparison, and the mutation of a gate outcome after the fact — has zero integration coverage, which is why defects like the discarded Improved/Inconclusive verdict and the lost floor on resume are invisible to the suite. A stub source returning canned durations would exercise all of it without hyperfine being installed.

- **binmap_measure::hyperfine::measure and Timing have no caller outside hyperfine.rs's own test module**
  - `crates/binmap-measure/src/hyperfine.rs`:61
  - requirement: F0.5; TOOLING §4.2 ("timing through hyperfine, JSON export parsed")
  - hyperfine is the obvious and intended MeasurementSource implementor, and the wiring between the two was never written. The environment panel tells the user to `cargo install hyperfine` and reports it as present, which promises a runtime objective the engine has no code path to produce.

- **SessionStore::save/load/read, artifact::import, Finding::revalidate and BinmapEngine::adopt_run have no non-test callers — sessions are never persisted or re-opened**
  - `crates/binmap-session/src/store/mod.rs`:42
  - requirement: F0.7 session persistence with export and import; F0.8 resumable sweeps; U13
  - A sweep costs 96 builds and nothing on the live path writes it to disk, so closing the app discards it; `Request::ResumeSweep` can only ever resume a run created in the same process. The import trust gate — the piece that makes an artifact from a colleague no more trusted than one from an agent — has never run outside a unit test, and binmap-eval has no import subcommand, so the exit criterion that the harness reproduce what the interface shows cannot cover it.

- **Engine::propose_configuration, Engine::proposals and the whole engine::manifest module are called only from engine/tests.rs**
  - `crates/binmap-core/src/facade.rs`:178
  - requirement: F0.6 / U0.3 — the Profile Lab turns a frontier point into a proposal with its diff; §5 Phase 0 "proposes configurations and shows their diffs"
  - Phase 0's deliverable is a user who sees a smaller configuration and can act on it. The sweep ends by emitting FrontierPoint findings and stops there: nothing converts a frontier point into the `[profile.release]` edit, and binmap-eval's `sweep` and `acceptance` commands print the configuration but never call propose_configuration, so the manifest-editing code has never been exercised against a real Cargo.toml outside its unit tests.

- **Engine::start — the entire request dispatch, run registry and detached-thread path — is never called by the binary or by binmap-eval**
  - `crates/binmap-build/src/engine/mod.rs`:221
  - requirement: Phase 0 exit criterion — "the headless harness reproduces every number the interface shows"; A1.3
  - The code path the interface will actually use is the one the harness does not exercise. Run minting, `Request::Sweep`/`ResumeSweep` dispatch, thread spawning, and cancellation mid-sweep are covered only by unit tests with a stub builder, so the harness cannot certify the numbers the interface will show — it certifies a different entry point. This is not the missing GUI: it is the harness taking a shortcut around the seam it exists to validate.

- **measure_size computes the section table on every configuration and the sweep throws it away, keeping only total_bytes**
  - `crates/binmap-build/src/sweep/mod.rs`:399
  - requirement: F0.4 — "Per configuration, measure total and per-section size, build time and test outcome"
  - Per-section size is half of F0.4 and it is computed and then discarded, so no view, session artifact or harness output can ever show it. The section data survives only inside the free-text evidence record built at size.rs:28-32, which is not a structure anything can query.

- **BuildOutcome.warnings is populated and never read; GatePlan::against_baseline_warnings is never called, so the NoNewWarnings gate always compares against zero**
  - `crates/binmap-core/src/traits.rs`:58
  - requirement: F0.4; binmap-verify/src/lib.rs:64-67 — "A candidate is judged on the difference, not the total: a project that starts with warnings is not thereby forbidden a smaller binary"
  - The documented behaviour is inverted on the live path. Any reference-corpus crate that already emits one warning fails NoNewWarnings on all 96 configurations, `report.passed()` is false for every one, `acceptance` prints "no configuration both built and passed its gates" and exits non-zero — the Phase 0 acceptance criterion cannot be met on such a crate for a reason that has nothing to do with the crate.

- **Request::NoiseFloor is dispatched but measures no noise floor, because the benchmark it needs is always None**
  - `crates/binmap-build/src/engine/mod.rs`:238
  - requirement: F0.5 — the noise floor is established before any runtime comparison; facade.rs:134-137 documents NoiseFloor as "Runs before any runtime comparison, and its result is shown beside every timing number"
  - A request the facade advertises returns success and produces nothing. A view wiring a "measure this machine's noise floor" action to it would get a completed run with an empty result and no error explaining why.

- **SweepMatrix.build_std and target_cpu are empty in every constructed config, so unstable_args and rustflags return empty on every live build**
  - `crates/binmap-core/src/config.rs`:128
  - requirement: F0.3 — "Optional extended sweep: build-std with panic_immediate_abort, and target-cpu"
  - F0.3 is in Phase 0 scope and there is no code path that can enable it: no configuration file is read, no setter exists on the engine, and the harness has no flag. The nightly/rust-src probe at environment/mod.rs:125-140 checks a prerequisite for an axis nothing can turn on. build-std with panic_immediate_abort is also one of the largest size wins available, so its absence directly reduces the chance of hitting the 25% acceptance bar.

- **The Engine facade exposes no way to obtain the measured configuration table, the frontier, the baseline or the noise floor — they live in SweepState, a type binmap-gui cannot name**
  - `crates/binmap-core/src/facade.rs`:149
  - requirement: §4 Profile Lab / U0.3 — "Pareto scatter with the frontier derived, not flagged; configuration table including rejected candidates; selected flags, gates"
  - This is an engine gap, not a missing view: the Profile Lab's designed contract needs per-configuration size, build time, gate outcomes and the settings table, and every one of those is computed into `MeasuredConfiguration`/`VerificationReport` and then kept in a crate the interface is structurally forbidden to depend on. Building the view will require adding facade methods and core-side types first.

- **Build time is recorded as unmeasured on every configuration under the default parallelism, emptying the BUILD column and the scatter's circle area**
  - `crates/binmap-build/src/sweep/mod.rs`:426
  - requirement: Profile Lab scatter legend "Circle area is build time" and the BUILD column (design configColumns key "build"); F0.6 frontier over size, runtime and build time
  - In the default configuration the design's third objective is always absent: the BUILD column is blank, every scatter circle is the same size, and build time can never contribute to the Pareto frontier (Point::dominates treats a missing axis as non-comparable). Either the design's build-time affordances need an engine that times builds under concurrency (e.g. per-build CPU time or a serial re-time of the frontier), or the design needs to state that build time is unmeasured.

- **Configurations rejected by a gate are never benchmarked, so the RUNTIME column is blank exactly for the rows the design shows runtime on**
  - `crates/binmap-build/src/sweep/mod.rs`:452
  - requirement: Profile Lab configuration table RUNTIME column and the scatter's "Rejected by a gate" series; §"a near miss is informative"
  - Rejected candidates are kept visible on purpose, but they land on the scatter with no x-coordinate and an empty RUNTIME cell — so the one view that is supposed to show a user why a candidate lost cannot show the cost that made it lose.

- **The baseline build produces no measured row, so the table's "(current)" configuration — its size, its gates, its zero delta — cannot be rendered**
  - `crates/binmap-build/src/sweep/mod.rs`:304
  - requirement: Profile Lab configuration table (design configs literal, row id "3-off"); §"every number in the run is stated against it"
  - The row every other row is measured against is the one row the table cannot draw. A user cannot see what default release costs, whether it passes its own gates, or where it sits on the scatter — which is the anchor the whole 25%-smaller claim rests on.

- **Proposal has no required trust tier and Verify/Apply are hard errors, so the Inspector's Proposal tab cannot render its tier badge, blocked reason, verification run or applied note**
  - `crates/binmap-core/src/facade.rs`:115
  - requirement: Findings Inspector — "proposal diff, verify, apply", plan §4 phase 0 (U4, U0.3); Tier and apply dialogs (U9, A2.4)
  - TrustTier exists and can state what it permits, but nothing links a proposal to the tier it needs — so the apply dialog cannot name the raise it is asking for, and the Verify button has no path behind it but an error string. The gating story the design sells is the one part of the proposal flow with no engine support.

- **A sweep never produces a proposal, and the interface cannot ask for one because propose_configuration needs a BuildConfiguration it can never obtain**
  - `crates/binmap-build/src/engine/mod.rs`:289
  - requirement: Findings Inspector Proposal tab; design proposal `t1` "Set opt-level = \"s\" and lto = \"fat\"" attached to finding t1, and the Tune view's "Apply to Cargo.toml" footer button
  - The Tune view's whole payoff — turn the winning frontier point into a Cargo.toml edit — is unreachable: the manifest editor and diff generator exist and are good, but no caller can name the configuration to feed them.

- **Evidence records carry no one-line summary and no provenance kind, and a rule-derived evidence row has no representation at all**
  - `crates/binmap-core/src/evidence.rs`:96
  - requirement: Findings Inspector Evidence tab — "each evidence row carries tool, arguments, digest and verbatim output" (plan §4, phase 0; U4, U0.3, AI.6)
  - Every evidence row would render with the raw command line as its heading and no way to distinguish a measurement from a rule application. The Pareto-dominance rule that decides the frontier — the one derivation Phase 0 makes — leaves no evidence row stating what it concluded, so the Evidence tab cannot show the grounds for the finding the Tune view is built around.

- **Runs are stored without a time, a result or a status, so the Target view's run-history table cannot be filled**
  - `crates/binmap-session/src/artifact.rs`:54
  - requirement: Target view — "run history" (plan §4, phase 0; U1, F0.1, F0.7)
  - Three of the four columns have no field to read. `Sweep::summary()` produces exactly the sentence the RESULT column wants but it is emitted as a transient `EngineEvent::Finished` and never persisted against the run, so reopening a session loses the history the design puts on the landing view.

- **The engine exposes no project name, commit or dirty flag, so the title bar's three props have no structured source**
  - `crates/binmap-session/src/artifact.rs`:33
  - requirement: Title bar — "Crate, commit and dirty flag" (plan §4, phase 0; U9); first-run configure step heading
  - The title bar would have to string-parse a human-readable probe detail to find the commit hash and infer dirtiness from which Probe constructor was used — and it has no way at all to learn the project's name. The dirty flag is the thing that tells a reader a measurement will not reproduce; it deserves a field, not a substring.

- **Nothing in the engine names a reasoner, its mode, its egress or its cost, so the title bar badge, the status bar's right half and first-run step 3 have nothing to render**
  - `crates/binmap-core/src/facade.rs`:149
  - requirement: Title bar "active reasoner with egress badge" and First run step 3 "reasoner choice" (plan §4, phase 0; U9, AI.1, AI.9); Phase 0's null model layer
  - Phase 0 ships the null model layer, and the design shows the null choice as a first-class, named, egress-labelled selection — "no reasoner · deterministic analysis only · nothing leaves this machine". Without a type for that, the status bar and title bar would hard-code strings the engine cannot later contradict, which is exactly the coupling the facade exists to prevent.

- **EngineEvent::Progress carries only a counter and a free-text message, so the run panel's stage list, and the status bar's best/elapsed, cannot be built**
  - `crates/binmap-core/src/event.rs`:37
  - requirement: Run progress panel and status bar (design progressTitle/stages/progress literals); U0.5 streaming and cancellable runs
  - The stage list is how the design tells a user which tool is running and why an hour-long sweep is worth waiting for. With only an unstructured message string, the interface must either invent its own stage vocabulary — which then cannot stay true to what the engine actually did — or drop the panel.

- **Configuration findings are emitted before benchmarking and never updated, so a configuration the benchmark later rejects still reads as passing and no finding ever carries a runtime impact**
  - `crates/binmap-build/src/sweep/mod.rs`:364
  - requirement: Findings Inspector impact line and nav-rail badge counts; design findings.tune[t1].impact
  - `Impact` has a `runtime_nanos` field and a `within_noise_floor` flag built exactly for this, and neither is ever populated by Phase 0. The Inspector therefore shows a size-only impact for a view whose selling point is the size/runtime trade-off, and a configuration rejected by the benchmark keeps a Finding titled "measured" that contradicts its own gate report.

- **`cargo deny check bans` errors on this deny.toml today, and nothing runs it, so the architectural boundary is unenforced**
  - `deny.toml`:14
  - requirement: TOOLING §6 (cargo-deny bans enforce DESIGN-GUI §4's architectural boundary); DESIGN-GUI §4 "Enforced in CI with cargo-deny's dependency bans, not by convention"
  - The boundary that stops a view from calling an analysis is currently convention only. A contributor who adds binmap-build to binmap-gui gets no signal, because the one command that would catch it exits non-zero for unrelated reasons and is never run. Fixing it means giving the workspace path dependencies a `version` field (or setting `publish = false`), not adding a flag.

- **The bloaty cross-check has no call site in the engine, and its test reports green on a machine without bloaty**
  - `crates/binmap-measure/src/bloaty.rs`:112
  - requirement: TOOLING §4.1 ("If your total attributed bytes disagree with bloaty's by more than a small margin, your attribution is wrong. Make that a test.")
  - The oracle exists on paper only. The Environment panel already tells the user bloaty is "the oracle that contradicts it" (crates/binmap-build/src/environment/mod.rs:39), which is a claim the engine does not honour — the cross-check runs neither in the product nor, in practice, in the suite.

- **The build-std axis emits `-Zbuild-std=core,alloc,panic_immediate_abort`, which is not how panic_immediate_abort is enabled**
  - `crates/binmap-core/src/configuration.rs`:162
  - requirement: TOOLING §3.3 ("`-Z build-std=std,panic_abort` with `-Z build-std-features=panic_immediate_abort`"); plan §5 F0.3
  - F0.3's headline size lever — the one the study calls "dramatic on small binaries" — cannot work. Every extended-sweep configuration will fail to build, and because a failed build is recorded as a rejected candidate rather than a tooling error, the user is told the axis does not help rather than that Binmap invoked it wrongly. The axis is off by default, so this is latent rather than currently visible.

- **BuildConfiguration::name() truncates values to 6 alphanumerics, so distinct build_std and target-cpu configurations collapse to the same name and the same target directory**
  - `crates/binmap-core/src/configuration.rs`:210
  - requirement: F0.3 (the extended build-std sweep); configuration.rs:200 doc: "A short, stable, filesystem-safe name. Used as the per-configuration target subdirectory, so a resumed sweep finds its own cached builds"
  - Two configurations share `target_directory_for()` (cargo.rs:47), so a parallel sweep has them racing on one target dir and a serial sweep measures the second against the first's cached artifact. `SweepState::remaining()` keys on the name (sweep/mod.rs:141), so on resume the second configuration is treated as already measured and silently never built — the extended sweep quietly loses half its points and attributes one build's size to two different profiles.

- **measure_size drops its PendingEvidence on every error path, leaving a permanent phantom entry in EvidenceStore::incomplete()**
  - `crates/binmap-measure/src/size.rs`:23
  - requirement: crates/binmap-core/src/evidence.rs:200 — `incomplete()`: "Invocations that were begun and never completed — a tool that hung, or a run that was cancelled. The environment panel surfaces these"
  - The comment states the opposite of what the code does. Every configuration whose artifact cargo named but did not produce adds a permanent record that the environment panel will report to the user as a hung tool, and the actual failure (the io error, with its message) is never recorded as evidence at all — so a failed measurement is unauditable. Contrast ToolRunner::run_with_env (tool.rs:107), which does complete the record on the spawn-failure path.

- **GatePlan::significance is only ever formatted into a label; the benchmark verdict is decided against a hard-coded 0.05**
  - `crates/binmap-measure/src/timing.rs`:181
  - requirement: crates/binmap-core/src/gate.rs:141 — "A threshold that decides a verdict belongs on screen next to the verdict"; binmap-verify/src/lib.rs:68 — "The significance level BenchmarkNotWorse decides at"
  - The one field whose stated purpose is to make the deciding threshold visible next to the verdict shows a number that decided nothing. A user who tightens the threshold gets no change in behaviour and no indication of that, and the existing test locks in the misleading label.

- **A failed sweep emits two EngineEvent::Failed events for one run, breaking the one-terminal-event invariant**
  - `crates/binmap-build/src/engine/mod.rs`:191
  - requirement: crates/binmap-core/src/event.rs:74 — `is_terminal`: "Whether this is the last event for its run. The interface stops showing progress on exactly these"; the invariant asserted by `a_recorded_run_ends_in_exactly_one_terminal_event` (event.rs:132)
  - The designed views apply events to entities and stop a run on the terminal event; a second Failed for an already-finished run either double-reports the error to the user or drives a state machine that has already torn the run down. The sweep's own test asserts exactly one terminal event, but only on the success path, so this is untested.

- **A manifest whose [profile.release] is an inline table produces a Proposal with an empty diff and a summary claiming it will write settings**
  - `crates/binmap-build/src/engine/manifest.rs`:47
  - requirement: U9 (the apply dialog states what it will write); manifest.rs:20 — "Returns the manifest unchanged … refusing to guess is better than writing a broken file"
  - Refusing to guess is right; returning a proposal that promises a write and carries no diff is not. The user is shown an actionable proposal whose diff pane is blank, with no sentence saying why, and (once Apply lands) a write that changes nothing while the tool reports success. `propose_configuration` should surface the refusal rather than emit an empty proposal.

## minor (42)

- **SessionArtifact.gates is declared and never filled: with_gates has no caller in the workspace**
  - `crates/binmap-session/src/artifact.rs`:141
  - requirement: F0.7 — "the stable contract carrying findings, evidence, gate results and target metadata" (plan §4 line 95)
  - Gate results reach the file only buried inside the opaque `RunRecord.state` blob, which artifact.rs:74-77 explicitly says no consumer other than the engine that wrote it may read. The artifact's own top-level, schema-versioned gate field — the part a colleague's build or a CI consumer would read — is always empty, so the "stable contract" carries three of its four advertised categories.

- **TargetMetadata::of hard-codes commit: None and dirty: false even though the environment probe already computes both**
  - `crates/binmap-session/src/artifact.rs`:59
  - requirement: F0.7 target metadata (plan §4 line 95); Phase 0 exit criterion on reproducibility
  - `dirty: false` is not "unknown", it is an assertion — every exported artifact claims it was measured on a clean tree at an unnamed commit. The recipient is told the opposite of what the field's own doc comment promises, and the probe that knows the truth is two crates away with nothing wiring them together.

- **benchmark_survivors re-times every passing configuration on each resume, with no skip for ones that already have a runtime**
  - `crates/binmap-build/src/sweep/mod.rs`:448
  - requirement: F0.8 resumable sweeps — "a resumed sweep skips the configurations already in `measured` rather than repeating an afternoon's builds" (sweep/mod.rs:106-108)
  - Benchmarks run strictly serially by design (sweep/mod.rs:11-12), so re-timing the full set of survivors is the most expensive phase to repeat and the resume path repeats all of it. It also silently overwrites earlier runtime figures with ones measured under a different machine state, which is the kind of cross-session comparison the noise-floor discipline exists to prevent.

- **Default parallelism is half the machine's cores, so build_time_is_measurable() is false out of the box and the shipped app never records a build time for any configuration**
  - `crates/binmap-core/src/config.rs`:268
  - requirement: F0.8 parallelism cap; the Profile Lab's build-time column in docs/design/Binmap v3.dc.html
  - Recording no build time rather than a wrong one is the right rule, but the default makes it the universal case: on any multi-core machine the shipped binary produces a build-time column that is empty for every row, and Point::with_build_time (sweep/mod.rs:91-93) is never reached, so build time is never an axis on the Pareto frontier. The engine offers no way for a user to opt into serial measurement — a designed view has a column the engine can never fill under its own defaults.

- **The export test asserts that redacted evidence reads as tampered but never checks whether the findings survive the import — and they do not**
  - `crates/binmap-session/src/store/tests.rs`:113
  - requirement: U13 (session export with redaction plus import)
  - U13's import half is untested at exactly the point where it is broken: a colleague opening a shared .binmap.json gets a refusal notice and an empty findings list. The suite is green because the one test in the neighbourhood stops asserting one line before the interesting question.

- **`the_gap_between_sections_and_the_file_is_reported_not_hidden` asserts only `>= 0`, which a section reader returning nothing at all also satisfies**
  - `crates/binmap-measure/src/size.rs`:70
  - requirement: F0.4 / the test's own claim that the gap is "a number we can state"
  - The assertion is satisfied by total failure of the thing it is testing. A useful version pins the gap to a plausible band (say, under a few per cent of the file) or asserts the section total is non-trivially close to the file size, so an empty or truncated section table fails.

- **`a_configuration_that_does_not_build_stays_visible_as_a_rejected_candidate` claims to use a failing build gate but passes `sh -c true`, so the Builds-gate rejection path is never exercised inside a sweep**
  - `crates/binmap-build/src/sweep/tests.rs`:209
  - requirement: F0.5; verify §: "A candidate failing a gate is rejected with the failing gate named"
  - The test name promises coverage of rejection-by-gate and the comment claims it explicitly, so a reader trusts it and no one writes the real test. Nothing in the suite asserts that a genuinely failing build inside a sweep produces `rejected_by() == Some(Gate::Builds)` on the stored report.

- **The commented-out `[patch.crates-io]` fork escape hatch is absent from Cargo.toml, and there is no README to document it in either**
  - `Cargo.toml`:51
  - requirement: DESIGN-GUI §7 rule 5; §2.2 ("switching later is a [patch.crates-io] entry rather than a code change. Keep that escape hatch documented")
  - The escape hatch is the mitigation for the largest stated risk in the interface stack — a pre-1.0 framework with frequent breaking changes and no charting fallback. The documentation rule exists because the person who needs it will be blocked on a mainline wall and will not be reading DESIGN-GUI.md at that moment. A three-line commented block in Cargo.toml is the whole cost.

- **The GPUI fork decision was never recorded as an ADR; no ADR exists anywhere in the repository**
  - `docs/design/uploads/DESIGN-GUI.md`:123
  - requirement: DESIGN-GUI §2 ("Record this as an ADR")
  - The reasoning for choosing Zed mainline over the forks — shaders not needed, a fork several hundred commits behind, switching is a patch entry — lives only in a design upload alongside four superseded copies of itself. That is exactly the shape of decision the document says gets silently reversed, and the audit trail for the second-most consequential technical choice in the project (after the GUI framework itself) is one paragraph in a file nothing in the build references.

- **binmap-gui does not go through `binmap_core::facade::Engine` — it never names it; the binary pulls from the engine and pushes plain data into AppState**
  - `crates/binmap-gui/src/state/mod.rs`:9
  - requirement: §2.4 ("binmap-gui ... reaches analyses through a facade trait defined in core"); binmap-core/src/facade.rs:147
  - This is not a violation — nothing is bypassed, and it is expected while the views are unbuilt — but it means the facade is entirely unexercised from the side it was designed for. Nothing has yet proved that `Request::Sweep`, the `Arc<dyn EventSink>` callback and `Cancellation` compose into something a GPUI view can hold and drive on the foreground while work runs on the background executor. The first real test of the seam is still ahead, and main.rs:49-52 acknowledges it ("The window is next"). Worth an integration test that drives AppState through an `Arc<dyn Engine>` fake before the views land.

- **F0.3's third extended axis, relocation-model, is absent from SweepMatrix and BuildConfiguration**
  - `crates/binmap-core/src/config.rs`:111
  - requirement: F0.3 (PRD.md:197)
  - One third of a Should requirement is simply not modelled. `-Crelocation-model=static` (with the matching no-PIE link flag) is one of the reliable single-digit-percent size levers on Linux binaries, and it is the axis a user would reach for after opt-level and lto are exhausted.

- **The default matrix spends 48 of its 96 builds on an explicit strip="none", the exact value the code's own docstring identifies as producing a binary nine times larger than the one the user ships**
  - `crates/binmap-core/src/config.rs`:125
  - requirement: F0.2; Phase 0 acceptance (a first sweep that finishes)
  - Half of the first sweep a user ever runs is spent producing artifacts that are 9x the baseline and can never be on the size frontier — the axis is sized "so a first sweep finishes rather than impresses" (config.rs:119-120) and then doubles itself for no reachable result. `Strip::None` in the matrix is also not the same thing as the baseline's unset strip, so the two are not comparable as a controlled pair.

- **The reproduction command shown beside each Profile Lab point is not runnable: the shell eats the TOML quotes and cargo rejects the result**
  - `crates/binmap-build/src/cargo.rs`:78
  - requirement: F0.2; the Profile Lab's reproduction affordance (TOOLING §3.2 records --config as the by-hand mechanism)
  - The one string in the product whose entire purpose is "you can check this yourself" fails when checked, on every axis whose TOML value is quoted — opt-level s/z, every lto value, panic, strip and line-tables-only debug. It needs shell quoting (`--config 'profile.release.lto="fat"'`), which is a display concern the type currently has no way to express.

- **`re_checking_is_cheap_enough_to_be_a_button` fails nondeterministically because it compares two live git-status probes**
  - `crates/binmap-build/src/environment/mod.rs`:395
  - requirement: Phase 0 exit criteria (the gates run; the suite is the gate)
  - The suite does not pass on a clean checkout, so `cargo test` cannot be used as the phase gate it is meant to be, and the failure is in the very module F0.3 depends on for nightly detection — the next real regression there will be dismissed as "that one's just flaky".

- **SizeNotWorse always passes when size is known, but the built design shows it failing a configuration that grew**
  - `crates/binmap-verify/src/lib.rs`:330
  - requirement: §6 gate table vs. the built design's Profile Lab contract (Binmap v3.dc.html:1733-1736)
  - Two authoritative documents disagree and the code silently picked one, so the Profile Lab cannot be built faithfully from the design literals: a row the design renders red will render green. It is a small divergence with a real consequence for the design's rejected-candidates story — 2-overflow is one of only two rejections in the mock, and under the current engine it becomes a pass, leaving the "rejected candidates stay visible" affordance with almost nothing to display. Worth resolving explicitly in one document rather than leaving the code to arbitrate.

- **`Engine::evidence` silently drops citations the store cannot resolve, so the Inspector shows fewer records than the finding claims with no notice**
  - `crates/binmap-build/src/engine/mod.rs`:282
  - requirement: R2; finding.rs:99-101 "The Evidence tab shows this and does not summarize it"
  - A finding is only as good as the evidence a user can open. Given the store today cannot lose an id it issued, this is latent rather than live — but it is exactly the read path that would surface a corrupted or partially-imported session, and it is written to hide that instead of reporting it.

- **`measure_size` begins an evidence record and abandons it on both error paths, leaving a permanent phantom entry in `incomplete()`**
  - `crates/binmap-measure/src/size.rs`:18
  - requirement: Plan §3 evidence store ordering; evidence.rs:200-205
  - The environment panel is documented to surface incomplete invocations as hangs. A missing artifact — the ordinary outcome for a configuration that did not build, which `measure_one` reaches on every failed candidate (crates/binmap-build/src/sweep/mod.rs:399) — will report as a hung tool, once per failed configuration, for the rest of the session.

- **A configuration rejected by BenchmarkNotWorse was already emitted as a passing Configuration finding and is never re-emitted as rejected**
  - `crates/binmap-build/src/sweep/mod.rs`:364
  - requirement: plan §6 ("A candidate failing any gate is reported as rejected with the failing gate named, and stays visible")
  - A configuration that regresses runtime past the noise floor reaches the interface as `FindingKind::Configuration` titled "<name>: measured", and the corrected verdict exists only inside `SweepState`, which the view is not told changed. The finding stream — the thing the Findings Inspector renders — names no failing gate for exactly the rejection the benchmark pass exists to catch.

- **The Builds gate's detail is Debug-formatted Duration, not the duration string the design shows**
  - `crates/binmap-verify/src/lib.rs`:272
  - requirement: design literals at Binmap v3.dc.html:1714 and :1721
  - The gate tally's measurement column is a designed presentation string; the engine hands over a Rust Debug rendering with nanosecond precision. A view either re-parses it or renders `238.492831241s` where the design shows `3m 58s` — and `detail` is the only place the build duration reaches the report, so there is no structured duration to format instead.

- **A sweep cancelled between the baseline size and the noise floor can never measure a floor, silently disabling all runtime measurement on resume**
  - `crates/binmap-build/src/sweep/mod.rs`:210
  - requirement: F0.5 ("with the machine's noise floor established first")
  - The resumed sweep completes, marks itself `complete`, emits a summary and a frontier, and every configuration silently has `runtime_nanos: None` — indistinguishable, in the state and in the interface, from a project that declared no benchmark. Given finding `dominance-asymmetric-on-missing-axes`, this also changes which points the frontier contains.

- **Request::NoiseFloor builds default release and measures no floor**
  - `crates/binmap-build/src/engine/mod.rs`:238
  - requirement: F0.5; facade.rs:134-137 ("Measure the machine's noise floor by timing one unchanged binary repeatedly")
  - The one facade request whose entire purpose is the noise floor performs a full release build and produces nothing, with no error and no event saying why. A view offering "measure this machine" would show a successful run and an empty result.

- **Findings never carry runtime or build-time impact and never set within_noise_floor, though Impact models all three**
  - `crates/binmap-build/src/sweep/mod.rs`:506
  - requirement: F0.4/F0.5/F0.6 surfaced through findings; plan §6 ("never coloured as a win")
  - The designed finding card states impact as "−32.6% size, −2.1% runtime" (Binmap v3.dc.html:1755); the engine can only ever supply the size half. And because `within_noise_floor` is never set, `Impact::is_a_win` (finding.rs:187) returns true for any negative delta, so the flag that exists precisely to stop noise being coloured as a win is dead.

- **BenchmarkCommand.samples and ProjectConfig.test_command are never read; sample counts and the test command are hardcoded instead**
  - `crates/binmap-core/src/config.rs`:229
  - requirement: F0.4 (test outcome), F0.5 ("the user's benchmark command")
  - A project whose suite is not plain `cargo test` (a workspace needing `--workspace`, a nextest runner, a feature flag) is gated on the wrong command, and the count the user declared for their benchmark is ignored in both directions. The baseline/candidate asymmetry also feeds Mann-Whitney unequal group sizes that nothing in the design anticipates.

- **A failed size measurement leaves its evidence pending forever, contradicting the comment that says it is recorded**
  - `crates/binmap-measure/src/size.rs`:20
  - requirement: F0.4; the evidence discipline
  - The comment asserts a behaviour the code does not have: a failed measurement produces no citable record and leaves a permanent entry in the pending map, so an evidence store audited at the end of a sweep has phantom in-flight invocations for every artifact that could not be measured.

- **ArtifactReader and Symbolizer are declared as two of the four backend traits and nothing implements either**
  - `crates/binmap-core/src/traits.rs`:78
  - requirement: §2.5 — "the four traits artifact-specific logic is confined to… adding a language is a matter of implementing them"
  - The claim that a second backend is a matter of implementing four traits is untested: two of the four have never been implemented once, and the code that does read artifacts bypasses them. If CargoBuildSystem/binmap-measure had been made to implement ArtifactReader, the trait would be known to fit the work; as it stands the abstraction is unvalidated.

- **Request::Verify and Request::Apply are matched only to return an error, so no proposal can be gated or written**
  - `crates/binmap-build/src/engine/mod.rs`:249
  - requirement: §5 Phase 0 (apply arrives with the apply dialog); U9 trust tier control
  - The stub is honest and the plan defers Apply, but Verify is a different case: re-running the five gates over a proposal without writing anything is exactly what TrustTier::Observe permits and what the harness exists to do. As written, the tier ladder cannot be exercised end to end — TrustTier::Apply can be set via set_trust_tier (itself called only from engine/tests.rs:41) and there is no action it unlocks.

- **ToolRegistry is never constructed outside tool.rs's own tests; no tool is ever registered, so authorize can never succeed**
  - `crates/binmap-core/src/tool.rs`:166
  - requirement: §3 / plan line 94 — "The tool registry. One registry, two exposures… Each declares whether it has side effects, so the trust tier can gate it whoever called it"; plan line 154 — Phase 0's model layer is a null backend, "the registry and evidence store exist"
  - The evidence store is genuinely load-bearing (every ToolRunner::run records through it); the registry is not wired into anything, so the side-effect declaration that is supposed to gate tools by tier gates nothing. Defensible for a null-model phase, but it means Phase 1's Mode A loop will be the first thing that ever populates it, and the abstraction is unproven until then.

- **Only Capability::ConfigurationSweep is ever set on a target, and Capabilities::require is never called in production**
  - `crates/binmap-build/src/project.rs`:26
  - requirement: §2.5 — a view a target cannot support is absent from the rail; F0.4 per-section size
  - The engine already reads section tables for every artifact (measure/src/size.rs:26), yet no target ever declares SizeAttribution, so View::Size (gui/state/mod.rs:52) can never appear in the nav rail even once a Size view is built. The capability declaration and what the engine can actually do have already diverged in the direction that hides working functionality.

- **bloaty::disagreement and bloaty::sections are called only from bloaty.rs's own test, contradicting the comment that claims the engine reaches for bloaty**
  - `crates/binmap-measure/src/bloaty.rs`:58
  - requirement: TOOLING §4.1 — "If your total attributed bytes disagree with bloaty's by more than a small margin, your attribution is wrong. Make that a test."
  - TOOLING asks for a test and a test exists, so the letter is met — but the oracle only ever validates one ELF file that Binmap did not produce under any swept configuration. Stripped, LTO'd and panic=abort binaries are precisely where section attribution is most likely to disagree, and none of them is ever cross-checked. The environment-panel copy also overstates what the engine does with the tool it tells the user to install.

- **TargetMetadata::of hardcodes commit: None, dirty: false and root: None, and nothing ever sets them**
  - `crates/binmap-session/src/artifact.rs`:34
  - requirement: F0.7; artifact.rs:22-25 — "A number measured on an uncommitted tree cannot be reproduced by anyone else, and the artifact says so rather than implying otherwise"
  - Every exported binmap.json claims `dirty: false` and names no commit, so an artifact measured on a dirty tree looks reproducible and a recipient has no way to tell. The field exists precisely to stop that, and the data it needs is already being gathered a few files away.

- **The session store cannot enumerate what was opened before, so the first-run welcome step's Recent table has no rows**
  - `crates/binmap-session/src/store/mod.rs`:31
  - requirement: First run step 0 — "no project open with recents" (plan §4, phase 0; U0.1)
  - The first thing a returning user sees is a list of what they last worked on. Nothing records that a project was opened, where it lived, or when — so the panel would render empty on every launch.

- **Target carries neither the artifact path nor whether it is a bin or a lib, so the target cards cannot show either**
  - `crates/binmap-core/src/traits.rs`:18
  - requirement: Target view — targets grouped by language with their capability line (plan §4, phase 0; U1, F0.1)
  - `Capabilities::sentence()` fills the capability line well, but the two cards the design distinguishes — a bin and a lib in the same Rust group — are indistinguishable to the interface, and neither can name the file it is about to measure.

- **Nothing parses the project's existing [profile.release], so the configure step's "Baseline profile" line and the "(current)" row label must be guessed**
  - `crates/binmap-build/src/engine/manifest.rs`:26
  - requirement: First run step 2, "Baseline profile" (plan §4, phase 0; U0.1)
  - A project with a customised [profile.release] — which is most projects that would reach for Binmap — would be shown cargo's defaults presented as its own settings. That is a wrong number delivered confidently, in the step where the user decides whether to trust the sweep.

- **Criterion is never detected or used; a project with existing benches gets a new harness imposed on it**
  - `crates/binmap-core/src/config.rs`:79
  - requirement: TOOLING §4.2 ("When a project already has Criterion benches, use them rather than imposing a new harness")
  - On a crate that already has calibrated Criterion benches with confidence intervals, Binmap will either have no runtime objective at all or ask the user to invent a command — discarding the statistically better measurements the study says to prefer. Deferrable behind the hyperfine wiring, but nothing in the code records the decision.

- **No option to share one target directory: the per-configuration split is unconditional, with no flag to trade disk for rebuild thrashing**
  - `crates/binmap-build/src/cargo.rs`:48
  - requirement: TOOLING §3.2 ("Use a separate CARGO_TARGET_DIR per configuration to avoid rebuild thrashing, at the cost of disk — offer a flag to trade one for the other")
  - The default matrix is 96 configurations (`SweepMatrix::default().cardinality() == 96`, asserted at config.rs:287). Ninety-six full target directories is tens of gigabytes on a real crate, and a user on a small SSD has no way to choose the other trade. The study explicitly names the flag as the mitigation.

- **The integration test asserting each sweep axis actually changes the output does not exist, though a doc comment says it does**
  - `crates/binmap-build/src/cargo.rs`:105
  - requirement: TOOLING §8 ("Cargo profile env vars · Medium fragility · Mitigation: integration test asserting each axis actually changes the output")
  - The exact failure the study warns about — a variable name cargo stops honouring — would produce a sweep where every configuration builds identically and the frontier is noise. Nothing in the suite would notice, and the comment tells a future reader that something already does.

- **The Environment panel is missing the two Project-group probes §9 and the design both render: workspace composition and symbol-mangling-version**
  - `crates/binmap-build/src/environment/mod.rs`:27
  - requirement: TOOLING §9 (the Environment panel's rendering) and §3.4; PRD U10/U0.4
  - The v0 row is the one probe with an in-app action the design specifies ("Enable for analysis builds"), and it is the check that decides whether Phase 1's generic grouping will work at all — TOOLING §2.4 says a legacy-mangled crate degrades to prefix matching and the tool must say so rather than silently producing worse results. Discovering that at the start of Phase 1 is exactly the "worse moment" §9 exists to avoid.

- **redact_secrets treats any line containing '=' or ':' whose prefix contains 'auth'/'token'/'secret' as a credential, so it redacts line numbers out of rustc diagnostics for files like src/auth.rs**
  - `crates/binmap-session/src/redact.rs`:167
  - requirement: U13; redact.rs:151 — "Replace the value in any KEY=value or \"key\": \"value\" pair whose key looks like a credential"
  - Source files named auth.rs, token.rs, credentials.rs, password.rs are common, and every compiler diagnostic pointing at one loses its line and column in the exported evidence record. The redactor advertises that it errs toward over-redacting, but silently corrupting the location out of a diagnostic makes the exported evidence unusable for the exact bug report the export exists to support.

- **The redaction pass rebuilds every string through lines().join("\n"), so it strips CR and drops a trailing blank line even when nothing was redacted**
  - `crates/binmap-session/src/redact.rs`:189
  - requirement: U13; redact.rs:10 — "it reports every substitution it made so nothing is quietly different from what the tool saw"
  - This is exactly the "quietly different" the module promises not to do: the record is altered, the RedactionReport counts zero substitutions, and the digest no longer matches — an evidence record that had nothing to hide is still marked tampered on import.

- **An empty quoted secret value produces malformed output with unbalanced quotes**
  - `crates/binmap-session/src/redact.rs`:176
  - requirement: redact.rs:174 — "Keep the key and the shape of the line; replace only the value"
  - The pass claims to preserve the line's shape and instead emits syntactically broken text into an evidence record a colleague may try to parse.

- **The benchmark pass emits Progress with a 0-based index into `measured`, so the progress counter jumps backwards after the build pass**
  - `crates/binmap-build/src/sweep/mod.rs`:458
  - requirement: crates/binmap-core/src/event.rs:41 — "Progress: One step finished. `completed` counts steps, not percent; the interface decides how to render it"; Started carries `total: Some(planned.len())`
  - The two passes emit `completed` in different units against the same `total`, so any designed progress view built on Started.total will run to 100% and then snap back to 0% and climb again. The engine gives the view no way to tell the two phases apart.

- **Two concurrent ResumeSweep requests for one run both clone the same state, rebuild everything, and duplicate the findings; the last writer wins**
  - `crates/binmap-build/src/engine/mod.rs`:164
  - requirement: F0.8 (a cancelled sweep resumes and skips what it already measured)
  - Nothing in the engine makes resume idempotent or exclusive. Beyond the doubled build cost, the interface's finding list gains duplicate ids, and whichever thread finishes second silently discards the other's measurements when it inserts its own clone of the state.

## nit (11)

- **`expansion_produces_exactly_the_matrix_cardinality` compares two spellings of the same `max(1)` product, one of which the other is allocated from**
  - `crates/binmap-core/src/configuration.rs`:279
  - requirement: F0.3 (the 96-configuration matrix)
  - It reads as an independent cross-check of the matrix size and is not one. The assertion that does carry weight is `config.rs:280` pinning cardinality to the literal 96; this one mostly adds confidence that isn't earned.

- **`every_failing_probe_carries_a_command_not_a_description` allowlists the exact prose it exists to forbid, and carries a dead entry from another project**
  - `crates/binmap-build/src/environment/mod.rs`:337
  - requirement: U0.4 / TOOLING §9: "every warning names the exact fix, as a command, not a description"
  - The one violation in the module was accommodated by widening the allowlist rather than fixed, so the test now certifies the opposite of its name. The dead "vite" entry suggests the list was copied rather than derived from the probes it guards.

- **The binmap-measure ban lists `binmap` as a wrapper, but binmap does not depend on binmap-measure — cargo-deny would emit an UnusedWrapper warning**
  - `deny.toml`:28
  - requirement: §2.4, as encoded in deny.toml
  - Harmless on its own — a warning, not an error — but it is direct evidence that this file has never been executed against the actual graph. Combined with the wildcards failure above, deny.toml should be treated as unverified rather than as an enforced constraint. Dropping "binmap" from that one list makes the file's model of the graph exact.

- **There is no `binmap-gui::widgets` module, and the spike's custom-painted views live directly in examples/**
  - `crates/binmap-gui/src/lib.rs`:16
  - requirement: §2.2 rule 2 ("every custom widget in binmap-gui::widgets, so an API change touches one module")
  - No violation yet, since no shipped custom widget exists. Raising it only because the rule's whole value is that it must be in place *before* the first widget is written — once the treemap, the Pareto scatter and the flamegraph are painted, retrofitting the module boundary costs more than creating an empty `src/widgets/mod.rs` now. Note also that the spike is committed and built by `cargo test --workspace` despite being declared throwaway, so a gpui-kit upgrade will require fixing 750 lines of code nobody intends to keep.

- **Gate::Builds::describes() says it checks warnings, which NoNewWarnings was split out to do**
  - `crates/binmap-core/src/gate.rs`:83
  - requirement: gate.rs:13-17; design gate rows at Binmap v3.dc.html:1714
  - `describes()` is the string the gate tally is meant to show for each row; as written, the Builds row would tell the user it checks something the row below it checks, which is the confusion the split was made to remove. It is unused now, so it will be wired up unread.

- **A configuration whose size measurement failed stays eligible with a size of u64::MAX and can land on the frontier**
  - `crates/binmap-build/src/sweep/mod.rs`:90
  - requirement: F0.4/F0.6
  - A frontier point with no known size is presented as a measured trade-off. Treating an unmeasured size the way the code already treats an unmeasured runtime — as a reason not to compare — would be consistent with the module's own stated rule.

- **ProjectConfig.test_command is never read; both binaries hardcode `cargo test --release --quiet`**
  - `crates/binmap-core/src/config.rs`:229
  - requirement: binmap-verify/src/lib.rs:59 — "The user's test command where they declared one; `cargo test` otherwise"
  - A project whose suite is not `cargo test` (a workspace needing `--workspace`, a nextest runner, a required feature flag) silently gets the wrong gate, and there is no configuration path that could correct it since nothing reads a config file into ProjectConfig.

- **CargoBuildSystem::reproduction_command has no caller outside cargo.rs's own test, and no facade method could deliver it**
  - `crates/binmap-build/src/cargo.rs`:78
  - requirement: U0.3 / DESIGN — the Profile Lab shows the command a user retypes to reproduce a point
  - The reproducibility affordance is computed by a method that nothing can reach: the sweep does not record it per configuration and the facade cannot return it, so wiring the Profile Lab to it will require plumbing through both layers rather than just calling it.

- **Gate details are sentences where the design's GateResult expects a measurement, so the measurement column has no figure to show for Builds or TestsPass**
  - `crates/binmap-verify/src/lib.rs`:302
  - requirement: Profile Lab GateResult rows and the Inspector's verification run (design passGates literal)
  - `GateOutcome` already has the right three slots — detail, caveat and parameters — and SizeNotWorse and BenchmarkNotWorse fill them properly. Builds and TestsPass are the two that produce a sentence where a figure belongs, so those rows read differently from the rest of the same list, and the test count the design shows is never parsed out of cargo's output at all.

- **Measurement probes report a version but not the tool's path, which is what the panel's detail column shows**
  - `crates/binmap-build/src/environment/mod.rs`:113
  - requirement: TOOLING §9; design envGroups literal
  - When a machine has two hyperfines or a stale cargo-installed bloaty on PATH, the version alone does not tell the user which one Binmap measured with — which is the question the path column answers.

- **§2.1's ELF symbol-size caveat has no home in the code, and the section reader's doc claims multi-format coverage it is only incidentally granted**
  - `crates/binmap-measure/src/sections.rs`:4
  - requirement: TOOLING §2.1 (the symbol-size caveat and the cross-check argument it feeds)
  - Deliberately deferred — symbol attribution is Phase 1 (F1.1) — but the caveat is the premise of §4.1's bloaty oracle, and the person who writes the symbol reader will be doing it in a codebase where the trap is not written down anywhere.
