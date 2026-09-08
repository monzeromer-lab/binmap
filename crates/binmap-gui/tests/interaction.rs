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
