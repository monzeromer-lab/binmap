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
pub use binmap_core::gate::{Gate, GateOutcome, GateResult, VerificationReport};
use binmap_core::tool::ToolRunner;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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
    /// the difference, not the total: a project that starts with warnings is
    /// not thereby forbidden a smaller binary.
    pub baseline_warnings: usize,
    /// The significance level `BenchmarkNotWorse` decides at. Shown beside the
    /// gate's name, because a threshold that decides a verdict belongs on
    /// screen next to the verdict.
    pub significance: f64,
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
            significance: 0.05,
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

    pub fn deciding_at(mut self, significance: f64) -> Self {
        self.significance = significance;
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

        // Building is the only thing everything else depends on, so it runs
        // first and its result decides how much of the rest can run at all.
        let (build, warnings) = self.gate_builds();
        let build_failed = build.result.is_failure();
        outcomes.push(build);

        if build_failed {
            for gate in Gate::ALL.into_iter().filter(|gate| *gate != Gate::Builds) {
                outcomes.push(GateOutcome::skipped(gate, "the candidate did not build"));
            }
            let mut report = VerificationReport { candidate: candidate.id.clone(), outcomes };
            report.in_display_order();
            return report;
        }

        outcomes.push(self.gate_warnings(warnings));

        let tests = self.gate_tests();
        let tests_failed = tests.result.is_failure();
        outcomes.push(tests);

        // Size is free — it was measured before the gates ran — so it is
        // recorded whatever else happened.
        outcomes.push(self.gate_size(candidate));

        if tests_failed {
            for gate in [Gate::BenchmarkNotWorse, Gate::MiriClean] {
                outcomes.push(GateOutcome::skipped(gate, "not reached"));
            }
            let mut report = VerificationReport { candidate: candidate.id.clone(), outcomes };
            report.in_display_order();
            return report;
        }

        outcomes.push(self.gate_benchmark(candidate));
        outcomes.push(self.gate_sanitizers(candidate));

        let mut report = VerificationReport { candidate: candidate.id.clone(), outcomes };
        report.in_display_order();
        report
    }

    /// Run the build, and report how long it took and how much it complained.
    ///
    /// Returns the warning count alongside, because the gate that judges it is
    /// a separate row and re-running the build to count them twice would be
    /// absurd.
    fn gate_builds(&self) -> (GateOutcome, Option<usize>) {
        let output = match self.runner.run_with_env(self.plan.build.clone(), &self.plan.env) {
            Ok(output) => output,
            Err(error) => {
                return (
                    GateOutcome::new(Gate::Builds, GateResult::Failed, error.to_string()),
                    None,
                );
            }
        };

        if !output.succeeded() {
            let detail = first_error_line(&output.stderr)
                .unwrap_or_else(|| format!("the build exited {}", output.exit_code));
            return (
                GateOutcome::new(Gate::Builds, GateResult::Failed, detail).citing(output.evidence),
                None,
            );
        }

        let warnings = count_warnings(&output.stderr);
        let outcome =
            GateOutcome::new(Gate::Builds, GateResult::Passed, format!("{:?}", output.duration))
                .citing(output.evidence);
        (outcome, Some(warnings))
    }

    /// Judged against the baseline, never against zero.
    fn gate_warnings(&self, warnings: Option<usize>) -> GateOutcome {
        let Some(warnings) = warnings else {
            return GateOutcome::skipped(
                Gate::NoNewWarnings,
                "the build produced no output to read",
            );
        };
        let baseline = self.plan.baseline_warnings;
        if warnings > baseline {
            let added = warnings - baseline;
            GateOutcome::new(Gate::NoNewWarnings, GateResult::Failed, format!("{added} new"))
        } else {
            GateOutcome::new(Gate::NoNewWarnings, GateResult::Passed, "0 new")
        }
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
            return GateOutcome::skipped(Gate::BenchmarkNotWorse, "no benchmark is declared")
                .configured(format!("significance: {}", self.plan.significance));
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
        outcome.configured(format!("significance: {}", self.plan.significance))
    }

    fn gate_sanitizers(&self, candidate: &Candidate) -> GateOutcome {
        if !candidate.touches_unsafe {
            return GateOutcome::skipped(Gate::MiriClean, "no unsafe touched");
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

/// Count the warnings cargo actually emitted.
///
/// Cargo prints a per-package tally of its own — "warning: `app` (lib)
/// generated 3 warnings" — which starts with `warning:` and is not one. Adding
/// it inflated the count by one per package and made NoNewWarnings fail on
/// candidates that added nothing.
fn count_warnings(stderr: &str) -> usize {
    stderr
        .lines()
        .map(str::trim_start)
        .filter(|line| line.starts_with("warning:"))
        .filter(|line| !line.contains("generated ") || !line.contains("warning"))
        .count()
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
