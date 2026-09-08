use super::*;
use binmap_core::evidence::{EvidenceStore, ToolInvocation};
use binmap_core::finding::{Confidence, FindingDraft, Provenance};
use binmap_core::location::Location;

fn target(id: &str, capabilities: &[Capability]) -> Target {
    Target {
        id: id.into(),
        name: id.rsplit("::").next().unwrap_or(id).into(),
        family: TargetFamily::Rust,
        package: id.split("::").next().unwrap_or(id).into(),
        manifest: "Cargo.toml".into(),
        capabilities: capabilities.iter().copied().collect(),
    }
}

/// A grounded finding, built the only way findings can be built.
fn finding(id: &str, kind: FindingKind) -> Finding {
    let store = EvidenceStore::new();
    let pending = store.begin(ToolInvocation::new("cargo", ["build"]));
    let evidence = store.complete(pending, "built", 0);
    Finding::new(
        FindingDraft::new(id, kind, format!("{id} happened"))
            .cite(evidence)
            .at(Location::configuration(id))
            .confidence(Confidence::Certain)
            .provenance(Provenance::Measured),
        &store,
    )
    .unwrap()
}

fn run() -> RunId {
    RunId("run-0001".into())
}

fn started(total: Option<usize>) -> EngineEvent {
    EngineEvent::Started { run: run(), description: "Sweeping".into(), total }
}

#[test]
fn a_view_the_target_cannot_support_is_absent_rather_than_empty() {
    let mut state = AppState::new();
    state.set_targets(vec![target("app::app", &[Capability::ConfigurationSweep])]);

    let views: Vec<View> = state.nav_entries().into_iter().map(|entry| entry.view).collect();
    assert_eq!(views, vec![View::Target, View::Tune, View::Agent, View::Environment]);
    // Not "present and disabled" — simply not there.
    assert!(!views.contains(&View::Replay));
    assert!(!views.contains(&View::Size));
}

#[test]
fn selecting_a_view_the_target_cannot_support_is_refused() {
    let mut state = AppState::new();
    state.set_targets(vec![target("app::app", &[Capability::ConfigurationSweep])]);

    assert!(!state.select_view(View::Replay));
    assert_eq!(state.view(), None);
    assert!(state.select_view(View::Tune));
    assert_eq!(state.view(), Some(View::Tune));
}

#[test]
fn switching_to_a_weaker_target_drops_a_view_it_cannot_support() {
    let mut state = AppState::new();
    state.set_targets(vec![
        target("rich::rich", &[Capability::ConfigurationSweep, Capability::ReplayDebugging]),
        target("plain::plain", &[Capability::ConfigurationSweep]),
    ]);

    assert!(state.select_target("rich::rich"));
    assert!(state.select_view(View::Replay));

    assert!(state.select_target("plain::plain"));
    assert_eq!(state.view(), None, "the Replay view cannot survive the switch");
}

#[test]
fn targets_are_grouped_by_language_for_the_project_view() {
    let mut state = AppState::new();
    let mut web = target("site::bundle", &[Capability::SizeAttribution]);
    web.family = TargetFamily::TypeScript;
    state.set_targets(vec![target("app::app", &[Capability::ConfigurationSweep]), web]);

    let groups = state.targets_by_family();
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].0, TargetFamily::Rust);
    assert_eq!(groups[1].0, TargetFamily::TypeScript);
}

#[test]
fn findings_appear_as_they_are_discovered_and_the_rail_counts_them() {
    let mut state = AppState::new();
    state.set_targets(vec![target("app::app", &[Capability::ConfigurationSweep])]);
    state.apply(started(Some(3)));

    let tune_count = |state: &AppState| {
        state.nav_entries().iter().find(|e| e.view == View::Tune).unwrap().findings
    };
    assert_eq!(tune_count(&state), 0);

    for (index, kind) in
        [FindingKind::Configuration, FindingKind::FrontierPoint, FindingKind::RejectedCandidate]
            .into_iter()
            .enumerate()
    {
        state.apply(EngineEvent::Finding {
            run: run(),
            finding: Box::new(finding(&format!("f{index}"), kind)),
        });
        assert_eq!(tune_count(&state), index + 1, "the count did not follow the finding");
    }
}

#[test]
fn the_first_finding_selects_itself_so_the_inspector_is_never_blank_for_nothing() {
    let mut state = AppState::new();
    state.apply(started(Some(1)));
    assert!(state.selected_finding().is_none());

    state.apply(EngineEvent::Finding {
        run: run(),
        finding: Box::new(finding("f0", FindingKind::Configuration)),
    });
    assert_eq!(state.selected_finding().map(|f| f.id.as_str()), Some("f0"));

    // A second finding does not steal the selection.
    state.apply(EngineEvent::Finding {
        run: run(),
        finding: Box::new(finding("f1", FindingKind::Configuration)),
    });
    assert_eq!(state.selected_finding().map(|f| f.id.as_str()), Some("f0"));
}

#[test]
fn a_finding_that_arrives_twice_replaces_rather_than_duplicates() {
    let mut state = AppState::new();
    state.apply(started(None));
    for _ in 0..2 {
        state.apply(EngineEvent::Finding {
            run: run(),
            finding: Box::new(finding("f0", FindingKind::Configuration)),
        });
    }
    assert_eq!(state.findings().len(), 1);
}

#[test]
fn progress_never_goes_backwards_when_workers_report_out_of_order() {
    let mut state = AppState::new();
    state.apply(started(Some(10)));

    for completed in [3, 7, 5, 6] {
        state.apply(EngineEvent::Progress {
            run: run(),
            completed,
            message: format!("{completed} done"),
        });
    }
    assert_eq!(state.run(&run()).unwrap().completed, 7);
}

#[test]
fn a_finished_run_reads_as_complete_even_if_an_event_was_missed() {
    let mut state = AppState::new();
    state.apply(started(Some(96)));
    state.apply(EngineEvent::Progress { run: run(), completed: 94, message: "nearly".into() });
    state.apply(EngineEvent::Finished { run: run(), summary: "96 measured".into() });

    let progress = state.run(&run()).unwrap();
    assert_eq!(progress.completed, 96);
    assert_eq!(progress.fraction(), Some(1.0));
    assert!(!progress.is_running());
    assert!(!state.is_busy());
}

#[test]
fn cancelling_keeps_the_findings_and_is_not_reported_as_a_failure() {
    let mut state = AppState::new();
    state.apply(started(Some(96)));
    state.apply(EngineEvent::Finding {
        run: run(),
        finding: Box::new(finding("f0", FindingKind::Configuration)),
    });
    state.apply(EngineEvent::Cancelled { run: run(), completed: 12 });

    assert_eq!(state.findings().len(), 1, "cancelling must not discard measurements");
    let progress = state.run(&run()).unwrap();
    assert!(matches!(progress.state, RunPhase::Cancelled { completed: 12 }));
    assert!(!matches!(progress.state, RunPhase::Failed { .. }));
}

#[test]
fn a_run_with_no_known_total_renders_indeterminate_rather_than_guessing() {
    let mut state = AppState::new();
    state.apply(started(None));
    state.apply(EngineEvent::Progress { run: run(), completed: 4, message: "working".into() });
    assert_eq!(state.run(&run()).unwrap().fraction(), None);
}

#[test]
fn events_for_a_run_we_never_saw_start_are_ignored_rather_than_invented() {
    let mut state = AppState::new();
    state.apply(EngineEvent::Progress {
        run: RunId("ghost".into()),
        completed: 5,
        message: "from nowhere".into(),
    });
    assert!(state.runs().next().is_none());
}

#[test]
fn probes_keep_their_reported_grouping_and_the_unmet_ones_are_counted() {
    let mut state = AppState::new();
    state.set_probes(vec![
        Probe {
            group: "Rust".into(),
            name: "cargo".into(),
            status: ProbeStatus::Present,
            detail: "cargo 1.96.0".into(),
            remedy: None,
        },
        Probe {
            group: "Verification".into(),
            name: "miri".into(),
            status: ProbeStatus::Missing,
            detail: "unsafe code will be reported as unchecked".into(),
            remedy: Some("rustup toolchain install nightly --component miri".into()),
        },
        Probe {
            group: "Rust".into(),
            name: "rustc".into(),
            status: ProbeStatus::Present,
            detail: "rustc 1.96.0".into(),
            remedy: None,
        },
    ]);

    let groups = state.probes_by_group();
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].0, "Rust");
    assert_eq!(groups[0].1.len(), 2, "a later Rust probe joins the group it named");
    assert_eq!(state.unmet_probes(), 1);
}

#[test]
fn selecting_something_that_is_not_there_is_refused_rather_than_stored() {
    let mut state = AppState::new();
    state.set_targets(vec![target("app::app", &[Capability::ConfigurationSweep])]);
    assert!(!state.select_target("ghost::ghost"));
    assert!(!state.select_finding("no-such-finding"));
    assert_eq!(state.selected_target().map(|t| t.id.as_str()), Some("app::app"));
}
