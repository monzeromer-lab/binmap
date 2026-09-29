use super::*;
use binmap_core::event::RecordedEvents;
use binmap_core::evidence::ToolInvocation;

fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
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
    assert!(targets.iter().any(|t| t.id == "binmap::binmap"), "{targets:?}");
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
fn the_engine_starts_at_propose_and_only_moves_when_told() {
    // Propose is the design's default: the tool is useful without ever
    // writing, so the first write is always a decision.
    let engine = engine();
    assert_eq!(engine.trust_tier(), TrustTier::Propose);
    engine.set_trust_tier(TrustTier::Tune);
    assert_eq!(engine.trust_tier(), TrustTier::Tune);
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
fn an_apply_that_is_refused_reports_the_refusal_rather_than_failing_to_start() {
    // A refused write is an answer, not an error starting a run. The run
    // registers, reports why nothing was written, and ends — so the reason
    // lands in the run log where every other outcome does.
    let engine = engine();
    let events = RecordedEvents::new();
    let (run, _) = engine
        .start(Request::Apply { proposal: "apply-invented".into() }, Arc::new(events.clone()))
        .expect("the run registers");

    let terminal: Vec<_> = events.events().into_iter().filter(|e| e.is_terminal()).collect();
    assert_eq!(terminal.len(), 1, "one terminal event per run");
    match &terminal[0] {
        EngineEvent::Failed { run: failed, error } => {
            assert_eq!(failed, &run);
            // The tier is checked before anything else, so this is the tier's
            // refusal — the cheapest and most fundamental of the three.
            assert!(error.contains("Tune"), "{error}");
            assert!(error.contains("Propose"), "{error}");
        }
        other => panic!("expected a refusal, got {other:?}"),
    }

    // Above the tier, the unknown proposal is what refuses it.
    engine.set_trust_tier(TrustTier::Tune);
    let events = RecordedEvents::new();
    engine
        .start(Request::Apply { proposal: "apply-invented".into() }, Arc::new(events.clone()))
        .expect("the run registers");
    let error = events
        .events()
        .into_iter()
        .find_map(|event| match event {
            EngineEvent::Failed { error, .. } => Some(error),
            _ => None,
        })
        .expect("a refusal");
    assert!(error.contains("not a proposal this session produced"), "{error}");
}

#[test]
fn verifying_a_proposal_on_its_own_says_where_its_gates_already_are() {
    let engine = engine();
    let error = engine
        .start(Request::Verify { proposal: "apply-ols".into() }, Arc::new(RecordedEvents::new()))
        .unwrap_err();
    let message = error.to_string();
    assert!(message.contains("apply-ols"), "{message}");
    assert!(message.contains("beside it"), "{message}");
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
    let state = SweepState::new(run.clone(), "binmap::binmap", Vec::new());
    engine.adopt_run(state);

    assert!(engine.run_state(&run).is_some());
    assert_eq!(engine.run_states().len(), 1);
    // With an empty matrix there is nothing to build, so this resolves the
    // target and returns rather than erroring.
    assert!(engine.start(Request::ResumeSweep { run }, Arc::new(RecordedEvents::new())).is_ok());
}

#[test]
fn a_finished_sweep_is_written_to_disk_and_can_be_restored() {
    // F0.7 and F0.8's resume were both unreachable until this: SessionStore
    // had no caller outside its own tests, so nothing was ever persisted
    // however complete the types looked.
    let directory = tempfile::tempdir().expect("a temporary directory");
    let mut config = ProjectConfig::new(workspace_root());
    config.target_directory = directory.path().to_path_buf();
    config.matrix = binmap_core::config::SweepMatrix {
        opt_level: vec![binmap_core::config::OptLevel::Three],
        lto: Vec::new(),
        codegen_units: Vec::new(),
        panic: Vec::new(),
        strip: Vec::new(),
        debug: Vec::new(),
        overflow_checks: Vec::new(),
        build_std: Vec::new(),
        target_cpu: Vec::new(),
    };

    let gates = GatePlan::new(ToolInvocation::new("sh", ["-c", "true"]));
    let engine = BinmapEngine::open(config.clone(), gates.clone()).expect("opens");
    let target = engine.target_by_id("binmap::binmap").expect("our own binary");

    // An adopted run stands in for a completed one, so the test does not have
    // to build the workspace ninety-six ways to check persistence.
    let run = RunId("run-0001".into());
    engine.adopt_run(SweepState::new(run.clone(), target.id.clone(), Vec::new()));
    engine.persist_for_test(&target).expect("the session is written");

    // A fresh engine on the same project finds it.
    let reopened = BinmapEngine::open(config, gates).expect("opens again");
    assert_eq!(reopened.run_states().len(), 0, "nothing is restored until asked");

    let adopted = reopened.restore(&target.id).expect("the session reads back");
    assert_eq!(adopted, 1, "the sweep did not survive the process");
    assert!(reopened.run_state(&run).is_some());
}

#[test]
fn an_exported_session_names_what_it_redacted() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let mut config = ProjectConfig::new(workspace_root());
    config.target_directory = directory.path().to_path_buf();

    let gates = GatePlan::new(ToolInvocation::new("sh", ["-c", "true"]));
    let engine = BinmapEngine::open(config, gates).expect("opens");
    let target = engine.target_by_id("binmap::binmap").expect("our own binary");

    // Opening the project already ran cargo metadata, so there is evidence
    // carrying this machine's absolute paths.
    assert!(!engine.evidence_store().is_empty());

    let path = directory.path().join("shared.binmap.json");
    let described = engine.export(&target, &path).expect("the export is written");
    assert!(path.exists());
    assert!(
        described.starts_with("Redacted: ") || described == "Nothing needed redacting.",
        "{described}"
    );
}

// ---------------------------------------------------------------------------
// Applying a configuration (U0.2, U9, A2.4)
// ---------------------------------------------------------------------------
//
// The one place in Phase 0 that touches a file the user owns. Each gate on it
// has a test, because a write that happens when it should not is the failure
// this product cannot afford.

/// A throwaway crate with a manifest we are allowed to rewrite.
fn writable_project() -> (tempfile::TempDir, ProjectConfig) {
    let directory = tempfile::tempdir().expect("a temporary directory");
    std::fs::write(
        directory.path().join("Cargo.toml"),
        "[package]\nname = \"subject\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n\
         [dependencies]\n",
    )
    .unwrap();
    std::fs::create_dir_all(directory.path().join("src")).unwrap();
    std::fs::write(directory.path().join("src/main.rs"), "fn main() {}\n").unwrap();

    let mut config = ProjectConfig::new(directory.path());
    config.target_directory = directory.path().join("target/binmap");
    (directory, config)
}

fn engine_over(config: ProjectConfig) -> BinmapEngine {
    BinmapEngine::open(config, GatePlan::new(ToolInvocation::new("sh", ["-c", "true"])))
        .expect("the project opens")
}

/// Register a measured configuration and the proposal that would write it.
fn proposal_for(engine: &BinmapEngine, passed: bool) -> String {
    let configuration = BuildConfiguration {
        opt_level: Some(binmap_core::config::OptLevel::Size),
        lto: Some(binmap_core::config::Lto::Fat),
        ..Default::default()
    };
    let name = configuration.name();

    let mut state = SweepState::new(RunId("r".into()), "subject::subject", Vec::new());
    state.measured.push(crate::sweep::MeasuredConfiguration {
        configuration: configuration.clone(),
        name: name.clone(),
        built: true,
        artifact: None,
        size_bytes: Some(1000),
        sections: None,
        build_time_nanos: None,
        runtime_nanos: None,
        report: binmap_core::gate::VerificationReport {
            candidate: name.clone(),
            outcomes: vec![binmap_core::gate::GateOutcome::new(
                binmap_core::gate::Gate::TestsPass,
                if passed {
                    binmap_core::gate::GateResult::Passed
                } else {
                    binmap_core::gate::GateResult::Failed
                },
                if passed { "the suite passes" } else { "failures:" },
            )],
        },
        evidence: Vec::new(),
    });
    engine.adopt_run(state);

    engine.propose_configuration(&configuration).expect("a proposal is made").id
}

#[test]
fn applying_below_the_tune_tier_is_refused_and_names_the_tier_it_needs() {
    let (_d, config) = writable_project();
    let manifest = config.root.join("Cargo.toml");
    let engine = engine_over(config);
    let before = std::fs::read_to_string(&manifest).unwrap();

    // Propose is the default, and it never applies.
    let proposal = proposal_for(&engine, true);
    let error = engine.apply(&proposal).unwrap_err().to_string();

    assert!(error.contains("Tune"), "{error}");
    assert!(error.contains("Propose"), "{error}");
    assert_eq!(std::fs::read_to_string(&manifest).unwrap(), before, "the manifest was written");
}

#[test]
fn applying_at_tune_writes_the_profile_and_records_the_diff() {
    let (_d, config) = writable_project();
    let manifest = config.root.join("Cargo.toml");
    let engine = engine_over(config);
    engine.set_trust_tier(TrustTier::Tune);

    let proposal = proposal_for(&engine, true);
    let summary = engine.apply(&proposal).expect("Tune may write build configuration");
    assert!(summary.contains("opt-level=s"), "{summary}");

    let after = std::fs::read_to_string(&manifest).unwrap();
    assert!(after.contains("[profile.release]"), "{after}");
    assert!(after.contains("opt-level = \"s\""), "{after}");
    assert!(after.contains("lto = \"fat\""), "{after}");
    // The package is still there — this is an edit, not a rewrite.
    assert!(after.contains("name = \"subject\""), "{after}");

    // And the write is on the record like every other tool invocation.
    let written = engine
        .evidence_store()
        .records()
        .into_iter()
        .find(|record| record.invocation.tool == "binmap:apply")
        .expect("the write was recorded");
    assert!(written.output.contains("+opt-level"), "{}", written.output);
    assert!(written.digest_matches());
}

#[test]
fn a_configuration_its_gates_rejected_is_never_written() {
    // However good its size looked. This is what the gates are for.
    let (_d, config) = writable_project();
    let manifest = config.root.join("Cargo.toml");
    let engine = engine_over(config);
    engine.set_trust_tier(TrustTier::Tune);

    let proposal = proposal_for(&engine, false);
    let before = std::fs::read_to_string(&manifest).unwrap();
    let error = engine.apply(&proposal).unwrap_err().to_string();

    assert!(error.contains("TestsPass"), "the failing gate is named: {error}");
    assert_eq!(std::fs::read_to_string(&manifest).unwrap(), before);
}

#[test]
fn a_proposal_we_never_issued_is_refused() {
    // The same class of mistake as citing evidence we never issued.
    let (_d, config) = writable_project();
    let engine = engine_over(config);
    engine.set_trust_tier(TrustTier::Autonomous);

    let error = engine.apply("apply-invented").unwrap_err().to_string();
    assert!(error.contains("not a proposal this session produced"), "{error}");
}
