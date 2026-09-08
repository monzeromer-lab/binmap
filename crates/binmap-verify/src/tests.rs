use super::*;
use binmap_core::evidence::EvidenceStore;

fn shell(script: &str) -> ToolInvocation {
    ToolInvocation::new("sh", ["-c", script])
}

fn runner() -> ToolRunner {
    ToolRunner::new(EvidenceStore::new(), ".")
}

fn measured(baseline: u64, candidate: u64) -> SizeObservation {
    SizeObservation { baseline_bytes: baseline, candidate_bytes: candidate }
}

#[test]
fn a_candidate_that_does_not_build_names_the_gate_and_skips_the_rest() {
    let runner = runner();
    let plan = GatePlan::new(shell("echo 'error: expected one of `,`' >&2; exit 1"))
        .testing_with(shell("true"));
    let report = Harness::new(&runner, plan).verify(&Candidate::new("candidate-1"));

    assert_eq!(report.rejected_by(), Some(Gate::Builds));
    assert_eq!(report.summary(), "Rejected by Builds");

    // The gates that did not run say so. An absent gate is never a passing one.
    for gate in [Gate::TestsPass, Gate::SizeNotWorse, Gate::BenchmarkNotWorse, Gate::MiriClean] {
        let outcome = report.outcome(gate).expect("every gate is reported");
        assert!(
            matches!(outcome.result, GateResult::Skipped { .. }),
            "{gate} should be skipped, was {:?}",
            outcome.result
        );
    }
}

#[test]
fn the_build_gate_judges_warnings_against_the_baseline_not_against_zero() {
    let runner = runner();
    let noisy = "echo 'warning: unused variable: `x`' >&2; echo 'warning: unused import' >&2";

    // Two warnings where the baseline already had two: nothing new, so it passes.
    let plan = GatePlan::new(shell(noisy)).against_baseline_warnings(2);
    let report = Harness::new(&runner, plan).verify(&Candidate::new("c").sized(measured(10, 10)));
    assert!(report.outcome(Gate::Builds).unwrap().result == GateResult::Passed);

    // The same two where the baseline had one: the candidate added a warning.
    let plan = GatePlan::new(shell(noisy)).against_baseline_warnings(1);
    let report = Harness::new(&runner, plan).verify(&Candidate::new("c").sized(measured(10, 10)));
    assert_eq!(report.rejected_by(), Some(Gate::Builds));
    assert!(report.outcome(Gate::Builds).unwrap().detail.contains("adds 1 warning"));
}

#[test]
fn a_failing_suite_stops_the_expensive_gates_but_size_is_still_recorded() {
    let runner = runner();
    let plan = GatePlan::new(shell("true")).testing_with(shell("echo 'failures:' ; exit 101"));
    let candidate = Candidate::new("c").sized(measured(1_000_000, 900_000));
    let report = Harness::new(&runner, plan).verify(&candidate);

    assert_eq!(report.rejected_by(), Some(Gate::TestsPass));
    // Size was measured before the suite ran, so it is reported rather than lost.
    let size = report.outcome(Gate::SizeNotWorse).unwrap();
    assert_eq!(size.result, GateResult::Passed);
    assert!(size.detail.contains("falls by"), "{}", size.detail);
}

#[test]
fn size_that_was_never_measured_fails_the_gate() {
    let runner = runner();
    let plan = GatePlan::new(shell("true")).testing_with(shell("true"));
    let report = Harness::new(&runner, plan).verify(&Candidate::new("c"));

    assert_eq!(report.rejected_by(), Some(Gate::SizeNotWorse));
    assert!(report.outcome(Gate::SizeNotWorse).unwrap().detail.contains("unknown"));
}

#[test]
fn a_size_increase_passes_the_gate_and_says_so_plainly() {
    // The gate asks whether the cost is known, not whether it is zero: a
    // candidate chosen for runtime is allowed to cost bytes, out loud.
    let runner = runner();
    let plan = GatePlan::new(shell("true")).testing_with(shell("true"));
    let candidate = Candidate::new("c").sized(measured(1_000_000, 1_400_000));
    let report = Harness::new(&runner, plan).verify(&candidate);

    assert!(report.passed());
    assert_eq!(report.outcome(Gate::SizeNotWorse).unwrap().detail, "size rises by 390.6 KiB");
}

#[test]
fn a_result_inside_the_noise_floor_is_inconclusive_and_never_a_pass() {
    let runner = runner();
    let plan = GatePlan::new(shell("true")).testing_with(shell("true"));
    let candidate = Candidate::new("c").sized(measured(10, 10)).benchmarked(
        BenchmarkVerdict::Inconclusive {
            detail: "the 1.2% difference is inside this machine's 3.0% noise floor".into(),
        },
    );
    let report = Harness::new(&runner, plan).verify(&candidate);

    // It does not reject the candidate, and it does not claim a win either.
    assert!(report.passed());
    assert_eq!(report.inconclusive(), vec![Gate::BenchmarkNotWorse]);
    assert_eq!(report.summary(), "Passed, with BenchmarkNotWorse inconclusive");
    assert_ne!(report.outcome(Gate::BenchmarkNotWorse).unwrap().result, GateResult::Passed);
}

#[test]
fn a_significant_regression_rejects_the_candidate() {
    let runner = runner();
    let plan = GatePlan::new(shell("true")).testing_with(shell("true"));
    let candidate = Candidate::new("c").sized(measured(10, 10)).benchmarked(
        BenchmarkVerdict::Regressed { detail: "8.4% slower, well outside the noise floor".into() },
    );
    let report = Harness::new(&runner, plan).verify(&candidate);
    assert_eq!(report.rejected_by(), Some(Gate::BenchmarkNotWorse));
}

#[test]
fn sanitizers_run_only_when_unsafe_is_touched_and_the_skip_states_why() {
    let runner = runner();
    let plan = GatePlan::new(shell("true"))
        .testing_with(shell("true"))
        .sanitizing_with(shell("exit 1"));
    let safe = Candidate::new("c").sized(measured(10, 10));
    let report = Harness::new(&runner, plan).verify(&safe);

    // The sanitizer command would have failed; it is not run, and the skip
    // does not read as a pass.
    assert!(report.passed());
    let outcome = report.outcome(Gate::MiriClean).unwrap();
    assert_eq!(outcome.result.label(), "Skipped");
    assert!(outcome.detail.contains("does not touch unsafe"));
}

#[test]
fn unsafe_code_with_no_sanitizer_available_is_reported_as_unchecked() {
    let runner = runner();
    let plan = GatePlan::new(shell("true")).testing_with(shell("true"));
    let candidate = Candidate::new("c").sized(measured(10, 10)).touching_unsafe(true);
    let report = Harness::new(&runner, plan).verify(&candidate);

    let outcome = report.outcome(Gate::MiriClean).unwrap();
    assert!(outcome.detail.contains("unsafe code is unchecked"), "{}", outcome.detail);
}

#[test]
fn a_clean_sanitizer_run_over_heavy_ffi_carries_the_caveat() {
    let runner = runner();
    let plan = GatePlan::new(shell("true"))
        .testing_with(shell("true"))
        .sanitizing_with(shell("true"))
        .with_substantial_ffi(true);
    let candidate = Candidate::new("c").sized(measured(10, 10)).touching_unsafe(true);
    let report = Harness::new(&runner, plan).verify(&candidate);

    assert!(report.passed());
    let caveats = report.caveats();
    assert_eq!(caveats.len(), 1);
    assert!(caveats[0].contains("foreign-function calls"));
}

#[test]
fn every_gate_outcome_that_ran_a_tool_cites_its_evidence() {
    let store = EvidenceStore::new();
    let runner = ToolRunner::new(store.clone(), ".");
    let plan = GatePlan::new(shell("true")).testing_with(shell("true"));
    let candidate = Candidate::new("c").sized(measured(10, 9));
    let report = Harness::new(&runner, plan).verify(&candidate);

    for gate in [Gate::Builds, Gate::TestsPass] {
        let outcome = report.outcome(gate).unwrap();
        assert_eq!(outcome.evidence.len(), 1, "{gate} cites nothing");
        assert!(store.issued(&outcome.evidence[0]));
    }
}

#[test]
fn a_report_round_trips_through_the_session_artifact() {
    let runner = runner();
    let plan = GatePlan::new(shell("true")).testing_with(shell("true"));
    let report = Harness::new(&runner, plan).verify(&Candidate::new("c").sized(measured(10, 8)));

    let json = serde_json::to_string(&report).unwrap();
    let restored: VerificationReport = serde_json::from_str(&json).unwrap();
    assert_eq!(report, restored);
}
