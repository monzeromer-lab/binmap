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
}

impl BinmapEngine {
    /// Open a project. Discovery runs here, so a caller holding an engine
    /// already knows what the project offers.
    pub fn open(config: ProjectConfig, gates: GatePlan) -> Result<Self> {
        Self::open_with(config, gates, None)
    }

    /// Open a project with the user's benchmark attached. Without one, runtime
    /// is simply not an objective — never a guessed one.
    pub fn open_with(
        config: ProjectConfig,
        gates: GatePlan,
        benchmark: Option<Arc<dyn MeasurementSource>>,
    ) -> Result<Self> {
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

    fn mint_run(&self) -> RunId {
        RunId(format!("run-{:04}", self.next_run.fetch_add(1, Ordering::SeqCst) + 1))
    }

    fn run_sweep(
        &self,
        run: RunId,
        target: &Target,
        events: &dyn EventSink,
        cancellation: &Cancellation,
    ) {
        let mut state = self.runs.lock().expect("runs poisoned").get(&run).cloned().unwrap_or_else(
            || {
                let matrix = self.config.read().expect("config poisoned").matrix.clone();
                SweepState::new(run.clone(), target.id.clone(), BuildConfiguration::expand(&matrix))
            },
        );

        let parallelism = self.config.read().expect("config poisoned").parallelism;
        let sweep = Sweep {
            builder: &self.builder,
            runner: &self.runner,
            benchmark: self.benchmark.as_deref(),
            options: SweepOptions::new(self.gates.clone()).with_parallelism(parallelism),
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

        if let Err(error) = outcome {
            events.emit(EngineEvent::Failed { run, error: error.to_string() });
        }
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
        let Some(finding) = findings.iter().find(|f| f.id == finding) else {
            return Vec::new();
        };
        finding.evidence.iter().filter_map(|id| self.inner.runner.store().get(id)).collect()
    }

    fn proposals(&self) -> Vec<Proposal> {
        self.inner.proposals.lock().expect("proposals poisoned").clone()
    }

    fn propose_configuration(&self, configuration: &BuildConfiguration) -> Result<Proposal> {
        let manifest =
            self.inner.config.read().expect("config poisoned").root.join("Cargo.toml");

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
