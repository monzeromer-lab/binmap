//! The gate vocabulary: what is checked, what was concluded, and how a
//! rejection is stated.
//!
//! The words live here rather than in `binmap-verify` for the same reason
//! `Finding` lives in this crate: the interface renders them and the session
//! artifact carries them, and neither may depend on the harness that runs
//! them. Behaviour is `binmap-verify`'s; this is the shared vocabulary (§2.4).

use crate::evidence::EvidenceId;
use serde::{Deserialize, Serialize};

/// The five gates. Ordered cheapest-first: a candidate that does not build is
/// not worth timing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Gate {
    /// The build succeeds, warnings diffed against the baseline. Always.
    Builds,
    /// The suite passes, using the user's command where declared. Always.
    TestsPass,
    /// Size measured and recorded, whatever the objective was. Always.
    ///
    /// The pass condition is deliberately "we know what happened to size",
    /// not "size did not increase": a candidate chosen for runtime is allowed
    /// to cost bytes, but it is never allowed to cost them unnoticed.
    SizeNotWorse,
    /// A statistically significant win, or no significant regression, against
    /// the measured noise floor. Only where a benchmark is declared.
    BenchmarkNotWorse,
    /// Sanitizers clean over the reachable portion. Only when unsafe is
    /// touched.
    MiriClean,
}

impl Gate {
    pub const ALL: [Gate; 5] = [
        Gate::Builds,
        Gate::TestsPass,
        Gate::SizeNotWorse,
        Gate::BenchmarkNotWorse,
        Gate::MiriClean,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Gate::Builds => "Builds",
            Gate::TestsPass => "TestsPass",
            Gate::SizeNotWorse => "SizeNotWorse",
            Gate::BenchmarkNotWorse => "BenchmarkNotWorse",
            Gate::MiriClean => "MiriClean",
        }
    }

    /// What this gate is checking, as the gate tally shows it.
    pub fn describes(self) -> &'static str {
        match self {
            Gate::Builds => "the build succeeds and adds no warnings",
            Gate::TestsPass => "the test suite passes",
            Gate::SizeNotWorse => "the size change is measured and recorded",
            Gate::BenchmarkNotWorse => "the benchmark does not significantly regress",
            Gate::MiriClean => "sanitizers are clean over the reachable code",
        }
    }
}

impl std::fmt::Display for Gate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// What a gate concluded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum GateResult {
    Passed,
    /// The candidate is rejected, and this gate is the reason.
    Failed,
    /// The gate did not apply. The reason is stated, so an absent gate is
    /// never mistaken for a passing one.
    Skipped { reason: String },
    /// The gate ran and could not tell — a difference inside the noise floor.
    /// Never rendered as a pass.
    Inconclusive { reason: String },
}

impl GateResult {
    pub fn is_failure(&self) -> bool {
        matches!(self, GateResult::Failed)
    }

    pub fn label(&self) -> &'static str {
        match self {
            GateResult::Passed => "Passed",
            GateResult::Failed => "Failed",
            GateResult::Skipped { .. } => "Skipped",
            GateResult::Inconclusive { .. } => "Inconclusive",
        }
    }
}

/// One gate's verdict, with the evidence behind it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateOutcome {
    pub gate: Gate,
    pub result: GateResult,
    /// One line, as the gate tally shows it.
    pub detail: String,
    /// A caveat the interface must show alongside a pass, where the pass means
    /// less than it sounds. `MiriClean` over code with substantial foreign
    /// calls is the case this exists for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caveat: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<EvidenceId>,
}

impl GateOutcome {
    pub fn new(gate: Gate, result: GateResult, detail: impl Into<String>) -> Self {
        Self { gate, result, detail: detail.into(), caveat: None, evidence: Vec::new() }
    }

    /// A gate that did not apply, carrying the reason as its detail so an
    /// absent gate never reads as a passing one.
    pub fn skipped(gate: Gate, reason: impl Into<String>) -> Self {
        let reason = reason.into();
        Self::new(gate, GateResult::Skipped { reason: reason.clone() }, reason)
    }

    pub fn citing(mut self, evidence: EvidenceId) -> Self {
        self.evidence.push(evidence);
        self
    }

    pub fn with_caveat(mut self, caveat: impl Into<String>) -> Self {
        self.caveat = Some(caveat.into());
        self
    }
}

/// Every gate's verdict on one candidate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationReport {
    /// The candidate this is about — a configuration name, or a proposal id.
    pub candidate: String,
    pub outcomes: Vec<GateOutcome>,
}

impl VerificationReport {
    /// The first gate that failed, if any. This is the name the interface puts
    /// on a rejected candidate — "rejected: TestsPass", never just "rejected".
    pub fn rejected_by(&self) -> Option<Gate> {
        self.outcomes.iter().find(|o| o.result.is_failure()).map(|o| o.gate)
    }

    pub fn passed(&self) -> bool {
        self.rejected_by().is_none()
    }

    /// The gates that ran and could not tell. Shown beside a pass, because a
    /// candidate that passed only because nothing could be measured is a
    /// different thing from one that passed on measurements.
    pub fn inconclusive(&self) -> Vec<Gate> {
        self.outcomes
            .iter()
            .filter(|o| matches!(o.result, GateResult::Inconclusive { .. }))
            .map(|o| o.gate)
            .collect()
    }

    /// Every caveat attached to a passing gate, so the interface can state the
    /// sanitizer gap rather than implying a guarantee it does not have.
    pub fn caveats(&self) -> Vec<&str> {
        self.outcomes.iter().filter_map(|o| o.caveat.as_deref()).collect()
    }

    /// The one-line verdict: what happened, and if it failed, which gate.
    pub fn summary(&self) -> String {
        match self.rejected_by() {
            Some(gate) => format!("Rejected by {gate}"),
            None => {
                let inconclusive = self.inconclusive();
                if inconclusive.is_empty() {
                    "All gates passed".to_string()
                } else {
                    let names: Vec<&str> = inconclusive.iter().map(|g| g.label()).collect();
                    format!("Passed, with {} inconclusive", names.join(" and "))
                }
            }
        }
    }

    pub fn outcome(&self, gate: Gate) -> Option<&GateOutcome> {
        self.outcomes.iter().find(|o| o.gate == gate)
    }
}
