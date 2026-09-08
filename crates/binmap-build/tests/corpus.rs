//! End-to-end proof against a real crate, with real cargo.
//!
//! The unit tests use a fake build system, which is right for speed but cannot
//! show that the *actual* cargo backend threads a configuration's environment
//! into the gates. This does, on `corpus/generics`, whose suite passes under
//! `overflow-checks = false` and fails under `true`.
//!
//! It is slow — it builds a small crate eight ways — and it is worth it. This
//! is the difference between "the gates work" as a claim about a test double
//! and as a fact about the tool.

use binmap_build::engine::BinmapEngine;
use binmap_build::sweep::SweepState;
use binmap_core::config::ProjectConfig;
use binmap_core::event::{Cancellation, RecordedEvents, RunId};
use binmap_core::evidence::ToolInvocation;
use binmap_core::gate::Gate;
use binmap_verify::GatePlan;

fn corpus(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the workspace root")
        .join("corpus")
        .join(name)
}

/// A configuration whose tests fail *under that configuration* is rejected,
/// and one whose tests pass is not.
///
/// The whole sweep is gated by the user's own `cargo build` and `cargo test`,
/// which know nothing about configurations. If the environment is not threaded
/// through, every candidate is gated on the same default build and all eight
/// pass — which is what happened until an audit found it.
#[test]
fn the_gates_reject_exactly_the_configurations_that_break_the_suite() {
    let root = corpus("generics");
    assert!(root.join("binmap.toml").exists(), "the corpus crate declares its own matrix");

    let mut config = ProjectConfig::new(&root);
    // Somewhere disposable, so a developer's corpus checkout is not left with
    // eight builds after `cargo test`.
    config.target_directory =
        std::env::temp_dir().join(format!("binmap-corpus-{}", std::process::id()));

    let gates = GatePlan::new(ToolInvocation::new("cargo", ["build", "--release", "--quiet"]))
        .testing_with(ToolInvocation::new("cargo", ["test", "--release", "--quiet"]));

    let engine = BinmapEngine::open(config.clone(), gates).expect("the corpus crate opens");
    let target = engine.target_by_id("generics::generics").expect("its library target");

    let run = RunId("corpus-0001".into());
    let events = RecordedEvents::new();
    engine.sweep_blocking(run.clone(), &target, &events, &Cancellation::new());

    let state: SweepState = engine.run_state(&run).expect("the sweep leaves state");
    let _ = std::fs::remove_dir_all(&config.target_directory);

    assert_eq!(state.measured.len(), 8, "binmap.toml describes eight configurations");

    let (checked, unchecked): (Vec<_>, Vec<_>) = state
        .measured
        .iter()
        .partition(|measured| measured.configuration.overflow_checks == Some(true));

    assert_eq!(checked.len(), 4);
    assert_eq!(unchecked.len(), 4);

    for measured in &checked {
        assert_eq!(
            measured.report.rejected_by(),
            Some(Gate::TestsPass),
            "{} asserts wrapping and was built with overflow-checks on, so its suite fails — \
             it must be rejected, and by TestsPass",
            measured.name
        );
    }

    for measured in &unchecked {
        assert!(
            measured.report.passed(),
            "{} was rejected by {:?} but its suite passes under its own configuration",
            measured.name,
            measured.report.rejected_by()
        );
    }

    // And the frontier only contains configurations that actually work.
    for index in state.frontier() {
        assert!(state.measured[index].report.passed());
    }
}
