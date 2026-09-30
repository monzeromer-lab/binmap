//! The concrete engine: the thing behind the facade.
//!
//! It lives here rather than in `binmap-gui` because the interface may not
//! compute (§2.4). The shipped binary constructs one of these and hands the
//! interface an `Arc<dyn Engine>`; the interface never names this type.
//!
//! The struct is a handle around a shared inner, which is what lets a run
//! detach onto a background thread while the caller keeps using the engine.
//! Nothing here blocks a caller, and nothing here paints.

pub mod manifest;

use crate::cargo::CargoBuildSystem;
use crate::sweep::{Sweep, SweepOptions, SweepState};
use binmap_core::config::{ProjectConfig, TrustTier};
use binmap_core::configuration::BuildConfiguration;
use binmap_core::error::{Error, Result};
use binmap_core::event::{Cancellation, EngineEvent, EventSink, RunId};
use binmap_core::evidence::{Evidence, EvidenceStore};
use binmap_core::facade::{Engine, Measurement, Probe, Proposal, Request, SweepSummary};
use binmap_core::finding::Finding;
use binmap_core::tool::ToolRunner;
use binmap_core::traits::{BuildSystem, MeasurementSource, Target};
use binmap_measure::HyperfineBenchmark;
use binmap_session::SessionStore;
use binmap_session::artifact::{SessionArtifact, TargetMetadata};
use binmap_verify::GatePlan;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};

/// One project, open.
///
/// Cheap to clone: every clone shares one evidence store, one set of findings
/// and one set of runs, which is what makes it safe to hand the same engine to
/// a view and to a background thread.
#[derive(Clone)]
pub struct BinmapEngine {
    inner: Arc<Inner>,
}

struct Inner {
    config: RwLock<ProjectConfig>,
    runner: ToolRunner,
    builder: CargoBuildSystem,
    benchmark: Option<Arc<dyn MeasurementSource>>,
    gates: GatePlan,
    targets: RwLock<Vec<Target>>,
    findings: Mutex<Vec<Finding>>,
    proposals: Mutex<Vec<Proposal>>,
    /// Sweep state per run, so a cancelled sweep can be resumed and a finished
    /// one can be written into the session artifact.
    runs: Mutex<BTreeMap<RunId, SweepState>>,
    /// The most recent size attribution, for the Size Explorer to read.
    attribution: Mutex<Option<binmap_core::attribution::Attribution>>,
    /// The most recent crash, for the Stack Pane to read.
    crash: Mutex<Option<binmap_core::crash::CrashReport>>,
    next_run: AtomicU64,
    /// Where sessions live.
    ///
    /// Until an audit found it, nothing in production ever wrote one:
    /// `SessionStore` had no caller outside its own tests, so `F0.7`'s
    /// persistence and `F0.8`'s resume-after-exit were both unreachable
    /// however complete the types looked.
    sessions: SessionStore,
}

impl BinmapEngine {
    /// Open a project. Discovery runs here, so a caller holding an engine
    /// already knows what the project offers.
    ///
    /// If the project declares a benchmark and hyperfine is installed, runtime
    /// becomes an objective; otherwise it does not, and the frontier ranks on
    /// what was actually measured. We never invent a workload.
    pub fn open(mut config: ProjectConfig, gates: GatePlan) -> Result<Self> {
        // The project's own settings, if it has any. F0.2's "configurable".
        crate::settings::apply(&mut config)?;
        let benchmark = config.benchmark.clone().and_then(|command| {
            let runner = ToolRunner::new(EvidenceStore::new(), config.root.clone());
            HyperfineBenchmark::new(runner, command)
                .map(|b| Arc::new(b) as Arc<dyn MeasurementSource>)
        });
        Self::open_with(config, gates, benchmark)
    }

    /// Open a project with the user's benchmark attached. Without one, runtime
    /// is simply not an objective — never a guessed one.
    pub fn open_with(
        config: ProjectConfig,
        gates: GatePlan,
        benchmark: Option<Arc<dyn MeasurementSource>>,
    ) -> Result<Self> {
        // Beside our own build products, never in the user's tree.
        let sessions = SessionStore::new(config.target_directory.join("sessions"));
        let runner = ToolRunner::new(EvidenceStore::new(), config.root.clone());
        let builder = CargoBuildSystem::new(runner.clone(), config.target_directory.clone())
            .with_profile(config.profile.clone());
        let targets = builder.targets(&config.root)?;

        Ok(Self {
            inner: Arc::new(Inner {
                config: RwLock::new(config),
                runner,
                builder,
                benchmark,
                gates,
                targets: RwLock::new(targets),
                findings: Mutex::new(Vec::new()),
                proposals: Mutex::new(Vec::new()),
                runs: Mutex::new(BTreeMap::new()),
                attribution: Mutex::new(None),
                crash: Mutex::new(None),
                next_run: AtomicU64::new(0),
                sessions,
            }),
        })
    }

    pub fn evidence_store(&self) -> &EvidenceStore {
        self.inner.runner.store()
    }

    /// The state of one run, for the session artifact.
    pub fn run_state(&self, run: &RunId) -> Option<SweepState> {
        self.inner.runs.lock().expect("runs poisoned").get(run).cloned()
    }

    pub fn run_states(&self) -> Vec<(RunId, SweepState)> {
        self.inner
            .runs
            .lock()
            .expect("runs poisoned")
            .iter()
            .map(|(id, state)| (id.clone(), state.clone()))
            .collect()
    }

    /// Adopt a sweep a previous session left unfinished, so
    /// [`Request::ResumeSweep`] has something to resume.
    pub fn adopt_run(&self, state: SweepState) {
        self.inner.runs.lock().expect("runs poisoned").insert(state.run.clone(), state);
    }

    /// Read back what a previous session on this target measured.
    ///
    /// Returns the number of runs adopted. Findings arrive already
    /// revalidated: import re-runs the grounding check, so a hand-edited
    /// artifact cannot inject a claim that cites evidence nobody issued.
    pub fn restore(&self, target_id: &str) -> Result<usize> {
        let Some(outcome) = self.inner.sessions.load(target_id)? else {
            return Ok(0);
        };

        self.inner.runner.store().adopt(outcome.artifact.evidence.clone());
        self.inner.findings.lock().expect("findings poisoned").extend(outcome.findings);
        if outcome.artifact.attribution.is_some() {
            *self.inner.attribution.lock().expect("attribution poisoned") =
                outcome.artifact.attribution.clone();
        }

        let mut adopted = usize::from(outcome.artifact.attribution.is_some());
        for record in &outcome.artifact.runs {
            if record.kind != "sweep" {
                continue;
            }
            if let Some(Ok(state)) = outcome.artifact.run_state::<SweepState>(&record.id) {
                self.inner.runs.lock().expect("runs poisoned").insert(state.run.clone(), state);
                adopted += 1;
            }
        }
        Ok(adopted)
    }

    /// Write a proposal into the working tree.
    ///
    /// The one place in Phase 0 that touches a file the user owns, and it is
    /// gated three ways:
    ///
    /// - The tier must be at least `Tune`. Below it the write is refused with
    ///   the tier it would need, not silently skipped (`U9`).
    /// - The proposal must be one we produced. Applying an id we never issued
    ///   is the same class of mistake as citing evidence we never issued.
    /// - The configuration it came from must have passed its gates. Writing a
    ///   configuration whose tests failed is the exact thing the gates exist
    ///   to prevent, however good its size looked.
    ///
    /// The write is recorded as evidence, so what was changed and when is on
    /// the same record as everything else.
    pub fn apply(&self, proposal_id: &str) -> Result<String> {
        let tier = self.inner.config.read().expect("config poisoned").trust_tier;
        tier.require(TrustTier::Tune, "write [profile.release] into Cargo.toml")?;

        let proposal = self
            .inner
            .proposals
            .lock()
            .expect("proposals poisoned")
            .iter()
            .find(|p| p.id == proposal_id)
            .cloned()
            .ok_or_else(|| {
                Error::Other(format!("`{proposal_id}` is not a proposal this session produced"))
            })?;

        // The configuration behind it must have passed. A proposal is only as
        // good as the measurement it came from.
        let name = proposal_id.strip_prefix("apply-").unwrap_or(proposal_id);
        let rejected = self
            .inner
            .runs
            .lock()
            .expect("runs poisoned")
            .values()
            .flat_map(|state| state.measured.iter())
            .find(|m| m.name == name)
            .and_then(|m| m.report.rejected_by());
        if let Some(gate) = rejected {
            return Err(Error::Other(format!(
                "`{name}` was rejected by {gate} and will not be written"
            )));
        }

        let manifest = proposal
            .writes
            .first()
            .ok_or_else(|| Error::Other(format!("`{proposal_id}` writes nothing")))?;

        let before =
            std::fs::read_to_string(manifest).map_err(|source| Error::io(manifest, source))?;
        let configuration = self.configuration_named(name).ok_or_else(|| {
            Error::Other(format!("the configuration behind `{proposal_id}` is no longer known"))
        })?;
        let after = manifest::with_release_profile(&before, &configuration);

        // Recorded before the write, like every other tool invocation.
        let pending = self.inner.runner.store().begin(binmap_core::evidence::ToolInvocation::new(
            "binmap:apply",
            [proposal_id.to_string(), manifest.display().to_string()],
        ));
        match std::fs::write(manifest, &after) {
            Ok(()) => {
                self.inner.runner.store().complete(
                    pending,
                    manifest::unified_diff(manifest, &before, &after),
                    0,
                );
            }
            Err(source) => {
                self.inner.runner.store().complete(
                    pending,
                    format!("failed to write: {source}"),
                    -1,
                );
                return Err(Error::io(manifest, source));
            }
        }

        Ok(format!("Wrote {} into {}", configuration.describe(), manifest.display()))
    }

    /// The configuration one of this session's measurements was built under.
    fn configuration_named(&self, name: &str) -> Option<BuildConfiguration> {
        self.inner
            .runs
            .lock()
            .expect("runs poisoned")
            .values()
            .flat_map(|state| state.measured.iter())
            .find(|m| m.name == name)
            .map(|m| m.configuration.clone())
    }

    /// Write the session now. Exposed for tests; production persists at the
    /// end of every run.
    #[doc(hidden)]
    pub fn persist_for_test(&self, target: &Target) -> Result<()> {
        self.inner.persist(target)
    }

    /// Export this session for someone else to read, with the redaction pass
    /// applied (`U13`).
    pub fn export(&self, target: &Target, path: &std::path::Path) -> Result<String> {
        let mut artifact = SessionArtifact::new(self.inner.metadata_for(target))
            .with_findings(self.findings())
            .with_evidence(self.inner.runner.store().records())
            .with_gates(self.inner.gate_reports());
        for (id, state) in self.inner.runs.lock().expect("runs poisoned").iter() {
            artifact = artifact.with_run(id.to_string(), "sweep", state)?;
        }
        Ok(self.inner.sessions.export(&artifact, path)?.describe())
    }

    /// Run a sweep to completion on the calling thread.
    ///
    /// The headless harness uses this; the interface never does, because it
    /// would block a frame on ninety-six builds.
    pub fn sweep_blocking(
        &self,
        run: RunId,
        target: &Target,
        events: &dyn EventSink,
        cancellation: &Cancellation,
    ) {
        self.inner.run_sweep(run, target, events, cancellation);
    }

    pub fn target_by_id(&self, id: &str) -> Result<Target> {
        self.inner.target_by_id(id)
    }
}

impl Inner {
    /// Whether this project permits a cloud model (`DESIGN-AI §6.3`).
    fn allow_cloud_models(&self) -> bool {
        self.config.read().expect("config poisoned").allow_cloud_models
    }

    /// Analyse a core dump against a target's binary (`F2.1`–`F2.8`).
    ///
    /// Refuses before symbolizing rather than after: a binary that did not
    /// produce this core yields a stack of real-looking symbols at
    /// real-looking lines, every one of them wrong, and nothing in the output
    /// would say so.
    fn analyse_crash(
        &self,
        target: &Target,
        core: &std::path::Path,
    ) -> Result<binmap_core::crash::CrashReport> {
        use binmap_crash::{CoreDump, bias, classify, correspondence, modules, symbolize, unwind};

        let artifact = self
            .builder
            .build(target, &BuildConfiguration::default_release())?
            .artifact
            .ok_or_else(|| {
                Error::Other(format!("{} did not produce a binary to compare against", target.name))
            })?;

        let core_data = std::fs::read(core).map_err(|source| Error::io(core, source))?;
        let binary = std::fs::read(&artifact).map_err(|source| Error::io(&artifact, source))?;

        let dump = CoreDump::parse(&core_data)?;
        let thread = dump.crashing_thread();
        let path = artifact.display().to_string();

        let verdict = correspondence::verify(&dump, &core_data, &binary, &path)?;
        if !verdict.permits_symbolization() {
            return Err(Error::Other(verdict.describe()));
        }
        let derived = bias::derive(&dump, &binary)?;

        let name =
            artifact.file_name().map(|name| name.to_string_lossy().to_string()).unwrap_or_default();
        let loaded = modules::Modules::load(&dump, Some((&name, &binary)));
        let stack = unwind::walk(
            &binmap_crash::memory::CoreMemory { dump: &dump, data: &core_data },
            &loaded,
            &thread.registers,
        )?;

        // One symbolizer per module, over every frame that landed in it.
        let mut by_module: BTreeMap<&str, Vec<u64>> = BTreeMap::new();
        for frame in &stack.frames {
            if let Some((module, address)) = frame.module.as_deref().zip(frame.link_time_address) {
                by_module.entry(module).or_default().push(address);
            }
        }
        let symbolizers: BTreeMap<&str, symbolize::Symbolizer> = by_module
            .iter()
            .filter_map(|(module, addresses)| {
                symbolize::Symbolizer::load(std::path::Path::new(module), addresses)
                    .ok()
                    .map(|symbolizer| (*module, symbolizer))
            })
            .collect();

        let resolved: Vec<symbolize::Resolved> = stack
            .frames
            .iter()
            .map(|frame| {
                frame
                    .module
                    .as_deref()
                    .zip(frame.link_time_address)
                    .and_then(|(module, address)| {
                        symbolizers.get(module).map(|symbolizer| symbolizer.resolve(address))
                    })
                    .unwrap_or_default()
            })
            .collect();

        let crash = classify::classify(&dump, thread, &resolved);
        Ok(binmap_crash::report::assemble(thread, &stack, &resolved, &crash, &verdict, &derived))
    }

    /// Drive one reasoning session, relaying its transcript as it goes
    /// (`A1.1`, `U1.3`).
    ///
    /// The direction here is the one `§2.4` insists on and that is easy to get
    /// backwards: orchestration reaches into the model layer, and the model
    /// layer never reaches back into an analysis. The question came from the
    /// interface; what answers it is decided here.
    fn run_reasoning(
        &self,
        run: RunId,
        target: &Target,
        question: &str,
        reasoner: &str,
        events: &dyn EventSink,
    ) {
        use binmap_agent::backend::ModelBackend;

        events.emit(EngineEvent::Started {
            run: run.clone(),
            description: format!("Asking {reasoner} about {}", target.name),
            total: None,
        });

        let registry = binmap_agent::Registry::phase_one();
        let mut gate = binmap_agent::Gate::new();
        let config = binmap_agent::AgentConfig::default();

        // Build the backend the chosen row describes. "none" is a legitimate
        // choice rather than an error, and it is the default.
        let backend: Box<dyn ModelBackend> = match self.backend_for(reasoner) {
            Ok(backend) => backend,
            Err(error) => {
                events.emit(EngineEvent::Failed { run, error: error.to_string() });
                return;
            }
        };

        let mut session = binmap_agent::Session {
            backend: backend.as_ref(),
            registry: &registry,
            gate: &mut gate,
            store: self.runner.store(),
            config,
            tier: self.config.read().expect("config poisoned").trust_tier,
        };
        let outcome = session.run(question);

        // Relay the transcript on the same stream as everything else, so the
        // panel is updated by the mechanism that already updates every view.
        for event in outcome.transcript.events() {
            events
                .emit(EngineEvent::Transcript { run: run.clone(), event: Box::new(event.clone()) });
        }

        // Findings are persisted before the terminal event, as everywhere else:
        // a harness that exits on the terminal event would otherwise lose them.
        if !outcome.findings.is_empty() {
            let mut findings = self.findings.lock().expect("findings poisoned");
            for finding in &outcome.findings {
                events.emit(EngineEvent::Finding {
                    run: run.clone(),
                    finding: Box::new(finding.clone()),
                });
                findings.push(finding.clone());
            }
        }

        // A session that failed is a failed run, and must not leave on
        // `Finished`. The same hole in the headless harness made a refused
        // sweep exit 0, which in CI is indistinguishable from success.
        //
        // Budget exhaustion is *not* a failure: the session did its job and
        // said what it believed, which §8 is explicit is a useful answer.
        match &outcome.stop_reason {
            binmap_core::transcript::StopReason::Failed { error } => {
                events.emit(EngineEvent::Failed { run, error: error.clone() })
            }
            _ => events.emit(EngineEvent::Finished { run, summary: outcome.summary() }),
        }
    }

    /// The backend a reasoner id names.
    fn backend_for(&self, reasoner: &str) -> Result<Box<dyn binmap_agent::backend::ModelBackend>> {
        if reasoner == "none" || reasoner.is_empty() {
            return Ok(Box::new(binmap_agent::NullBackend::new()));
        }
        let (provider_id, model) = reasoner.split_once('/').ok_or_else(|| {
            Error::Other(format!(
                "`{reasoner}` is not a reasoner id; they look like `local/qwen3-coder`"
            ))
        })?;
        let spec = binmap_agent::provider::provider(provider_id)
            .ok_or_else(|| Error::Other(format!("there is no provider `{provider_id}`")))?;
        if spec.cloud && !self.allow_cloud_models() {
            return Err(Error::Other(format!(
                "{} is a cloud provider and this project does not allow cloud models",
                spec.display
            )));
        }
        binmap_agent::backend::backend_for(
            spec,
            model,
            std::sync::Arc::new(binmap_agent::UreqTransport::new()),
        )
    }

    fn target_by_id(&self, id: &str) -> Result<Target> {
        self.targets
            .read()
            .expect("targets poisoned")
            .iter()
            .find(|target| target.id == id)
            .cloned()
            .ok_or_else(|| Error::Other(format!("no target `{id}` in this project")))
    }

    /// A run identifier nothing else is using.
    ///
    /// The counter restarts at zero each process, so after a restored session
    /// adopted `run-0001` the next sweep minted `run-0001` too — and, finding
    /// state under that key, resumed the old run instead of starting a new
    /// one. Skipping past what is already there is the fix, and it is cheap
    /// because runs are few.
    fn mint_run(&self) -> RunId {
        loop {
            let candidate =
                RunId(format!("run-{:04}", self.next_run.fetch_add(1, Ordering::SeqCst) + 1));
            if !self.runs.lock().expect("runs poisoned").contains_key(&candidate) {
                return candidate;
            }
        }
    }

    fn run_sweep(
        &self,
        run: RunId,
        target: &Target,
        events: &dyn EventSink,
        cancellation: &Cancellation,
    ) {
        let mut state =
            self.runs.lock().expect("runs poisoned").get(&run).cloned().unwrap_or_else(|| {
                let matrix = self.config.read().expect("config poisoned").matrix.clone();
                SweepState::new(run.clone(), target.id.clone(), BuildConfiguration::expand(&matrix))
            });

        let parallelism = self.config.read().expect("config poisoned").parallelism;
        let keep_build_directories =
            self.config.read().expect("config poisoned").keep_build_directories;

        // Give the harness a sanitizer if the machine has one. Without this,
        // MiriClean reported "no sanitizer is available" on machines that had
        // just been probed and found one.
        let mut gates = self.gates.clone();
        if gates.miri.is_none() && self.runner.is_available("cargo-miri") {
            gates = gates.sanitizing_with(binmap_core::evidence::ToolInvocation::new(
                "cargo",
                ["miri", "test", "--quiet"],
            ));
        }

        // And tell it whether there is any unsafe to check. Nothing set this,
        // so MiriClean reported "the change does not touch unsafe" for every
        // project including ones full of it — and the FFI caveat, the sentence
        // §6 says the interface must show, could never appear.
        let root = self.config.read().expect("config poisoned").root.clone();
        let unsafety = crate::unsafety::Unsafety::scan(&root);
        gates = gates.with_substantial_ffi(unsafety.foreign_calls);

        // A cross-compiled project's tests are built for a machine that is not
        // this one, so `cargo test` compiles them and then cannot run them.
        // Reporting that as a failing suite condemned every configuration of a
        // perfectly healthy crate — corpus/wasm swept ninety-six and passed
        // none of them.
        if let Some(target) = crate::project::default_target(&root)
            && !self.builder.host().is_some_and(|host| host == target)
        {
            gates = gates.without_tests(format!(
                "this project builds for {target}, so its tests cannot run on this machine. \
                 Declare a test command in binmap.toml if you have a runner for it."
            ));
        }

        let touches_unsafe = unsafety.present;
        let sweep = Sweep {
            builder: &self.builder,
            runner: &self.runner,
            benchmark: self.benchmark.as_deref(),
            options: SweepOptions::new(gates)
                .with_parallelism(parallelism)
                .keeping_build_directories(keep_build_directories)
                .touching_unsafe(touches_unsafe),
        };

        // Findings are kept as they stream, so a view opened mid-run has
        // something to show without waiting for the end.
        let collector = FindingCollector { inner: events, findings: Mutex::new(Vec::new()) };
        let outcome = sweep.run(target, &mut state, &collector, cancellation);

        self.findings
            .lock()
            .expect("findings poisoned")
            .extend(collector.findings.into_inner().expect("collector poisoned"));
        self.runs.lock().expect("runs poisoned").insert(run.clone(), state);

        // Persist before reporting, so a session that is interrupted a moment
        // later still has everything this run measured. A sweep the user
        // cannot resume after closing the window is a sweep they will simply
        // run again.
        // One terminal event per run. Emitting Failed for a save problem and
        // again for the sweep's own error broke that invariant, which the
        // interface relies on to stop showing progress exactly once.
        let saved = self.persist(target);
        match (outcome, saved) {
            (Err(error), _) => events.emit(EngineEvent::Failed { run, error: error.to_string() }),
            (Ok(()), Err(error)) => events.emit(EngineEvent::Failed {
                run,
                error: format!("the run finished but the session could not be saved: {error}"),
            }),
            (Ok(()), Ok(())) => {}
        }
    }

    /// What the environment probe learned about the working tree.
    ///
    /// The git probe already establishes both; the artifact was hardcoding
    /// `dirty: false` and discarding it.
    fn provenance_of_the_tree(&self) -> (Option<String>, bool) {
        let probes = crate::environment::probe_all(&self.runner);
        let Some(git) = probes.iter().find(|probe| probe.name == "git") else {
            return (None, false);
        };
        let dirty = git.detail.contains("uncommitted");
        let commit = git.detail.split(',').next().map(str::trim).filter(|c| !c.is_empty());
        (commit.map(str::to_string), dirty)
    }

    fn metadata_for(&self, target: &Target) -> TargetMetadata {
        let (commit, dirty) = self.provenance_of_the_tree();
        TargetMetadata::of(target)
            .at_commit(commit, dirty)
            .at_root(self.config.read().expect("config poisoned").root.clone())
    }

    /// Every gate report this session produced.
    fn gate_reports(&self) -> Vec<binmap_core::gate::VerificationReport> {
        self.runs
            .lock()
            .expect("runs poisoned")
            .values()
            .flat_map(|state| state.measured.iter().map(|m| m.report.clone()))
            .collect()
    }

    /// Attribute the artifact's bytes, keeping whatever it finds.
    fn run_attribution(
        &self,
        run: RunId,
        target: &Target,
        events: &dyn EventSink,
        cancellation: &Cancellation,
    ) {
        let analysis = crate::attribute::SizeAnalysis {
            builder: &self.builder,
            runner: &self.runner,
            own_crates: self.own_crates(),
        };

        let collector = FindingCollector { inner: events, findings: Mutex::new(Vec::new()) };
        match analysis.run(run.clone(), target, &collector, cancellation) {
            Ok(attribution) => {
                self.findings
                    .lock()
                    .expect("findings poisoned")
                    .extend(collector.findings.into_inner().expect("collector poisoned"));
                let summary = analysis.summary(&attribution);
                *self.attribution.lock().expect("attribution poisoned") = Some(attribution);

                // Persist before reporting, so a harness that exits on the
                // terminal event does not exit during the write.
                match self.persist(target) {
                    Ok(()) => events.emit(EngineEvent::Finished { run, summary }),
                    Err(error) => events.emit(EngineEvent::Failed {
                        run,
                        error: format!("the attribution finished but could not be saved: {error}"),
                    }),
                }
            }
            Err(Error::Cancelled) => {
                events.emit(EngineEvent::Cancelled { run, completed: 0 });
            }
            Err(error) => {
                events.emit(EngineEvent::Failed { run, error: error.to_string() });
            }
        }
    }

    /// The crates the user wrote.
    ///
    /// Read from the workspace members, because there is no marker in a symbol
    /// name for "mine" and guessing from the crate name would be wrong for
    /// anyone whose crate is called `serde`.
    fn own_crates(&self) -> Vec<String> {
        self.targets
            .read()
            .expect("targets poisoned")
            .iter()
            .map(|target| target.package.replace('-', "_"))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    /// Write everything this session knows to disk.
    fn persist(&self, target: &Target) -> Result<()> {
        let mut artifact = SessionArtifact::new(self.metadata_for(target))
            .with_findings(self.findings.lock().expect("findings poisoned").clone())
            .with_evidence(self.runner.store().records())
            .with_gates(self.gate_reports())
            .with_attribution(self.attribution.lock().expect("attribution poisoned").clone());

        // Run state travels as opaque JSON: its shape is this crate's
        // business, and the interface must not learn it.
        for (id, state) in self.runs.lock().expect("runs poisoned").iter() {
            artifact = artifact.with_run(id.to_string(), "sweep", state)?;
        }

        self.sessions.save(&artifact)?;
        Ok(())
    }
}

/// Turn a sweep's state into what the Profile Lab reads.
///
/// The frontier is derived here rather than stored, so it cannot drift from
/// the measurements it is derived from — and it is derived against the
/// machine's measured noise floor, so a difference the machine invented does
/// not decide which configuration is shown.
fn summarise(state: &SweepState) -> SweepSummary {
    let frontier: std::collections::BTreeSet<usize> = state.frontier().into_iter().collect();

    let measured = state
        .measured
        .iter()
        .enumerate()
        .map(|(index, m)| Measurement {
            id: m.name.clone(),
            flags: m.configuration.describe(),
            settings: m
                .configuration
                .settings()
                .into_iter()
                .map(|(axis, value)| (axis.to_string(), value))
                .collect(),
            size_bytes: m.size_bytes,
            size_delta: match (state.baseline_bytes, m.size_bytes) {
                (Some(baseline), Some(bytes)) => Some(bytes as i64 - baseline as i64),
                _ => None,
            },
            runtime_nanos: m.runtime_nanos,
            build_time_nanos: m.build_time_nanos,
            gates: m.report.clone(),
            on_frontier: frontier.contains(&index),
            built: m.built,
        })
        .collect();

    SweepSummary {
        run: state.run.clone(),
        target: state.target.clone(),
        baseline_bytes: state.baseline_bytes,
        noise_floor: state.noise_floor.as_ref().map(|floor| floor.relative),
        noise_floor_samples: state
            .noise_floor
            .as_ref()
            .map(|floor| floor.samples.len())
            .unwrap_or(0),
        measured,
        complete: state.complete,
    }
}

/// Passes events through while keeping the findings, so the engine can answer
/// [`Engine::findings`] for a view that opened after the run began.
struct FindingCollector<'a> {
    inner: &'a dyn EventSink,
    findings: Mutex<Vec<Finding>>,
}

impl EventSink for FindingCollector<'_> {
    fn emit(&self, event: EngineEvent) {
        if let EngineEvent::Finding { finding, .. } = &event {
            self.findings.lock().expect("collector poisoned").push((**finding).clone());
        }
        self.inner.emit(event);
    }
}

impl Engine for BinmapEngine {
    fn targets(&self) -> Result<Vec<Target>> {
        Ok(self.inner.targets.read().expect("targets poisoned").clone())
    }

    fn probe_environment(&self) -> Vec<Probe> {
        crate::environment::probe_all(&self.inner.runner)
    }

    fn start(&self, request: Request, events: Arc<dyn EventSink>) -> Result<(RunId, Cancellation)> {
        let cancellation = Cancellation::new();

        let (run, target) = match request {
            Request::Sweep { target } => (self.inner.mint_run(), self.inner.target_by_id(&target)?),
            Request::ResumeSweep { run } => {
                let state = self
                    .inner
                    .runs
                    .lock()
                    .expect("runs poisoned")
                    .get(&run)
                    .cloned()
                    .ok_or_else(|| Error::Other(format!("no sweep `{run}` to resume")))?;
                let target = self.inner.target_by_id(&state.target)?;
                (run, target)
            }
            Request::NoiseFloor { target } => {
                // The noise floor is measured as the baseline of any sweep, so
                // asking for it alone is a sweep with an empty matrix.
                let target = self.inner.target_by_id(&target)?;
                let run = self.inner.mint_run();
                self.inner.runs.lock().expect("runs poisoned").insert(
                    run.clone(),
                    SweepState::new(run.clone(), target.id.clone(), Vec::new()),
                );
                (run, target)
            }
            // Attribution is a single build and a read, so it detaches like a
            // sweep does — a large binary's symbol table takes long enough
            // that blocking the interface on it would be felt.
            Request::AttributeSize { target } => {
                let target = self.inner.target_by_id(&target)?;
                target.capabilities.require(binmap_core::Capability::SizeAttribution)?;

                let run = RunId(format!("size-{}", target.id.replace("::", "-")));
                let inner = Arc::clone(&self.inner);
                let detached = run.clone();
                let detached_cancellation = cancellation.clone();
                std::thread::Builder::new()
                    .name(format!("binmap-{run}"))
                    .spawn(move || {
                        inner.run_attribution(
                            detached,
                            &target,
                            events.as_ref(),
                            &detached_cancellation,
                        );
                    })
                    .map_err(|source| Error::Other(format!("could not start the run: {source}")))?;
                return Ok((run, cancellation));
            }
            // Applying is not a sweep: it is one write, it finishes in
            // milliseconds, and the user is waiting for the answer. It runs
            // here rather than being detached onto a thread.
            Request::Apply { proposal } => {
                let outcome = self.apply(&proposal);
                let run = RunId(format!("apply-{proposal}"));
                match outcome {
                    Ok(summary) => events.emit(EngineEvent::Finished { run: run.clone(), summary }),
                    Err(error) => events
                        .emit(EngineEvent::Failed { run: run.clone(), error: error.to_string() }),
                }
                return Ok((run, cancellation));
            }
            Request::Verify { proposal } => {
                return Err(Error::Other(format!(
                    "`{proposal}` cannot be verified on its own yet: a configuration is \
                     verified by the sweep that measured it, and its gate rows are already \
                     beside it"
                )));
            }
            // A reasoning session detaches like a sweep: a model call takes
            // seconds at best, and blocking the interface on one would be felt.
            //
            // This is the wiring `§2.4` describes in the direction it insists
            // on: orchestration reaches the model layer, and the model layer
            // never reaches back. The interface asked a question; what answers
            // it is decided here.
            // A crash is analysed rather than swept: it is one read of a file
            // the user already has, and it finishes in well under a second on
            // anything but an enormous core.
            Request::AnalyseCrash { target, core } => {
                let target = self.inner.target_by_id(&target)?;
                let run = RunId(format!("crash-{}", target.id.replace("::", "-")));
                match self.inner.analyse_crash(&target, &core) {
                    Ok(report) => {
                        let summary = format!("{} · {}", report.title, report.describe());
                        *self.inner.crash.lock().expect("crash poisoned") = Some(report);
                        events.emit(EngineEvent::Finished { run: run.clone(), summary });
                    }
                    Err(error) => events
                        .emit(EngineEvent::Failed { run: run.clone(), error: error.to_string() }),
                }
                return Ok((run, cancellation));
            }
            Request::Reason { target, question, reasoner } => {
                let target = self.inner.target_by_id(&target)?;
                let run = RunId(format!("reason-{}", target.id.replace("::", "-")));
                let inner = Arc::clone(&self.inner);
                let detached = run.clone();
                std::thread::Builder::new()
                    .name(format!("binmap-{run}"))
                    .spawn(move || {
                        inner.run_reasoning(
                            detached,
                            &target,
                            &question,
                            &reasoner,
                            events.as_ref(),
                        );
                    })
                    .map_err(|source| Error::Other(format!("could not start the run: {source}")))?;
                return Ok((run, cancellation));
            }
        };

        // Detached, so `start` returns as soon as the run is registered. The
        // shared inner outlives this call by construction.
        let inner = Arc::clone(&self.inner);
        let detached_run = run.clone();
        let detached_cancellation = cancellation.clone();
        std::thread::Builder::new()
            .name(format!("binmap-{run}"))
            .spawn(move || {
                inner.run_sweep(detached_run, &target, events.as_ref(), &detached_cancellation);
            })
            .map_err(|source| Error::Other(format!("could not start the run: {source}")))?;

        Ok((run, cancellation))
    }

    fn findings(&self) -> Vec<Finding> {
        self.inner.findings.lock().expect("findings poisoned").clone()
    }

    fn evidence(&self, finding: &str) -> Vec<Evidence> {
        let findings = self.inner.findings.lock().expect("findings poisoned");
        let Some(finding) = findings.iter().find(|f| f.id() == finding) else {
            return Vec::new();
        };
        finding.evidence().iter().filter_map(|id| self.inner.runner.store().get(id)).collect()
    }

    fn export_session(&self, target: &str) -> Result<(std::path::PathBuf, String)> {
        let target = self.target_by_id(target)?;

        let directory = self.inner.config.read().expect("config poisoned").target_directory.clone();
        let path = directory.join("sessions").join(format!(
            "{}.export.binmap.json",
            target.id.replace(|c: char| !c.is_ascii_alphanumeric() && c != '-', "_")
        ));

        let described = self.export(&target, &path)?;
        Ok((path, described))
    }

    fn benchmark_command(&self) -> Option<String> {
        self.inner.config.read().expect("config poisoned").benchmark.as_ref().map(|command| {
            std::iter::once(command.program.clone())
                .chain(command.arguments.iter().cloned())
                .collect::<Vec<_>>()
                .join(" ")
        })
    }

    fn restore_session(&self, target: &str) -> usize {
        // A session that cannot be read is worth saying so about, but it is
        // not worth refusing to open the project over.
        match self.restore(target) {
            Ok(runs) => runs,
            Err(error) => {
                tracing::warn!("could not restore the session for {target}: {error}");
                0
            }
        }
    }

    fn attribution(&self) -> Option<binmap_core::attribution::Attribution> {
        self.inner.attribution.lock().expect("attribution poisoned").clone()
    }

    fn sweeps(&self) -> Vec<SweepSummary> {
        self.inner.runs.lock().expect("runs poisoned").values().map(summarise).collect()
    }

    fn trust_tier(&self) -> TrustTier {
        self.inner.config.read().expect("config poisoned").trust_tier
    }

    fn set_trust_tier(&self, tier: TrustTier) {
        self.inner.config.write().expect("config poisoned").trust_tier = tier;
    }

    /// Every reasoner the table offers, as the picker reads them.
    ///
    /// `allow_cloud` comes from the project's own configuration, because `§6.3`
    /// lets a project forbid sending its code off the machine and that
    /// decision belongs to the project rather than to the picker.
    fn cores(&self) -> Vec<std::path::PathBuf> {
        let root = self.inner.config.read().expect("config poisoned").root.clone();
        // The three places a core actually is: beside the project, in a
        // `cores/` directory someone made, or under the target directory
        // where a test harness dropped it. Not a recursive walk — a project
        // with a large `target/` would take seconds to search and find
        // nothing.
        let mut found = Vec::new();
        for directory in [root.clone(), root.join("cores"), root.join("target")] {
            let Ok(entries) = std::fs::read_dir(&directory) else { continue };
            found.extend(
                entries
                    .filter_map(|entry| entry.ok())
                    .map(|entry| entry.path())
                    .filter(|path| path.is_file())
                    .filter(|path| {
                        let name = path.file_name().unwrap_or_default().to_string_lossy();
                        // `core`, `core.1234`, and anything ending `.core`,
                        // which covers what the kernel writes and what gdb's
                        // `generate-core-file` produces.
                        name == "core" || name.starts_with("core.") || name.ends_with(".core")
                    }),
            );
        }
        found.sort();
        found.dedup();
        found
    }

    fn crash(&self) -> Option<binmap_core::crash::CrashReport> {
        self.inner.crash.lock().expect("crash poisoned").clone()
    }

    fn reasoners(&self) -> Vec<binmap_core::reasoner::Reasoner> {
        binmap_agent::provider::reasoners(self.inner.allow_cloud_models())
    }

    fn proposals(&self) -> Vec<Proposal> {
        self.inner.proposals.lock().expect("proposals poisoned").clone()
    }

    fn propose_configuration(&self, configuration: &BuildConfiguration) -> Result<Proposal> {
        let manifest = self.inner.config.read().expect("config poisoned").root.join("Cargo.toml");

        let current =
            std::fs::read_to_string(&manifest).map_err(|source| Error::io(&manifest, source))?;
        let updated = manifest::with_release_profile(&current, configuration);

        let proposal = Proposal {
            id: format!("apply-{}", configuration.name()),
            finding: format!("frontier-{}", configuration.name()),
            summary: format!("Write {} into [profile.release]", configuration.describe()),
            diff: manifest::unified_diff(&manifest, &current, &updated),
            writes: vec![manifest],
        };
        self.inner.proposals.lock().expect("proposals poisoned").push(proposal.clone());
        Ok(proposal)
    }
}

#[cfg(test)]
mod tests;
