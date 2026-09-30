use super::*;
use binmap_core::evidence::{EvidenceStore, ToolInvocation};
use binmap_core::finding::{Confidence, FindingDraft, Provenance};
use binmap_core::location::Location;

fn target(id: &str, capabilities: &[Capability]) -> Target {
    Target {
        id: id.into(),
        name: id.rsplit("::").next().unwrap_or(id).into(),
        family: TargetFamily::Rust,
        kind: "bin".to_string(),
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
    assert_eq!(state.selected_finding().map(|f| f.id()), Some("f0"));

    // A second finding does not steal the selection.
    state.apply(EngineEvent::Finding {
        run: run(),
        finding: Box::new(finding("f1", FindingKind::Configuration)),
    });
    assert_eq!(state.selected_finding().map(|f| f.id()), Some("f0"));
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
        Probe::present("Rust target", "cargo", "cargo 1.96.0"),
        Probe::needs(
            "Verification",
            "miri",
            ProbeStatus::Missing,
            "unsafe code will be reported as unchecked",
            "rustup toolchain install nightly --component miri",
        )
        .with_action("Install"),
        Probe::present("Rust target", "rustc", "rustc 1.96.0"),
    ]);

    let groups = state.probes_by_group();
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].0, "Rust target");
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

// --- the reasoner and the transcript (`U1.3`) -------------------------------

use binmap_core::reasoner::{Mode, Reasoner, Unavailable};
use binmap_core::transcript::{Origin, StopReason};

fn a_local_reasoner() -> Reasoner {
    Reasoner {
        id: "local/qwen3-coder".into(),
        display: "Local · Qwen3 Coder".into(),
        mode: Mode::Native,
        cloud: false,
        unavailable: None,
        cost_per_mtok: None,
    }
}

fn a_forbidden_cloud_reasoner() -> Reasoner {
    Reasoner {
        id: "openai/gpt-5".into(),
        display: "OpenAI · GPT-5".into(),
        mode: Mode::Native,
        cloud: true,
        unavailable: Some(Unavailable::CloudForbidden),
        cost_per_mtok: Some((1.25, 10.0)),
    }
}

#[test]
fn the_default_reasoner_calls_no_model() {
    // §7's phase table has the product working with no model at all, so a
    // default that reached for one would contradict the product.
    let state = AppState::new();
    assert_eq!(state.reasoner().selected, "none");
    assert!(!state.reasoner().uses_a_model());
    assert!(!state.can_reason(), "with no target and no model there is nothing to ask");
}

#[test]
fn choosing_a_reasoner_and_a_target_is_what_makes_asking_possible() {
    let mut state = AppState::new();
    state.set_targets(vec![target("crate::bin", &[Capability::ConfigurationSweep])]);
    state.set_reasoners(vec![a_local_reasoner(), Reasoner::none()]);

    assert!(!state.can_reason(), "a target alone is not enough");
    assert!(state.select_reasoner("local/qwen3-coder"));
    assert!(state.can_reason());
}

#[test]
fn choosing_an_unavailable_reasoner_keeps_the_reason_and_changes_nothing() {
    // The reason is already known. Discovering it at the first model call would
    // report a setup problem as a session failure.
    let mut state = AppState::new();
    state.set_reasoners(vec![a_forbidden_cloud_reasoner(), Reasoner::none()]);

    assert!(!state.select_reasoner("openai/gpt-5"));
    assert_eq!(state.reasoner().selected, "none", "the selection did not move");

    let refusal = state.reasoner_refusal().expect("the reason is kept for the panel");
    assert!(refusal.contains("does not allow cloud"), "{refusal}");
}

#[test]
fn a_successful_choice_clears_an_earlier_refusal() {
    // A stale refusal beside a working selection reads as a current problem.
    let mut state = AppState::new();
    state.set_reasoners(vec![a_forbidden_cloud_reasoner(), a_local_reasoner(), Reasoner::none()]);

    assert!(!state.select_reasoner("openai/gpt-5"));
    assert!(state.reasoner_refusal().is_some());

    assert!(state.select_reasoner("local/qwen3-coder"));
    assert!(state.reasoner_refusal().is_none(), "the old refusal must not linger");
}

#[test]
fn a_selection_that_is_no_longer_offered_falls_back_to_none() {
    // Re-probing can remove a reasoner — a key was unset, a project forbade the
    // cloud. Staying pointed at it would leave a chosen row that cannot run.
    let mut state = AppState::new();
    state.set_reasoners(vec![a_local_reasoner(), Reasoner::none()]);
    assert!(state.select_reasoner("local/qwen3-coder"));

    state.set_reasoners(vec![Reasoner::none()]);
    assert_eq!(state.reasoner().selected, "none");
}

#[test]
fn a_selection_that_survives_a_reprobe_is_kept() {
    let mut state = AppState::new();
    state.set_reasoners(vec![a_local_reasoner(), Reasoner::none()]);
    assert!(state.select_reasoner("local/qwen3-coder"));

    state.set_reasoners(vec![a_local_reasoner(), a_forbidden_cloud_reasoner(), Reasoner::none()]);
    assert_eq!(state.reasoner().selected, "local/qwen3-coder", "a working choice is not reset");
}

#[test]
fn a_selection_that_becomes_unavailable_falls_back_even_though_it_is_still_listed() {
    // The subtle case: the row is still there, but greyed out. Keeping it
    // selected would show a chosen row the Ask button cannot use.
    let mut state = AppState::new();
    state.set_reasoners(vec![a_local_reasoner(), Reasoner::none()]);
    assert!(state.select_reasoner("local/qwen3-coder"));

    let now_unavailable = Reasoner {
        unavailable: Some(Unavailable::NoCredential { variable: "X".into() }),
        ..a_local_reasoner()
    };
    state.set_reasoners(vec![now_unavailable, Reasoner::none()]);
    assert_eq!(state.reasoner().selected, "none");
}

#[test]
fn transcript_events_arrive_on_the_engines_own_stream() {
    // Not a second channel: the panel is updated by the mechanism that already
    // updates every other view, so the ordering is the one everything else has.
    let mut state = AppState::new();
    let run = RunId("reason-1".into());

    state.apply(EngineEvent::Transcript {
        run: run.clone(),
        event: Box::new(TranscriptEvent::Started {
            reasoner: "local/qwen3-coder".into(),
            origin: Origin::Native,
        }),
    });
    state.apply(EngineEvent::Transcript {
        run,
        event: Box::new(TranscriptEvent::Hypothesis {
            step: 2,
            belief: "fmt dominates".into(),
            refuted_by: "small fmt symbols".into(),
        }),
    });

    assert_eq!(state.transcript().len(), 2);
    assert_eq!(state.session_cost().steps, 2, "the meter reads the step from the transcript");
}

#[test]
fn a_new_session_replaces_the_previous_transcript() {
    // Appending would run two sessions together into one unreadable list.
    let mut state = AppState::new();
    let run = RunId("reason-1".into());
    let started = |name: &str| EngineEvent::Transcript {
        run: RunId("reason-1".into()),
        event: Box::new(TranscriptEvent::Started { reasoner: name.into(), origin: Origin::Native }),
    };

    state.apply(started("first"));
    state.apply(EngineEvent::Transcript {
        run,
        event: Box::new(TranscriptEvent::Message {
            text: "thinking".into(),
            origin: Origin::Native,
        }),
    });
    assert_eq!(state.transcript().len(), 2);

    state.apply(started("second"));
    assert_eq!(state.transcript().len(), 1, "the new session starts clean");
    assert_eq!(state.session_cost().steps, 0, "and so does the meter");
}

#[test]
fn a_finished_session_records_why_it_stopped_and_how_long_it_took() {
    let mut state = AppState::new();
    state.apply(EngineEvent::Transcript {
        run: RunId("reason-1".into()),
        event: Box::new(TranscriptEvent::Finished {
            reason: StopReason::BudgetExhausted { limit: "steps (25 of 25)".into() },
            steps: 25,
            elapsed: std::time::Duration::from_secs(42),
        }),
    });

    match state.transcript().stop_reason() {
        Some(StopReason::BudgetExhausted { limit }) => assert!(limit.contains("25"), "{limit}"),
        other => panic!("expected the budget reason, got {other:?}"),
    }
    assert_eq!(state.session_cost().steps, 25);
    assert_eq!(state.session_cost().elapsed.as_secs(), 42);
}

#[test]
fn the_cost_meter_says_free_rather_than_zero_for_a_local_model() {
    // "free" and "$0.00" mean different things: one is a local model, the other
    // is a priced model nobody has called yet.
    let mut cost = SessionCost { max_steps: 25, ..SessionCost::default() };
    assert!(cost.label().contains("free"), "{}", cost.label());

    cost.cost = Some(0.0);
    assert!(cost.label().contains("$0.00"), "{}", cost.label());
}

#[test]
fn a_cost_under_a_cent_does_not_round_to_free() {
    // "$0.00" beside real spending reads as free when it is not.
    let cost = SessionCost { max_steps: 25, cost: Some(0.004), ..SessionCost::default() };
    assert!(cost.label().contains("under $0.01"), "{}", cost.label());
}

#[test]
fn the_budget_bar_never_leaves_its_track() {
    // A run that overshot its budget must not draw past the end of the bar.
    for (steps, max, expected) in [(0, 25, 0.0), (25, 25, 1.0), (30, 25, 1.0), (5, 0, 0.0)] {
        let cost = SessionCost { steps, max_steps: max, ..SessionCost::default() };
        assert_eq!(cost.step_fraction(), expected, "{steps} of {max}");
    }
}

#[test]
fn the_budget_meter_shows_a_real_budget_before_anything_has_run() {
    // It read "0 of 0 steps", which says nothing and looks like a bug. Seen
    // only by opening the panel.
    let state = AppState::new();
    let label = state.session_cost().label();
    assert!(!label.contains("of 0 steps"), "a budget of zero is not a budget: {label}");
    assert!(label.contains("of 25 steps"), "{label}");
}

#[test]
fn a_meter_with_no_budget_at_all_omits_the_denominator() {
    let cost = SessionCost { steps: 3, max_steps: 0, ..SessionCost::default() };
    let label = cost.label();
    assert!(label.contains("3 steps"), "{label}");
    assert!(!label.contains("of 0"), "{label}");
}

// --- the crash pane (`U2.1`–`U2.3`) ----------------------------------------

use binmap_core::crash::{CrashReport, FrameConfidence, Grounding, StackEntry};

fn entry(frame: usize, function: &str, line: Option<u32>, inlined: bool) -> StackEntry {
    StackEntry {
        frame,
        function: Some(function.into()),
        file: line.map(|_| "src/main.rs".to_string()),
        line,
        inlined,
        confidence: FrameConfidence::Certain,
        module: Some("app".into()),
        address: 0x1000 + frame as u64,
    }
}

fn a_report(entries: Vec<StackEntry>) -> CrashReport {
    let total = entries.len();
    let resolved = entries.iter().filter(|e| e.line.is_some()).count();
    CrashReport {
        title: "null pointer dereference".into(),
        what_to_look_at: "what was expected to be non-null".into(),
        entries,
        provenance: Grounding {
            binary_matches: true,
            correspondence: "same build id".into(),
            bias_corroborated: true,
            resolved,
            total,
            incomplete_because: None,
        },
        pid: 42,
        signal: 11,
    }
}

#[test]
fn opening_a_crash_lands_on_the_readers_own_code() {
    // The innermost frame of a panic is `pthread_kill`, and nobody opened a
    // debugger to look at that.
    let mut state = AppState::new();
    state.set_own_crates(vec!["app".into()]);
    state.set_crash(a_report(vec![
        entry(0, "pthread_kill", Some(1), false),
        entry(1, "abort", Some(2), false),
        entry(2, "app::do_the_thing", Some(40), false),
    ]));

    assert_eq!(state.selected_frame(), Some(2), "the first frame that is ours");
}

#[test]
fn a_crash_with_none_of_your_code_selects_nothing_rather_than_guessing() {
    let mut state = AppState::new();
    state.set_own_crates(vec!["app".into()]);
    state.set_crash(a_report(vec![entry(0, "libc::abort", Some(1), false)]));
    assert_eq!(state.selected_frame(), None);
}

#[test]
fn a_frame_with_no_source_location_cannot_be_followed() {
    // Selecting one would scroll the source pane to nothing.
    let mut state = AppState::new();
    state.set_own_crates(vec!["app".into()]);
    state.set_crash(a_report(vec![
        entry(0, "app::a", Some(10), false),
        entry(1, "stripped_thing", None, false),
    ]));

    assert!(state.select_frame(0));
    assert_eq!(state.selected_frame(), Some(0));
    assert!(!state.select_frame(1), "a frame with no line is not selectable");
    assert_eq!(state.selected_frame(), Some(0), "and the selection did not move");
}

#[test]
fn selecting_a_frame_that_does_not_exist_is_refused() {
    let mut state = AppState::new();
    state.set_crash(a_report(vec![entry(0, "app::a", Some(10), false)]));
    assert!(!state.select_frame(99));
}

#[test]
fn learning_which_crates_are_yours_reselects_the_frame() {
    // Discovery can arrive after a crash is loaded, and a pane still pointing
    // at libc would be showing the wrong thing.
    let mut state = AppState::new();
    state.set_crash(a_report(vec![
        entry(0, "libc::abort", Some(1), false),
        entry(1, "app::main", Some(7), false),
    ]));
    assert_eq!(state.selected_frame(), None, "nothing is known to be ours yet");

    state.set_own_crates(vec!["app".into()]);
    assert_eq!(state.selected_frame(), Some(1));
}

#[test]
fn inlined_entries_share_the_frame_they_were_inlined_into() {
    // Counting them as separate frames would make the stack longer than it
    // was; hiding them loses the frames a reader most wants.
    let report = a_report(vec![
        entry(0, "core::ptr::write_volatile", Some(1440), true),
        entry(0, "app::null_write", Some(40), false),
        entry(1, "app::main", Some(79), false),
    ]);

    assert_eq!(report.frame_count(), 2, "two physical frames");
    assert_eq!(report.entries.len(), 3, "three entries");
    assert_eq!(report.inlined_count(), 1);
    assert!(report.describe().contains("inlined"), "{}", report.describe());
}

#[test]
fn a_trustworthy_stack_carries_no_caveats() {
    let report = a_report(vec![entry(0, "app::a", Some(1), false)]);
    assert!(report.provenance.is_trustworthy());
    assert!(report.provenance.caveats().is_empty());
}

#[test]
fn an_unproven_binary_is_the_first_caveat() {
    // A reader deciding whether to act on a stack needs this before reading
    // it, not after.
    let mut report = a_report(vec![entry(0, "app::a", Some(1), false)]);
    report.provenance.binary_matches = false;
    report.provenance.correspondence = "this binary may have been rebuilt".into();

    assert!(!report.provenance.is_trustworthy());
    let caveats = report.provenance.caveats();
    assert_eq!(caveats[0], "this binary may have been rebuilt", "{caveats:?}");
}

#[test]
fn an_incomplete_stack_and_a_poor_coverage_are_separate_caveats() {
    // Several can apply at once, and joining them into prose buries the first.
    let mut report = a_report(vec![
        entry(0, "app::a", Some(1), false),
        entry(1, "nothing", None, false),
        entry(2, "nothing", None, false),
    ]);
    report.provenance.incomplete_because = Some("no unwind information".into());

    let caveats = report.provenance.caveats();
    assert!(caveats.len() >= 2, "{caveats:?}");
    assert!(caveats.iter().any(|c| c.contains("incomplete")), "{caveats:?}");
    assert!(caveats.iter().any(|c| c.contains("debug information")), "{caveats:?}");
}

#[test]
fn a_frame_is_yours_only_when_it_is_in_one_of_your_crates() {
    let own = vec!["app".into(), "app_core".to_string()];
    assert!(entry(0, "app::main", Some(1), false).is_probably_yours(&own));
    assert!(entry(0, "app_core::parse", Some(1), false).is_probably_yours(&own));
    // A crate whose name merely starts the same is not yours.
    assert!(!entry(0, "application_x::thing", Some(1), false).is_probably_yours(&own));
    assert!(!entry(0, "std::rt::lang_start", Some(1), false).is_probably_yours(&own));
    assert!(
        !StackEntry { function: None, ..entry(0, "x", Some(1), false) }.is_probably_yours(&own)
    );
}

#[test]
fn a_gate_name_is_bounded_so_its_column_can_be_too() {
    // The Profile Lab gives the gate name a fixed-width column and lets the
    // detail shrink. That only works if the names are bounded — and
    // `qualified_label()` is not: with a configuration it becomes
    // `BenchmarkNotWorse { significance: 0.01 }`, forty characters, which in a
    // fixed column does not truncate. It wraps one character per line and
    // turns one row into forty. Seen by running the app.
    use binmap_core::gate::Gate;

    for gate in Gate::ALL {
        let label = gate.label();
        assert!(
            label.len() <= 20,
            "{label} is {} characters, which will not fit the gate column",
            label.len()
        );
        assert!(!label.contains(' '), "{label} should be one token");
    }
}
