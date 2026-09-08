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
use binmap_core::facade::{Engine, Probe, Proposal, Request};
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
        let builder = CargoBuildSystem::new(runner.clone(), config.target_directory.clone());
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
                next_run: AtomicU64::new(0),
                sessions,
            }),
        })
    }

    pub fn evidence_store(&self) -> &EvidenceStore {
        self.inner.runner.store()
    }

    pub fn trust_tier(&self) -> TrustTier {
        self.inner.config.read().expect("config poisoned").trust_tier
    }

    /// Raise or lower the tier. Always a deliberate act by the user, which is
    /// why it is a method and never a side effect of anything else.
    pub fn set_trust_tier(&self, tier: TrustTier) {
        self.inner.config.write().expect("config poisoned").trust_tier = tier;
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

        let mut adopted = 0;
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

        let sweep = Sweep {
            builder: &self.builder,
            runner: &self.runner,
            benchmark: self.benchmark.as_deref(),
            options: SweepOptions::new(gates).with_parallelism(parallelism),
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

    /// Write everything this session knows to disk.
    fn persist(&self, target: &Target) -> Result<()> {
        let mut artifact = SessionArtifact::new(self.metadata_for(target))
            .with_findings(self.findings.lock().expect("findings poisoned").clone())
            .with_evidence(self.runner.store().records())
            .with_gates(self.gate_reports());

        // Run state travels as opaque JSON: its shape is this crate's
        // business, and the interface must not learn it.
        for (id, state) in self.runs.lock().expect("runs poisoned").iter() {
            artifact = artifact.with_run(id.to_string(), "sweep", state)?;
        }

        self.sessions.save(&artifact)?;
        Ok(())
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
            Request::Verify { proposal } | Request::Apply { proposal } => {
                return Err(Error::Other(format!(
                    "`{proposal}` cannot be verified or applied yet: Phase 0 proposes \
                     configurations and shows their diffs, and applying one arrives with the \
                     apply dialog"
                )));
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
