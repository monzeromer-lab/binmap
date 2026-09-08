//! The gate vocabulary: what is checked, what was concluded, and how a
//! rejection is stated.
//!
//! The words live here rather than in `binmap-verify` for the same reason
//! `Finding` lives in this crate: the interface renders them and the session
//! artifact carries them, and neither may depend on the harness that runs
//! them. Behaviour is `binmap-verify`'s; this is the shared vocabulary (§2.4).

use crate::evidence::EvidenceId;
use serde::{Deserialize, Serialize};

/// The gates, in the order the Profile Lab lists them.
///
/// The plan's §6 table folds the warning diff into `Builds`; the design gives
/// it its own row, and the design is right: a candidate that builds but adds
/// warnings the baseline did not have is a different outcome from one that
/// does not build, and a user reading a rejection deserves to see which.
///
/// This is display order. Execution order is cheapest-first — a candidate that
/// does not build is not worth timing — and the report reorders to this before
/// anything renders it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Gate {
    /// The build succeeds, warnings diffed against the baseline. Always.
    Builds,
    /// The suite passes, using the user's command where declared. Always.
    TestsPass,
    /// The candidate adds no warning the baseline did not already emit.
    /// Judged against the baseline, never against zero: a project that starts
    /// with warnings is not thereby forbidden a smaller binary.
    NoNewWarnings,
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
    /// Display order, as the Profile Lab lists them.
    pub const ALL: [Gate; 6] = [
        Gate::Builds,
        Gate::TestsPass,
        Gate::NoNewWarnings,
        Gate::BenchmarkNotWorse,
        Gate::SizeNotWorse,
        Gate::MiriClean,
    ];

    /// Execution order: cheapest first, so an expensive gate never runs for a
    /// candidate a cheap one has already rejected.
    pub const IN_COST_ORDER: [Gate; 6] = [
        Gate::Builds,
        Gate::NoNewWarnings,
        Gate::TestsPass,
        Gate::SizeNotWorse,
        Gate::BenchmarkNotWorse,
        Gate::MiriClean,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Gate::Builds => "Builds",
            Gate::TestsPass => "TestsPass",
            Gate::NoNewWarnings => "NoNewWarnings",
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
            Gate::NoNewWarnings => "no warning appears that the baseline did not have",
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
    Skipped {
        reason: String,
    },
    /// The gate ran and could not tell — a difference inside the noise floor.
    /// Never rendered as a pass.
    Inconclusive {
        reason: String,
    },
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
    /// What the gate was configured with, rendered beside its name as
    /// `BenchmarkNotWorse { significance: 0.05 }`. A threshold that decides a
    /// verdict belongs on screen next to the verdict.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<EvidenceId>,
}

impl GateOutcome {
    pub fn new(gate: Gate, result: GateResult, detail: impl Into<String>) -> Self {
        Self {
            gate,
            result,
            detail: detail.into(),
            caveat: None,
            parameters: None,
            evidence: Vec::new(),
        }
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

    /// Record what the gate was configured with.
    pub fn configured(mut self, parameters: impl Into<String>) -> Self {
        self.parameters = Some(parameters.into());
        self
    }

    /// The gate's name as the Profile Lab prints it, carrying its
    /// configuration where it has any.
    pub fn qualified_label(&self) -> String {
        match &self.parameters {
            Some(parameters) => format!("{} {{ {parameters} }}", self.gate.label()),
            None => self.gate.label().to_string(),
        }
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

    /// The gates that did not run at all.
    pub fn skipped(&self) -> Vec<Gate> {
        self.outcomes
            .iter()
            .filter(|o| matches!(o.result, GateResult::Skipped { .. }))
            .map(|o| o.gate)
            .collect()
    }

    /// The one-line verdict: what happened, and if it failed, which gate.
    ///
    /// "All gates passed" is reserved for a candidate where all of them
    /// actually ran. It was printed for one where two of six were skipped,
    /// which reads as a stronger statement than the evidence supports — and
    /// the whole point of naming gates is that the reader knows what was
    /// checked.
    pub fn summary(&self) -> String {
        if let Some(gate) = self.rejected_by() {
            return format!("Rejected by {gate}");
        }

        let inconclusive = self.inconclusive();
        let skipped = self.skipped();

        let mut caveats = Vec::new();
        if !inconclusive.is_empty() {
            let names: Vec<&str> = inconclusive.iter().map(|g| g.label()).collect();
            caveats.push(format!("{} inconclusive", names.join(" and ")));
        }
        if !skipped.is_empty() {
            let names: Vec<&str> = skipped.iter().map(|g| g.label()).collect();
            caveats.push(format!("{} not run", names.join(" and ")));
        }

        if caveats.is_empty() {
            "All gates passed".to_string()
        } else {
            format!("Passed, with {}", caveats.join("; "))
        }
    }

    pub fn outcome(&self, gate: Gate) -> Option<&GateOutcome> {
        self.outcomes.iter().find(|o| o.gate == gate)
    }

    /// Put the outcomes into the order the Profile Lab lists them.
    ///
    /// They are produced cheapest-first, which is an execution concern; a
    /// reader wants them in a stable order that does not shuffle depending on
    /// which gate stopped the run.
    pub fn in_display_order(&mut self) {
        self.outcomes.sort_by_key(|outcome| {
            Gate::ALL.iter().position(|gate| *gate == outcome.gate).unwrap_or(usize::MAX)
        });
    }
}
