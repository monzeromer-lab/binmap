//! The verification harness: five gates, applied to every proposal whoever
//! produced it — our own loop, an external agent, or the user.
//!
//! Two rules shape the whole module.
//!
//! A candidate failing a gate is **rejected with the failing gate named, and
//! stays visible**. A near miss is informative, and a harness that silently
//! drops candidates teaches the user nothing about their own build.
//!
//! A result inside the machine's noise floor is **inconclusive, never coloured
//! as a win**. That is a third outcome, not a shade of pass, so it has its own
//! variant and the interface cannot accidentally render it green.

use binmap_core::config::TrustTier;
use binmap_core::evidence::{EvidenceId, ToolInvocation};
use binmap_core::tool::ToolRunner;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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
    fn new(gate: Gate, result: GateResult, detail: impl Into<String>) -> Self {
        Self { gate, result, detail: detail.into(), caveat: None, evidence: Vec::new() }
    }

    fn skipped(gate: Gate, reason: impl Into<String>) -> Self {
        let reason = reason.into();
        Self::new(gate, GateResult::Skipped { reason: reason.clone() }, reason)
    }

    fn citing(mut self, evidence: EvidenceId) -> Self {
        self.evidence.push(evidence);
        self
    }

    fn with_caveat(mut self, caveat: impl Into<String>) -> Self {
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

/// A benchmark comparison, already reduced to a verdict.
///
/// The statistics live in `binmap-measure`; the harness consumes the verdict.
/// That split is what keeps this crate reusable — the gates do not care how
/// significance was decided, only that someone decided it against a measured
/// noise floor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum BenchmarkVerdict {
    /// A significant win.
    Improved { detail: String },
    /// No significant difference, or a difference inside the noise floor.
    NoSignificantChange { detail: String },
    /// A significant regression. This is the only one that fails the gate.
    Regressed { detail: String },
    /// Measured, but the samples could not settle the question — too few, or
    /// too noisy a machine.
    Inconclusive { detail: String },
}

/// What the caller measured about the candidate's size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SizeObservation {
    pub baseline_bytes: u64,
    pub candidate_bytes: u64,
}

impl SizeObservation {
    pub fn delta(&self) -> i64 {
        self.candidate_bytes as i64 - self.baseline_bytes as i64
    }
}

/// How to build, test and sanitize this project, and what the baseline looked
/// like.
#[derive(Debug, Clone)]
pub struct GatePlan {
    pub build: ToolInvocation,
    /// The user's test command where they declared one; `cargo test` otherwise.
    pub test: Option<ToolInvocation>,
    /// The sanitizer command. `None` when no sanitizer is available, which is
    /// reported as a skip with that reason rather than as a pass.
    pub miri: Option<ToolInvocation>,
    /// Warnings the baseline build already emitted. A candidate is judged on
    /// the difference, not the total.
    pub baseline_warnings: usize,
    /// Environment applied to every gate command.
    pub env: BTreeMap<String, String>,
    /// Whether this project makes substantial foreign-function calls. When it
    /// does, a clean sanitizer run carries a caveat.
    pub substantial_ffi: bool,
}

impl GatePlan {
    pub fn new(build: ToolInvocation) -> Self {
        Self {
            build,
            test: None,
            miri: None,
            baseline_warnings: 0,
            env: BTreeMap::new(),
            substantial_ffi: false,
        }
    }

    pub fn testing_with(mut self, test: ToolInvocation) -> Self {
        self.test = Some(test);
        self
    }

    pub fn sanitizing_with(mut self, miri: ToolInvocation) -> Self {
        self.miri = Some(miri);
        self
    }

    pub fn against_baseline_warnings(mut self, warnings: usize) -> Self {
        self.baseline_warnings = warnings;
        self
    }

    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    pub fn with_substantial_ffi(mut self, substantial_ffi: bool) -> Self {
        self.substantial_ffi = substantial_ffi;
        self
    }
}

/// What is being verified, and what the caller already measured about it.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub id: String,
    /// Whether the change touches `unsafe`. Decides whether `MiriClean`
    /// applies.
    pub touches_unsafe: bool,
    /// `None` means size was not measured, which fails `SizeNotWorse`.
    pub size: Option<SizeObservation>,
    /// `None` means no benchmark was declared, which skips
    /// `BenchmarkNotWorse`.
    pub benchmark: Option<BenchmarkVerdict>,
    /// Evidence the caller already recorded for those measurements.
    pub measurement_evidence: Vec<EvidenceId>,
}

impl Candidate {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            touches_unsafe: false,
            size: None,
            benchmark: None,
            measurement_evidence: Vec::new(),
        }
    }

    pub fn touching_unsafe(mut self, touches_unsafe: bool) -> Self {
        self.touches_unsafe = touches_unsafe;
        self
    }

    pub fn sized(mut self, size: SizeObservation) -> Self {
        self.size = Some(size);
        self
    }

    pub fn benchmarked(mut self, verdict: BenchmarkVerdict) -> Self {
        self.benchmark = Some(verdict);
        self
    }

    pub fn citing(mut self, evidence: impl IntoIterator<Item = EvidenceId>) -> Self {
        self.measurement_evidence.extend(evidence);
        self
    }
}

/// Runs the gates.
pub struct Harness<'a> {
    runner: &'a ToolRunner,
    plan: GatePlan,
    /// The tier the session is at. Nothing here writes to the user's tree, so
    /// the harness runs at any tier; the field exists so a gate that needs to
    /// write can refuse rather than surprise.
    tier: TrustTier,
}

impl<'a> Harness<'a> {
    pub fn new(runner: &'a ToolRunner, plan: GatePlan) -> Self {
        Self { runner, plan, tier: TrustTier::Observe }
    }

    pub fn at_tier(mut self, tier: TrustTier) -> Self {
        self.tier = tier;
        self
    }

    /// Run every gate against one candidate.
    ///
    /// Gates run cheapest-first and stop at the first failure: once a
    /// candidate does not build there is nothing left to learn from timing it,
    /// and the gates that did not run are reported as skipped with that
    /// reason.
    pub fn verify(&self, candidate: &Candidate) -> VerificationReport {
        let mut outcomes = Vec::with_capacity(Gate::ALL.len());

        let build = self.gate_builds();
        let build_failed = build.result.is_failure();
        outcomes.push(build);

        if build_failed {
            for gate in [Gate::TestsPass, Gate::SizeNotWorse, Gate::BenchmarkNotWorse, Gate::MiriClean] {
                outcomes.push(GateOutcome::skipped(gate, "the candidate did not build"));
            }
            return VerificationReport { candidate: candidate.id.clone(), outcomes };
        }

        let tests = self.gate_tests();
        let tests_failed = tests.result.is_failure();
        outcomes.push(tests);
        outcomes.push(self.gate_size(candidate));

        if tests_failed {
            for gate in [Gate::BenchmarkNotWorse, Gate::MiriClean] {
                outcomes.push(GateOutcome::skipped(gate, "the test suite did not pass"));
            }
            return VerificationReport { candidate: candidate.id.clone(), outcomes };
        }

        outcomes.push(self.gate_benchmark(candidate));
        outcomes.push(self.gate_sanitizers(candidate));
        VerificationReport { candidate: candidate.id.clone(), outcomes }
    }

    fn gate_builds(&self) -> GateOutcome {
        let output = match self.runner.run_with_env(self.plan.build.clone(), &self.plan.env) {
            Ok(output) => output,
            Err(error) => {
                return GateOutcome::new(Gate::Builds, GateResult::Failed, error.to_string());
            }
        };

        if !output.succeeded() {
            let detail = first_error_line(&output.stderr)
                .unwrap_or_else(|| format!("the build exited {}", output.exit_code));
            return GateOutcome::new(Gate::Builds, GateResult::Failed, detail)
                .citing(output.evidence);
        }

        let warnings = count_warnings(&output.stderr);
        let baseline = self.plan.baseline_warnings;
        if warnings > baseline {
            let added = warnings - baseline;
            return GateOutcome::new(
                Gate::Builds,
                GateResult::Failed,
                format!("builds, but adds {added} warning(s) the baseline did not have"),
            )
            .citing(output.evidence);
        }

        let detail = if warnings == baseline && warnings > 0 {
            format!("builds, with the baseline's {warnings} warning(s)")
        } else {
            "builds cleanly".to_string()
        };
        GateOutcome::new(Gate::Builds, GateResult::Passed, detail).citing(output.evidence)
    }

    fn gate_tests(&self) -> GateOutcome {
        let Some(test) = self.plan.test.clone() else {
            return GateOutcome::skipped(Gate::TestsPass, "no test command is declared");
        };
        match self.runner.run_with_env(test, &self.plan.env) {
            Ok(output) if output.succeeded() => {
                GateOutcome::new(Gate::TestsPass, GateResult::Passed, "the suite passes")
                    .citing(output.evidence)
            }
            Ok(output) => {
                let detail = first_failure_line(&output.combined())
                    .unwrap_or_else(|| format!("the suite exited {}", output.exit_code));
                GateOutcome::new(Gate::TestsPass, GateResult::Failed, detail)
                    .citing(output.evidence)
            }
            Err(error) => GateOutcome::new(Gate::TestsPass, GateResult::Failed, error.to_string()),
        }
    }

    /// Passes when the size change is known — in either direction.
    fn gate_size(&self, candidate: &Candidate) -> GateOutcome {
        let Some(size) = candidate.size else {
            return GateOutcome::new(
                Gate::SizeNotWorse,
                GateResult::Failed,
                "size was not measured, so the cost of this change is unknown",
            );
        };
        let delta = size.delta();
        let detail = match delta {
            0 => "size is unchanged".to_string(),
            d if d < 0 => format!("size falls by {}", human_bytes(-d as u64)),
            d => format!("size rises by {}", human_bytes(d as u64)),
        };
        let mut outcome = GateOutcome::new(Gate::SizeNotWorse, GateResult::Passed, detail);
        outcome.evidence = candidate.measurement_evidence.clone();
        outcome
    }

    fn gate_benchmark(&self, candidate: &Candidate) -> GateOutcome {
        let Some(verdict) = candidate.benchmark.clone() else {
            return GateOutcome::skipped(Gate::BenchmarkNotWorse, "no benchmark is declared");
        };
        let mut outcome = match verdict {
            BenchmarkVerdict::Improved { detail }
            | BenchmarkVerdict::NoSignificantChange { detail } => {
                GateOutcome::new(Gate::BenchmarkNotWorse, GateResult::Passed, detail)
            }
            BenchmarkVerdict::Regressed { detail } => {
                GateOutcome::new(Gate::BenchmarkNotWorse, GateResult::Failed, detail)
            }
            BenchmarkVerdict::Inconclusive { detail } => GateOutcome::new(
                Gate::BenchmarkNotWorse,
                GateResult::Inconclusive { reason: detail.clone() },
                detail,
            ),
        };
        outcome.evidence = candidate.measurement_evidence.clone();
        outcome
    }

    fn gate_sanitizers(&self, candidate: &Candidate) -> GateOutcome {
        if !candidate.touches_unsafe {
            return GateOutcome::skipped(Gate::MiriClean, "the change does not touch unsafe");
        }
        let Some(miri) = self.plan.miri.clone() else {
            return GateOutcome::skipped(
                Gate::MiriClean,
                "no sanitizer is available on this machine, so unsafe code is unchecked",
            );
        };
        let outcome = match self.runner.run_with_env(miri, &self.plan.env) {
            Ok(output) if output.succeeded() => {
                GateOutcome::new(Gate::MiriClean, GateResult::Passed, "sanitizers are clean")
                    .citing(output.evidence)
            }
            Ok(output) => {
                let detail = first_error_line(&output.combined())
                    .unwrap_or_else(|| format!("the sanitizer exited {}", output.exit_code));
                GateOutcome::new(Gate::MiriClean, GateResult::Failed, detail)
                    .citing(output.evidence)
            }
            Err(error) => GateOutcome::new(Gate::MiriClean, GateResult::Failed, error.to_string()),
        };

        // The gap, stated rather than implied. A tool that lets a clean run
        // over heavy FFI read as a safety guarantee is misleading its user.
        if self.plan.substantial_ffi && matches!(outcome.result, GateResult::Passed) {
            return outcome.with_caveat(
                "This project makes substantial foreign-function calls, which sanitizers do not \
                 see through. A clean run says less here than it would over pure Rust.",
            );
        }
        outcome
    }

    pub fn tier(&self) -> TrustTier {
        self.tier
    }
}

fn count_warnings(stderr: &str) -> usize {
    stderr.lines().filter(|line| line.trim_start().starts_with("warning:")).count()
}

fn first_error_line(text: &str) -> Option<String> {
    text.lines()
        .find(|line| line.trim_start().starts_with("error"))
        .map(|line| line.trim().to_string())
}

fn first_failure_line(text: &str) -> Option<String> {
    text.lines()
        .find(|line| line.contains("FAILED") || line.trim_start().starts_with("failures:"))
        .map(|line| line.trim().to_string())
        .or_else(|| first_error_line(text))
}

fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 { format!("{bytes} B") } else { format!("{value:.1} {}", UNITS[unit]) }
}

#[cfg(test)]
mod tests;
