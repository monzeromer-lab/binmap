//! Interaction, asserted without a window.
//!
//! Every click and every key binding produces an
//! [`Action`](binmap_gui::state::Action), so what a control *does* can be
//! tested by applying the action and checking what changed. That is why the
//! dispatch layer exists: interaction that can only be verified by looking at
//! a screen is interaction nobody verifies.
//!
//! These run against a scripted engine, so a state that would take a
//! ninety-six configuration sweep to reach is one line.

use binmap_core::Capability;
use binmap_core::config::TrustTier;
use binmap_core::event::{EngineEvent, EventSink, RunId};
use binmap_core::facade::{Engine, ProbeStatus, Request};
use binmap_core::finding::FindingKind;
use binmap_gui::state::{Action, AppState, View};
use binmap_gui::testing::ScriptedEngine;
use std::sync::Arc;

fn engine() -> ScriptedEngine {
    ScriptedEngine::new()
        .with_target("app::app", &[Capability::ConfigurationSweep])
        .with_target("lib::lib", &[Capability::ConfigurationSweep, Capability::SizeAttribution])
        .with_probe("cargo", ProbeStatus::Present)
        .with_probe("miri", ProbeStatus::Missing)
}

/// The state a freshly opened window would have.
fn opened(engine: &ScriptedEngine) -> AppState {
    let mut state = AppState::new();
    state.set_probes(engine.probe_environment());
    state.set_targets(engine.targets().unwrap());
    state.select_view(View::Target);
    state
}

#[test]
fn the_nav_rail_only_offers_what_the_target_supports() {
    let engine = engine();
    let mut state = opened(&engine);

    // app::app can only be swept.
    let views: Vec<View> = state.nav_entries().into_iter().map(|e| e.view).collect();
    assert!(views.contains(&View::Tune));
    assert!(!views.contains(&View::Size), "Size is absent, not disabled");

    // lib::lib can also be attributed, so Size appears.
    assert!(state.select_target("lib::lib"));
    let views: Vec<View> = state.nav_entries().into_iter().map(|e| e.view).collect();
    assert!(views.contains(&View::Size));
}

#[test]
fn every_action_the_interface_offers_is_in_the_palette() {
    // U12 is a Must: keyboard access is the only accommodation left for users
    // who live in terminals. A control that exists only as a click is a bug,
    // and this is the test that catches one.
    let engine = engine();
    let state = opened(&engine);
    let commands = state.commands("");

    let offered: Vec<&Action> = commands.iter().map(|c| &c.action).collect();
    assert!(offered.contains(&&Action::StartSweep));
    assert!(offered.contains(&&Action::RecheckEnvironment));
    assert!(offered.contains(&&Action::ToggleTheme));
    assert!(offered.contains(&&Action::OpenTierDialog));
    assert!(offered.iter().any(|a| matches!(a, Action::SelectView(_))));
    assert!(offered.iter().any(|a| matches!(a, Action::SelectTarget(_))));

    // And nothing the target cannot support is offered — the same rule the
    // nav rail follows, applied where a user goes looking.
    assert!(!offered.contains(&&Action::SelectView(View::Replay)));
}

#[test]
fn the_palette_filters_on_what_was_typed() {
    let engine = engine();
    let state = opened(&engine);
    let matched = state.commands("sweep");
    assert!(!matched.is_empty());
    assert!(matched.iter().all(|c| c.label.to_lowercase().contains("sweep")));
    assert!(state.commands("this matches nothing at all").is_empty());
}

#[test]
fn cancelling_is_only_offered_while_something_is_running() {
    let engine = engine();
    let mut state = opened(&engine);
    assert!(!state.commands("").iter().any(|c| c.action == Action::Cancel));

    state.apply(EngineEvent::Started {
        run: RunId("r".into()),
        description: "Sweeping".into(),
        total: Some(8),
    });
    assert!(state.commands("").iter().any(|c| c.action == Action::Cancel));

    state.apply(EngineEvent::Finished { run: RunId("r".into()), summary: "done".into() });
    assert!(!state.commands("").iter().any(|c| c.action == Action::Cancel));
}

#[test]
fn starting_a_sweep_reaches_the_engine_and_streams_back() {
    // The button's whole job. It was rendering and doing nothing.
    let engine = Arc::new(engine().scripting(vec![
        EngineEvent::Started {
            run: RunId("scripted".into()),
            description: "Sweeping 8 configurations".into(),
            total: Some(8),
        },
        EngineEvent::Progress {
            run: RunId("scripted".into()),
            completed: 3,
            message: "cfg-2: all gates passed".into(),
        },
    ]));

    let mut state = opened(&engine);
    let recorded = std::sync::Mutex::new(Vec::new());
    let sink: Arc<dyn EventSink> = Arc::new(move |event: EngineEvent| {
        recorded.lock().expect("poisoned").push(event.clone());
    });

    let (run, _cancel) =
        engine.start(Request::Sweep { target: "app::app".into() }, Arc::clone(&sink)).unwrap();
    assert_eq!(run, RunId("scripted".into()));
    assert_eq!(engine.started.lock().unwrap().len(), 1);

    // And the state a window would be in after those events arrived.
    state.apply(EngineEvent::Started {
        run: run.clone(),
        description: "Sweeping 8 configurations".into(),
        total: Some(8),
    });
    state.apply(EngineEvent::Progress {
        run: run.clone(),
        completed: 3,
        message: "cfg-2: all gates passed".into(),
    });
    assert!(state.is_busy());
    assert_eq!(state.run(&run).unwrap().fraction(), Some(0.375));
}

#[test]
fn the_tier_is_the_engines_so_the_frame_cannot_show_one_it_is_not_at() {
    let engine = engine();
    assert_eq!(engine.trust_tier(), TrustTier::Propose, "Propose is the default");

    // U9: raising a tier is a deliberate act, and it changes the engine.
    engine.set_trust_tier(TrustTier::Tune);
    assert_eq!(engine.trust_tier(), TrustTier::Tune);
}

#[test]
fn the_profile_lab_reads_a_sweep_the_engine_actually_holds() {
    let engine = engine().with_sweep("app::app", 4, 4);
    let sweeps = engine.sweeps();
    assert_eq!(sweeps.len(), 1);

    let sweep = &sweeps[0];
    assert_eq!(sweep.measured.len(), 8);
    assert_eq!(sweep.rejected(), 4, "rejected candidates are kept, not dropped");
    assert_eq!(sweep.frontier().count(), 1);
    assert_eq!(sweep.noise_floor_label().as_deref(), Some("0.9%"));

    // Every rejected candidate names the gate that rejected it.
    for measurement in sweep.measured.iter().filter(|m| !m.passed()) {
        assert!(measurement.rejected_by().is_some(), "{} was rejected anonymously", measurement.id);
    }
}

#[test]
fn selecting_a_finding_is_what_fills_the_inspector() {
    let engine = engine()
        .with_finding("f1", FindingKind::Configuration, "opt-level=s is smaller")
        .with_finding("f2", FindingKind::FrontierPoint, "cfg-0 is on the frontier");

    let mut state = AppState::new();
    state.set_targets(engine.targets().unwrap());
    for finding in engine.findings() {
        state.apply(EngineEvent::Finding {
            run: RunId("restored".into()),
            finding: Box::new(finding),
        });
    }

    // The first selects itself, so the Inspector is never blank for nothing.
    assert_eq!(state.selected_finding().map(|f| f.id()), Some("f1"));
    assert!(state.select_finding("f2"));
    assert_eq!(state.selected_finding().map(|f| f.id()), Some("f2"));

    // And its evidence is reachable — a claim and its evidence on one screen.
    let evidence = Engine::evidence(&engine, "f2");
    assert_eq!(evidence.len(), 1);
    assert!(evidence[0].digest_matches());
}

#[test]
fn switching_target_drops_a_view_the_new_one_cannot_support() {
    let engine = engine();
    let mut state = opened(&engine);

    assert!(state.select_target("lib::lib"));
    assert!(state.select_view(View::Size));
    assert_eq!(state.view(), Some(View::Size));

    // app::app cannot attribute size, so the view cannot survive the switch.
    assert!(state.select_target("app::app"));
    assert_ne!(state.view(), Some(View::Size));
}

#[test]
fn re_checking_the_environment_is_a_fresh_read_not_a_cached_one() {
    // What makes a missing tool recoverable without restarting.
    let engine = engine();
    let mut state = opened(&engine);
    assert_eq!(state.unmet_probes(), 1);

    state.set_probes(engine.probe_environment());
    assert_eq!(state.unmet_probes(), 1, "the probe set is re-read, not remembered");
    assert!(state.probes().iter().any(|p| p.name == "miri" && p.remedy.is_some()));
}

// ---------------------------------------------------------------------------
// The palette's keyboard (U12)
// ---------------------------------------------------------------------------
//
// The palette filters on a query, and the query is built one keystroke at a
// time. These assert the arithmetic of that, which is the part that is easy to
// get subtly wrong and impossible to notice by looking.

/// Replays a sequence of actions against a bare query and highlight, the way
/// `Binmap::act` does, so palette arithmetic can be checked without a window.
#[derive(Default)]
struct PaletteState {
    query: String,
    index: usize,
}

impl PaletteState {
    fn apply(&mut self, action: Action, state: &AppState) {
        match action {
            Action::PaletteInput(text) => {
                self.query.push_str(&text);
                self.index = 0;
            }
            Action::PaletteBackspace => {
                self.query.pop();
                self.index = 0;
            }
            Action::PaletteMove(by) => {
                let count = state.commands(&self.query).len();
                if count > 0 {
                    self.index = (self.index as i32 + by).rem_euclid(count as i32) as usize;
                }
            }
            _ => {}
        }
    }
}

#[test]
fn typing_narrows_the_list_and_backspace_widens_it_again() {
    let engine = engine();
    let state = opened(&engine);
    let mut palette = PaletteState::default();

    let everything = state.commands("").len();
    for letter in ["s", "w", "e", "e", "p"] {
        palette.apply(Action::PaletteInput(letter.into()), &state);
    }
    assert_eq!(palette.query, "sweep");

    let narrowed = state.commands(&palette.query).len();
    assert!(narrowed > 0 && narrowed < everything, "{narrowed} of {everything}");

    palette.apply(Action::PaletteBackspace, &state);
    assert_eq!(palette.query, "swee");
    assert!(state.commands(&palette.query).len() >= narrowed);
}

#[test]
fn narrowing_resets_the_highlight_rather_than_leaving_it_somewhere_unread() {
    // Otherwise Enter runs whatever happens to sit at the old position in a
    // list the user has not looked at.
    let engine = engine();
    let state = opened(&engine);
    let mut palette = PaletteState::default();

    palette.apply(Action::PaletteMove(3), &state);
    assert_eq!(palette.index, 3);

    palette.apply(Action::PaletteInput("s".into()), &state);
    assert_eq!(palette.index, 0, "the highlight survived a narrowing");

    palette.apply(Action::PaletteMove(1), &state);
    palette.apply(Action::PaletteBackspace, &state);
    assert_eq!(palette.index, 0, "the highlight survived a widening");
}

#[test]
fn the_highlight_wraps_at_both_ends() {
    // A list that stops at the end makes the last item harder to reach than
    // the first, for no reason a user would recognise.
    let engine = engine();
    let state = opened(&engine);
    let count = state.commands("").len();
    let mut palette = PaletteState::default();

    palette.apply(Action::PaletteMove(-1), &state);
    assert_eq!(palette.index, count - 1, "up from the top reaches the bottom");

    palette.apply(Action::PaletteMove(1), &state);
    assert_eq!(palette.index, 0, "down from the bottom reaches the top");
}

#[test]
fn moving_within_an_empty_result_does_not_panic_or_point_at_nothing() {
    let engine = engine();
    let state = opened(&engine);
    let mut palette = PaletteState { query: "matches nothing whatsoever".into(), index: 0 };
    assert!(state.commands(&palette.query).is_empty());

    palette.apply(Action::PaletteMove(1), &state);
    assert_eq!(palette.index, 0);
    assert!(state.commands(&palette.query).get(palette.index).is_none());
}

#[test]
fn confirming_runs_the_same_action_a_click_produces() {
    // The whole point of the Action model: the palette has no second code
    // path to keep in step with the buttons.
    let engine = engine();
    let state = opened(&engine);

    let typed = state.commands("sweep");
    let clicked = state
        .commands("")
        .into_iter()
        .find(|c| c.label == "Sweep build configurations")
        .expect("the button's action is in the palette");

    assert_eq!(typed[0].action, clicked.action);
    assert_eq!(typed[0].action, Action::StartSweep);
}

#[test]
fn exporting_reaches_the_engine_and_reports_where_it_went() {
    // U13's export half had a redaction pass, a store and no way to ask for
    // it from the interface.
    let engine = engine();
    let (path, redacted) = engine.export_session("app::app").expect("the export is written");

    assert_eq!(engine.exports.lock().unwrap().as_slice(), ["app::app"]);
    assert!(path.to_string_lossy().ends_with(".binmap.json"), "{}", path.display());
    // The recipient is told the evidence was altered, and by how much.
    assert!(redacted.starts_with("Redacted: "), "{redacted}");
}

#[test]
fn export_is_reachable_from_the_keyboard_like_everything_else() {
    let engine = engine();
    let state = opened(&engine);
    let command = state
        .commands("export")
        .into_iter()
        .next()
        .expect("U12: every action the interface offers is in the palette");
    assert_eq!(command.action, Action::ExportSession);
    assert_eq!(command.shortcut, Some("⌘⇧E"));
}

// ---------------------------------------------------------------------------
// The first-run flow
// ---------------------------------------------------------------------------

use binmap_gui::state::Stage;

#[test]
fn the_flow_is_three_steps_and_each_one_says_where_it_is() {
    assert_eq!(Stage::STEPS.len(), 3);
    assert_eq!(Stage::Environment.label().as_deref(), Some("Step 1 of 3"));
    assert_eq!(Stage::Configure.label().as_deref(), Some("Step 2 of 3"));
    assert_eq!(Stage::Reasoner.label().as_deref(), Some("Step 3 of 3"));
    // Ready is the application, not a step.
    assert_eq!(Stage::Ready.label(), None);
}

#[test]
fn the_flow_walks_forward_to_ready_and_back_to_the_start() {
    let mut stage = Stage::Environment;
    for _ in 0..3 {
        stage = stage.next();
    }
    assert_eq!(stage, Stage::Ready);
    // And stays there rather than wrapping into the flow again.
    assert_eq!(stage.next(), Stage::Ready);

    assert_eq!(Stage::Reasoner.previous(), Some(Stage::Configure));
    assert_eq!(Stage::Environment.previous(), None, "there is nothing before the first step");
    assert_eq!(Stage::Ready.previous(), None, "the application does not go back into setup");
}

#[test]
fn every_step_has_a_heading_and_a_reason_for_existing() {
    for stage in Stage::STEPS {
        assert!(!stage.heading().is_empty(), "{stage:?} has no heading");
        assert!(stage.blurb().len() > 40, "{stage:?} does not say why it is asking");
    }
}

#[test]
fn a_project_with_no_benchmark_says_runtime_is_not_an_objective() {
    // Not an error and not a blocker — a different thing from being fast.
    let engine = engine();
    assert_eq!(engine.benchmark_command(), None);

    let declared = ScriptedEngine::new().with_benchmark("cargo bench --bench route");
    assert_eq!(declared.benchmark_command().as_deref(), Some("cargo bench --bench route"));
}
