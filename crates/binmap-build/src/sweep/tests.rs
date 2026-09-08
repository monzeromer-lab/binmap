use super::*;
use binmap_core::capability::Capabilities;
use binmap_core::config::{Lto, OptLevel, SweepMatrix};
use binmap_core::event::RecordedEvents;
use binmap_core::evidence::{EvidenceStore, ToolInvocation};
use binmap_core::tool::ToolRunner;
use binmap_core::traits::{BuildOutcome, TargetFamily};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU32;

/// A build system that writes files of a size we choose, so a sweep can be
/// tested end to end without waiting for rustc.
struct FakeCargo {
    runner: ToolRunner,
    directory: PathBuf,
    /// Bytes per configuration name; anything unlisted gets the default.
    sizes: BTreeMap<String, u64>,
    default_size: u64,
    /// Configuration names whose build fails.
    broken: Vec<String>,
    builds: AtomicU32,
}

impl FakeCargo {
    fn new(runner: ToolRunner, directory: PathBuf, default_size: u64) -> Self {
        Self {
            runner,
            directory,
            sizes: BTreeMap::new(),
            default_size,
            broken: Vec::new(),
            builds: AtomicU32::new(0),
        }
    }

    fn sized(mut self, name: &str, bytes: u64) -> Self {
        self.sizes.insert(name.to_string(), bytes);
        self
    }

    fn breaking(mut self, name: &str) -> Self {
        self.broken.push(name.to_string());
        self
    }
}

impl BuildSystem for FakeCargo {
    fn targets(&self, _root: &Path) -> Result<Vec<Target>> {
        Ok(vec![target()])
    }

    fn build(&self, _target: &Target, configuration: &BuildConfiguration) -> Result<BuildOutcome> {
        self.builds.fetch_add(1, Ordering::SeqCst);
        let name = configuration.name();
        // Recorded like a real build would be, so findings have evidence to cite.
        let pending = self
            .runner
            .store()
            .begin(ToolInvocation::new("cargo", ["build", "--release", "--config", &name]));

        if self.broken.contains(&name) {
            let evidence = self.runner.store().complete(pending, "error: it does not build", 1);
            return Ok(BuildOutcome {
                configuration: configuration.clone(),
                artifact: None,
                succeeded: false,
                duration: Duration::from_millis(10),
                warnings: 0,
                evidence,
            });
        }

        let bytes = self.sizes.get(&name).copied().unwrap_or(self.default_size);
        std::fs::create_dir_all(&self.directory).unwrap();
        let artifact = self.directory.join(&name);
        std::fs::write(&artifact, vec![0u8; bytes as usize]).unwrap();
        let evidence = self.runner.store().complete(pending, format!("built {name}"), 0);

        Ok(BuildOutcome {
            configuration: configuration.clone(),
            artifact: Some(artifact),
            succeeded: true,
            duration: Duration::from_millis(100),
            warnings: 0,
            evidence,
        })
    }
}

fn target() -> Target {
    Target {
        id: "app::app".into(),
        name: "app".into(),
        family: TargetFamily::Rust,
        package: "app".into(),
        manifest: "Cargo.toml".into(),
        capabilities: Capabilities::none(),
    }
}

fn shell(script: &str) -> ToolInvocation {
    ToolInvocation::new("sh", ["-c", script])
}

fn passing_gates() -> GatePlan {
    GatePlan::new(shell("true")).testing_with(shell("true"))
}

struct Fixture {
    _directory: tempfile::TempDir,
    runner: ToolRunner,
    store: EvidenceStore,
    path: PathBuf,
}

fn fixture() -> Fixture {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let store = EvidenceStore::new();
    let runner = ToolRunner::new(store.clone(), directory.path());
    let path = directory.path().to_path_buf();
    Fixture { _directory: directory, runner, store, path }
}

/// A three-point matrix: two axes, small enough to read in a failure message.
fn small_matrix() -> SweepMatrix {
    SweepMatrix {
        opt_level: vec![OptLevel::Three, OptLevel::Size],
        lto: vec![Lto::Off, Lto::Fat],
        codegen_units: Vec::new(),
        panic: Vec::new(),
        strip: Vec::new(),
        debug: Vec::new(),
        overflow_checks: Vec::new(),
        build_std: Vec::new(),
        target_cpu: Vec::new(),
    }
}

fn state_for(matrix: &SweepMatrix) -> SweepState {
    SweepState::new(RunId("run-1".into()), "app::app", BuildConfiguration::expand(matrix))
}

#[test]
fn a_sweep_measures_every_configuration_and_states_them_against_the_baseline() {
    let fixture = fixture();
    // Default release is what every number is stated against, so it is the
    // one configuration whose size the fixture pins explicitly.
    let baseline = BuildConfiguration::default_release().name();
    let builder = FakeCargo::new(fixture.runner.clone(), fixture.path.clone(), 1000)
        .sized(&baseline, 4000);
    let sweep = Sweep {
        builder: &builder,
        runner: &fixture.runner,
        benchmark: None,
        options: SweepOptions::new(passing_gates()),
    };

    let matrix = small_matrix();
    let mut state = state_for(&matrix);
    let events = RecordedEvents::new();
    sweep.run(&target(), &mut state, &events, &Cancellation::new()).unwrap();

    assert_eq!(state.measured.len(), 4);
    assert!(state.complete);
    // Every number is stated against default release, which was measured first.
    assert_eq!(state.baseline_bytes, Some(4000));
    // 1000 bytes against a 4000-byte baseline is a 75% reduction.
    assert_eq!(state.best_size_reduction(), Some(0.75));

    let terminal: Vec<_> = events.events().into_iter().filter(|e| e.is_terminal()).collect();
    assert_eq!(terminal.len(), 1);
}

#[test]
fn findings_stream_as_they_are_discovered_rather_than_arriving_in_a_batch() {
    let fixture = fixture();
    let builder = FakeCargo::new(fixture.runner.clone(), fixture.path.clone(), 1000);
    let sweep = Sweep {
        builder: &builder,
        runner: &fixture.runner,
        benchmark: None,
        options: SweepOptions::new(passing_gates()),
    };

    let matrix = small_matrix();
    let mut state = state_for(&matrix);
    let events = RecordedEvents::new();
    sweep.run(&target(), &mut state, &events, &Cancellation::new()).unwrap();

    let all = events.events();
    let last_finding = all.iter().rposition(|e| matches!(e, EngineEvent::Finding { .. })).unwrap();
    let first_finding = all.iter().position(|e| matches!(e, EngineEvent::Finding { .. })).unwrap();
    let finished = all.iter().position(|e| matches!(e, EngineEvent::Finished { .. })).unwrap();

    assert!(first_finding < last_finding, "findings should not all arrive at once");
    assert!(last_finding < finished, "findings arrive before the run ends");

    // Every finding is grounded, by construction.
    for finding in events.findings() {
        assert!(!finding.evidence.is_empty());
        for id in &finding.evidence {
            assert!(fixture.store.issued(id));
        }
    }
}

#[test]
fn a_configuration_that_does_not_build_stays_visible_as_a_rejected_candidate() {
    let fixture = fixture();
    let builder = FakeCargo::new(fixture.runner.clone(), fixture.path.clone(), 1000)
        .breaking("ols-lfat");
    let sweep = Sweep {
        builder: &builder,
        runner: &fixture.runner,
        benchmark: None,
        // A gate plan whose build command fails, so the broken configuration
        // is actually rejected rather than merely unbuilt.
        options: SweepOptions::new(GatePlan::new(shell("true")).testing_with(shell("true"))),
    };

    let matrix = small_matrix();
    let mut state = state_for(&matrix);
    let events = RecordedEvents::new();
    sweep.run(&target(), &mut state, &events, &Cancellation::new()).unwrap();

    let broken = state.measured.iter().find(|m| m.name == "ols-lfat").expect("still in the table");
    assert!(!broken.built);
    // It has no size, so it cannot reach the frontier — but it was not dropped.
    assert_eq!(broken.size_bytes, None);
    assert_eq!(state.measured.len(), 4);

    let frontier_names: Vec<&str> =
        state.frontier().into_iter().map(|i| state.measured[i].name.as_str()).collect();
    assert!(!frontier_names.contains(&"ols-lfat"), "{frontier_names:?}");
}

#[test]
fn cancelling_keeps_what_was_measured_and_a_resume_owes_only_the_rest() {
    let fixture = fixture();
    let builder = FakeCargo::new(fixture.runner.clone(), fixture.path.clone(), 1000);
    let cancellation = Cancellation::new();
    // Cancel before any configuration is built.
    cancellation.cancel();

    let sweep = Sweep {
        builder: &builder,
        runner: &fixture.runner,
        benchmark: None,
        options: SweepOptions::new(passing_gates()),
    };
    let matrix = small_matrix();
    let mut state = state_for(&matrix);
    let events = RecordedEvents::new();
    sweep.run(&target(), &mut state, &events, &cancellation).unwrap();

    assert!(!state.complete);
    assert_eq!(state.remaining().len(), 4, "everything is still owed");
    assert!(matches!(events.events().last(), Some(EngineEvent::Cancelled { .. })));

    // Resuming finishes the job without repeating anything.
    let fresh = Cancellation::new();
    sweep.run(&target(), &mut state, &events, &fresh).unwrap();
    assert!(state.complete);
    assert_eq!(state.measured.len(), 4);
    assert!(state.remaining().is_empty());
}

#[test]
fn a_resume_does_not_rebuild_what_it_already_measured() {
    let fixture = fixture();
    let builder = FakeCargo::new(fixture.runner.clone(), fixture.path.clone(), 1000);
    let sweep = Sweep {
        builder: &builder,
        runner: &fixture.runner,
        benchmark: None,
        options: SweepOptions::new(passing_gates()),
    };
    let matrix = small_matrix();
    let mut state = state_for(&matrix);
    let events = RecordedEvents::new();

    sweep.run(&target(), &mut state, &events, &Cancellation::new()).unwrap();
    let after_first = builder.builds.load(Ordering::SeqCst);

    // Running again over a complete state builds nothing but the baseline,
    // which is already known — so nothing at all.
    sweep.run(&target(), &mut state, &events, &Cancellation::new()).unwrap();
    assert_eq!(builder.builds.load(Ordering::SeqCst), after_first);
}

#[test]
fn a_parallel_sweep_records_no_build_times_rather_than_wrong_ones() {
    let fixture = fixture();
    let builder = FakeCargo::new(fixture.runner.clone(), fixture.path.clone(), 1000);
    let options = SweepOptions::new(passing_gates()).with_parallelism(4);
    assert!(!options.build_time_is_measurable());

    let sweep =
        Sweep { builder: &builder, runner: &fixture.runner, benchmark: None, options };
    let matrix = small_matrix();
    let mut state = state_for(&matrix);
    sweep.run(&target(), &mut state, &RecordedEvents::new(), &Cancellation::new()).unwrap();

    assert_eq!(state.measured.len(), 4);
    for measured in &state.measured {
        assert_eq!(
            measured.build_time_nanos, None,
            "{} claimed a build time from a concurrent sweep",
            measured.name
        );
    }
}

#[test]
fn a_serial_sweep_does_record_build_times() {
    let fixture = fixture();
    let builder = FakeCargo::new(fixture.runner.clone(), fixture.path.clone(), 1000);
    let sweep = Sweep {
        builder: &builder,
        runner: &fixture.runner,
        benchmark: None,
        options: SweepOptions::new(passing_gates()).with_parallelism(1),
    };
    let matrix = small_matrix();
    let mut state = state_for(&matrix);
    sweep.run(&target(), &mut state, &RecordedEvents::new(), &Cancellation::new()).unwrap();

    assert!(state.measured.iter().all(|m| m.build_time_nanos.is_some()));
}

#[test]
fn the_frontier_finding_is_derived_and_says_which_rule_derived_it() {
    let fixture = fixture();
    let builder = FakeCargo::new(fixture.runner.clone(), fixture.path.clone(), 1000)
        .sized("ols-lfat", 500);
    let sweep = Sweep {
        builder: &builder,
        runner: &fixture.runner,
        benchmark: None,
        options: SweepOptions::new(passing_gates()),
    };
    let matrix = small_matrix();
    let mut state = state_for(&matrix);
    let events = RecordedEvents::new();
    sweep.run(&target(), &mut state, &events, &Cancellation::new()).unwrap();

    let frontier: Vec<_> = events
        .findings()
        .into_iter()
        .filter(|f| f.kind == FindingKind::FrontierPoint)
        .collect();
    assert!(!frontier.is_empty());
    for finding in &frontier {
        // A measurement is Certain; a conclusion about measurements is not the
        // same thing, and the badge says so.
        assert_eq!(
            finding.provenance,
            Provenance::Derived { rule: "pareto-dominance".into() }
        );
        assert_eq!(finding.confidence, Confidence::High);
        assert_eq!(finding.provenance.glyph(), '◈');
    }
}

#[test]
fn the_state_round_trips_so_a_sweep_survives_the_session() {
    let fixture = fixture();
    let builder = FakeCargo::new(fixture.runner.clone(), fixture.path.clone(), 1000);
    let sweep = Sweep {
        builder: &builder,
        runner: &fixture.runner,
        benchmark: None,
        options: SweepOptions::new(passing_gates()),
    };
    let matrix = small_matrix();
    let mut state = state_for(&matrix);
    sweep.run(&target(), &mut state, &RecordedEvents::new(), &Cancellation::new()).unwrap();

    let json = serde_json::to_string(&state).unwrap();
    let restored: SweepState = serde_json::from_str(&json).unwrap();
    assert_eq!(state, restored);
    assert!(restored.remaining().is_empty());
}
