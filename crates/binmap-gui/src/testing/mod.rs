//! A fake engine, for driving the interface without building anything.
//!
//! DESIGN-GUI §8 asks for "view state against a fake `Engine` returning
//! scripted `EngineEvent`s", and this is it. Everything it returns is
//! scripted, so a test can put the interface into a state that would otherwise
//! take a ninety-six configuration sweep to reach.
//!
//! It is real enough to be worth trusting: findings come out of a real
//! `EvidenceStore` through the real `Finding::new`, so a test cannot
//! accidentally assert against a claim the product could never construct.

use binmap_core::config::TrustTier;
use binmap_core::configuration::BuildConfiguration;
use binmap_core::event::{Cancellation, EngineEvent, EventSink, RunId};
use binmap_core::evidence::{Evidence, EvidenceStore, ToolInvocation};
use binmap_core::facade::{
    Engine, Measurement, Probe, ProbeStatus, Proposal, Request, SweepSummary,
};
use binmap_core::finding::{Confidence, Finding, FindingDraft, FindingKind, Impact, Provenance};
use binmap_core::gate::{Gate, GateOutcome, GateResult, VerificationReport};
use binmap_core::traits::{Target, TargetFamily};
use binmap_core::{Capabilities, Capability};
use std::sync::{Arc, Mutex};

/// An engine that measures nothing and answers everything.
pub struct ScriptedEngine {
    store: EvidenceStore,
    targets: Vec<Target>,
    probes: Vec<Probe>,
    findings: Mutex<Vec<Finding>>,
    sweeps: Mutex<Vec<SweepSummary>>,
    tier: Mutex<TrustTier>,
    /// What every `start` will emit, in order.
    script: Mutex<Vec<EngineEvent>>,
    /// Recorded so a test can assert a run was actually requested.
    pub started: Mutex<Vec<Request>>,
    pub restores: Mutex<Vec<String>>,
}

impl Default for ScriptedEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ScriptedEngine {
    pub fn new() -> Self {
        Self {
            store: EvidenceStore::new(),
            targets: Vec::new(),
            probes: Vec::new(),
            findings: Mutex::new(Vec::new()),
            sweeps: Mutex::new(Vec::new()),
            tier: Mutex::new(TrustTier::default()),
            script: Mutex::new(Vec::new()),
            started: Mutex::new(Vec::new()),
            restores: Mutex::new(Vec::new()),
        }
    }

    /// A target that can only be swept — Phase 0's real shape, and the one
    /// that makes the nav rail's capability filtering observable.
    pub fn with_target(mut self, id: &str, capabilities: &[Capability]) -> Self {
        self.targets.push(Target {
            id: id.to_string(),
            name: id.rsplit("::").next().unwrap_or(id).to_string(),
            family: TargetFamily::Rust,
            package: id.split("::").next().unwrap_or(id).to_string(),
            manifest: "Cargo.toml".into(),
            capabilities: capabilities.iter().copied().collect::<Capabilities>(),
        });
        self
    }

    pub fn with_probe(mut self, name: &str, status: ProbeStatus) -> Self {
        self.probes.push(match status {
            ProbeStatus::Present => Probe::present("Rust target", name, "present"),
            other => Probe::needs("Rust target", name, other, "missing", "cargo install it")
                .with_action("Re-check"),
        });
        self
    }

    /// Record a tool invocation and return the identifier a finding may cite.
    ///
    /// Named `record` rather than `evidence` so it does not shadow the trait's
    /// `Engine::evidence`, which answers a different question.
    pub fn record(&self, tool: &str, output: &str) -> binmap_core::EvidenceId {
        let pending = self.store.begin(ToolInvocation::new(tool, ["--scripted"]));
        self.store.complete(pending, output, 0)
    }

    /// A grounded finding, built the only way findings can be built.
    pub fn with_finding(self, id: &str, kind: FindingKind, title: &str) -> Self {
        let evidence = self.record("cargo", "scripted output");
        let finding = Finding::new(
            FindingDraft::new(id, kind, title)
                .cite(evidence)
                .detail("scripted")
                .impact(Impact::size(-1024))
                .confidence(Confidence::Certain)
                .provenance(Provenance::Measured),
            &self.store,
        )
        .expect("a scripted finding is grounded");
        self.findings.lock().expect("poisoned").push(finding);
        self
    }

    /// A sweep with `passing` configurations that passed and `rejected` that
    /// did not, the first passing one on the frontier.
    pub fn with_sweep(self, target: &str, passing: usize, rejected: usize) -> Self {
        let mut measured = Vec::new();
        for index in 0..(passing + rejected) {
            let ok = index < passing;
            let report = VerificationReport {
                candidate: format!("cfg-{index}"),
                outcomes: vec![
                    GateOutcome::new(Gate::Builds, GateResult::Passed, "builds"),
                    GateOutcome::new(
                        Gate::TestsPass,
                        if ok { GateResult::Passed } else { GateResult::Failed },
                        if ok { "the suite passes" } else { "failures:" },
                    ),
                ],
            };
            measured.push(Measurement {
                id: format!("cfg-{index}"),
                flags: format!("opt-level={index}"),
                settings: vec![("opt-level".into(), index.to_string())],
                size_bytes: Some(1_000_000 - index as u64 * 1_000),
                size_delta: Some(-(index as i64) * 1_000),
                runtime_nanos: None,
                build_time_nanos: None,
                gates: report,
                on_frontier: index == 0,
                built: true,
            });
        }

        self.sweeps.lock().expect("poisoned").push(SweepSummary {
            run: RunId("scripted".into()),
            target: target.to_string(),
            baseline_bytes: Some(1_000_000),
            noise_floor: Some(0.009),
            noise_floor_samples: 40,
            measured,
            complete: true,
        });
        self
    }

    /// What `start` will emit.
    pub fn scripting(self, events: Vec<EngineEvent>) -> Self {
        *self.script.lock().expect("poisoned") = events;
        self
    }
}

impl Engine for ScriptedEngine {
    fn targets(&self) -> binmap_core::Result<Vec<Target>> {
        Ok(self.targets.clone())
    }

    fn probe_environment(&self) -> Vec<Probe> {
        self.probes.clone()
    }

    fn start(
        &self,
        request: Request,
        events: Arc<dyn EventSink>,
    ) -> binmap_core::Result<(RunId, Cancellation)> {
        self.started.lock().expect("poisoned").push(request);
        let run = RunId("scripted".into());
        for event in self.script.lock().expect("poisoned").iter() {
            events.emit(event.clone());
        }
        Ok((run, Cancellation::new()))
    }

    fn findings(&self) -> Vec<Finding> {
        self.findings.lock().expect("poisoned").clone()
    }

    fn evidence(&self, finding: &str) -> Vec<Evidence> {
        self.findings
            .lock()
            .expect("poisoned")
            .iter()
            .find(|f| f.id() == finding)
            .map(|f| f.evidence().iter().filter_map(|id| self.store.get(id)).collect())
            .unwrap_or_default()
    }

    fn proposals(&self) -> Vec<Proposal> {
        Vec::new()
    }

    fn restore_session(&self, target: &str) -> usize {
        self.restores.lock().expect("poisoned").push(target.to_string());
        0
    }

    fn sweeps(&self) -> Vec<SweepSummary> {
        self.sweeps.lock().expect("poisoned").clone()
    }

    fn trust_tier(&self) -> TrustTier {
        *self.tier.lock().expect("poisoned")
    }

    fn set_trust_tier(&self, tier: TrustTier) {
        *self.tier.lock().expect("poisoned") = tier;
    }

    fn propose_configuration(
        &self,
        configuration: &BuildConfiguration,
    ) -> binmap_core::Result<Proposal> {
        Ok(Proposal {
            id: format!("apply-{}", configuration.name()),
            finding: String::new(),
            summary: "scripted".into(),
            diff: String::new(),
            writes: Vec::new(),
        })
    }
}
