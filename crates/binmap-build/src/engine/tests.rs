use super::*;
use binmap_core::event::RecordedEvents;
use binmap_core::evidence::ToolInvocation;

fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap().to_path_buf()
}

/// An engine opened on our own workspace, with a matrix small enough to be a
/// test and gates that do not actually build anything.
fn engine() -> BinmapEngine {
    let mut config = ProjectConfig::new(workspace_root());
    config.target_directory = std::env::temp_dir().join("binmap-engine-tests");
    let gates = GatePlan::new(ToolInvocation::new("sh", ["-c", "true"]));
    BinmapEngine::open(config, gates).expect("our own workspace opens")
}

#[test]
fn opening_a_project_enumerates_its_targets_up_front() {
    let engine = engine();
    let targets = engine.targets().unwrap();
    assert!(targets.iter().any(|t| t.id == "binmap-gui::binmap"), "{targets:?}");
    // Every one states what it can do, in the words the project view uses.
    assert!(targets.iter().all(|t| !t.capabilities.is_empty()));
}

#[test]
fn opening_something_that_is_not_a_project_fails_by_path() {
    let config = ProjectConfig::new("/tmp/definitely-not-a-cargo-project");
    let gates = GatePlan::new(ToolInvocation::new("sh", ["-c", "true"]));
    let Err(error) = BinmapEngine::open(config, gates) else {
        panic!("/tmp/definitely-not-a-cargo-project is not a cargo project");
    };
    assert!(matches!(error, Error::NoProject(_)), "{error}");
}

#[test]
fn the_engine_starts_at_the_lowest_tier_and_only_moves_when_told() {
    let engine = engine();
    assert_eq!(engine.trust_tier(), TrustTier::Observe);
    engine.set_trust_tier(TrustTier::Apply);
    assert_eq!(engine.trust_tier(), TrustTier::Apply);
}

#[test]
fn asking_for_a_target_that_is_not_there_says_which_one() {
    let engine = engine();
    let error = engine
        .start(Request::Sweep { target: "ghost::ghost".into() }, Arc::new(RecordedEvents::new()))
        .unwrap_err();
    assert!(error.to_string().contains("ghost::ghost"), "{error}");
}

#[test]
fn resuming_a_sweep_that_never_ran_says_so_rather_than_starting_one() {
    let engine = engine();
    let error = engine
        .start(
            Request::ResumeSweep { run: RunId("run-9999".into()) },
            Arc::new(RecordedEvents::new()),
        )
        .unwrap_err();
    assert!(error.to_string().contains("run-9999"), "{error}");
}

#[test]
fn phase_zero_says_plainly_that_applying_is_not_built_yet() {
    let engine = engine();
    let error = engine
        .start(Request::Apply { proposal: "apply-ols".into() }, Arc::new(RecordedEvents::new()))
        .unwrap_err();
    let message = error.to_string();
    assert!(message.contains("apply-ols"), "{message}");
    assert!(message.contains("apply dialog"), "{message}");
}

#[test]
fn a_proposal_shows_the_diff_without_writing_anything() {
    let engine = engine();
    let manifest = workspace_root().join("Cargo.toml");
    let before = std::fs::read_to_string(&manifest).unwrap();

    let configuration = BuildConfiguration {
        opt_level: Some(binmap_core::config::OptLevel::Size),
        ..Default::default()
    };
    let proposal = engine.propose_configuration(&configuration).unwrap();

    assert_eq!(proposal.writes, vec![manifest.clone()]);
    assert!(proposal.diff.contains("+opt-level = \"s\""), "{}", proposal.diff);
    assert!(proposal.summary.contains("[profile.release]"));
    // The manifest is untouched: a proposal is a proposal.
    assert_eq!(std::fs::read_to_string(&manifest).unwrap(), before);
    assert_eq!(engine.proposals().len(), 1);
}

#[test]
fn probing_the_environment_goes_through_the_facade_and_records_itself() {
    let engine = engine();
    let probes = engine.probe_environment();
    assert!(probes.iter().any(|probe| probe.name == "cargo"));
    // Probes run tools, and tools are recorded.
    assert!(!engine.evidence_store().is_empty());
}

#[test]
fn evidence_for_a_finding_that_does_not_exist_is_empty_rather_than_a_panic() {
    let engine = engine();
    assert!(engine.evidence("no-such-finding").is_empty());
    assert!(engine.findings().is_empty());
}

#[test]
fn a_run_adopted_from_a_previous_session_can_be_resumed() {
    let engine = engine();
    let run = RunId("run-0001".into());
    let state = SweepState::new(run.clone(), "binmap-gui::binmap", Vec::new());
    engine.adopt_run(state);

    assert!(engine.run_state(&run).is_some());
    assert_eq!(engine.run_states().len(), 1);
    // With an empty matrix there is nothing to build, so this resolves the
    // target and returns rather than erroring.
    assert!(
        engine
            .start(Request::ResumeSweep { run }, Arc::new(RecordedEvents::new()))
            .is_ok()
    );
}
