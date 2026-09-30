//! The root view: the frame, and the wiring between engine events and state.
//!
//! This is the one place that holds an `Arc<dyn Engine>`, and it holds it as a
//! trait object — the interface never names a concrete engine. Every long run
//! goes to the background executor and reports through an
//! [`EventSink`](binmap_core::event::EventSink); the foreground applies the
//! events to [`AppState`] and calls `cx.notify()`.
//!
//! Forgetting that notify is the most common GPUI bug (DESIGN-GUI §4), so
//! there is exactly one place events are applied and it always notifies.

use crate::dispatch::Dispatch;
use crate::state::{Action, AppState, Stage, View};
use crate::theme::{Appearance, Theme, space};
use crate::views::chrome::{NavRail, StatusBar, TitleBar};
use crate::views::environment::EnvironmentPanel;
use crate::views::inspector::{Inspector, Tab};
use crate::views::profile_lab::ProfileLab;
use crate::views::targets::{TargetList, TargetView};
use binmap_core::event::{Cancellation, EngineEvent, EventSink, RunId};
use binmap_core::facade::{Engine, Request};
use gpui_kit::prelude::*;
use gpui_kit::{App, Context, Entity, Window, div};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

/// An [`EventSink`] that forwards to the foreground.
///
/// A plain channel rather than anything framework-specific, so the engine
/// crates never have to agree with the interface about an async runtime. The
/// view drains it on a timer and applies whatever arrived.
struct ChannelSink(Sender<EngineEvent>);

impl EventSink for ChannelSink {
    fn emit(&self, event: EngineEvent) {
        // A closed channel means the window has gone. The run keeps going and
        // keeps recording; there is simply nobody watching.
        let _ = self.0.send(event);
    }
}

// The keyboard map. `U12` is a Must rather than a Should because keyboard
// access is the only accommodation left for users who live in terminals, and
// that is most of the audience.
gpui_kit::actions!(
    binmap,
    [
        /// Write the session out, redacted.
        Export,
        /// Open or close the command palette.
        TogglePalette,
        /// Dismiss whatever is open; if nothing is, cancel the run.
        Dismiss,
        /// Sweep build configurations.
        Sweep,
        /// Attribute the artifact's bytes.
        AttributeSize,
        /// Switch between the dark and light themes.
        ToggleTheme,
    ]
);

/// Bind the keys. Called once, when the application starts.
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        gpui_kit::KeyBinding::new("cmd-k", TogglePalette, None),
        gpui_kit::KeyBinding::new("ctrl-k", TogglePalette, None),
        gpui_kit::KeyBinding::new("escape", Dismiss, None),
        gpui_kit::KeyBinding::new("cmd-shift-t", Sweep, None),
        gpui_kit::KeyBinding::new("ctrl-shift-t", Sweep, None),
        gpui_kit::KeyBinding::new("cmd-shift-l", ToggleTheme, None),
        gpui_kit::KeyBinding::new("ctrl-shift-l", ToggleTheme, None),
        gpui_kit::KeyBinding::new("cmd-shift-s", AttributeSize, None),
        gpui_kit::KeyBinding::new("ctrl-shift-s", AttributeSize, None),
        gpui_kit::KeyBinding::new("cmd-shift-e", Export, None),
        gpui_kit::KeyBinding::new("ctrl-shift-e", Export, None),
    ]);
}

/// The application.
pub struct Binmap {
    engine: Arc<dyn Engine>,
    state: AppState,
    theme: Theme,
    tab: Tab,
    project: String,
    root: Option<String>,
    events: Receiver<EngineEvent>,
    sender: Sender<EngineEvent>,
    /// The run in flight, so it can be cancelled. A sweep the user cannot stop
    /// is a hostile tool.
    active: Option<(RunId, Cancellation)>,
    /// `U12`: open, what has been typed into it, and which row is highlighted.
    palette: bool,
    query: String,
    palette_index: usize,
    /// `U9`: the tier dialog, which is how a tier is raised.
    tier_dialog: bool,
    /// `U0.2`: the configuration the apply dialog is offering to write.
    apply_dialog: Option<String>,
    /// Which row of the Profile Lab is open. `None` selects the frontier's
    /// first point, so the panel is never empty when there is something to
    /// show.
    selected_configuration: Option<String>,
    /// How many runs a previous session left behind.
    restored: usize,
    /// Where the first-run flow has got to.
    stage: Stage,
    /// The window's focus, so key bindings reach the frame.
    focus: gpui_kit::FocusHandle,
}

impl Binmap {
    pub fn new(
        engine: Arc<dyn Engine>,
        project: impl Into<String>,
        root: Option<String>,
        cx: &mut Context<Self>,
    ) -> Self {
        let (sender, events) = channel();

        let mut state = AppState::new();
        state.set_probes(engine.probe_environment());
        if let Ok(targets) = engine.targets() {
            state.set_targets(targets);
        }
        state.select_view(View::Target);

        // Whatever the last session on this target measured. A tool that
        // forgets an hour-long sweep because a window closed is a tool people
        // run twice.
        let restored = match state.selected_target().map(|target| target.id.clone()) {
            Some(id) => engine.restore_session(&id),
            None => 0,
        };
        for finding in engine.findings() {
            state.apply(EngineEvent::Finding {
                run: RunId("restored".into()),
                finding: Box::new(finding),
            });
        }
        // Land where the restored work is. Someone reopening a project after a
        // sweep wants the sweep, not the target list they already chose from.
        //
        // A restored session also means the flow has been through once
        // already: being walked through setup again is a tool that does not
        // remember you.
        // Land where the most recent work is. A sweep is the bigger
        // investment, so it wins where both exist.
        let stage = if restored > 0 {
            if !engine.sweeps().is_empty() {
                state.select_view(View::Tune);
            } else if engine.attribution().is_some() {
                state.select_view(View::Size);
            }
            Stage::Ready
        } else {
            Stage::Environment
        };

        // Drain the channel on the foreground. Every applied event notifies,
        // which is the repaint request.
        cx.spawn(async move |view, cx| {
            loop {
                cx.background_executor().timer(std::time::Duration::from_millis(50)).await;
                let updated = view.update(cx, |this: &mut Binmap, cx| {
                    let mut applied = false;
                    while let Ok(event) = this.events.try_recv() {
                        if event.is_terminal() {
                            this.active = None;
                        }
                        this.state.apply(event);
                        applied = true;
                    }
                    if applied {
                        cx.notify();
                    }
                });
                if updated.is_err() {
                    return;
                }
            }
        })
        .detach();

        Self {
            engine,
            state,
            theme: Theme::default(),
            tab: Tab::default(),
            project: project.into(),
            root,
            events,
            sender,
            active: None,
            selected_configuration: None,
            restored,
            stage,
            palette: false,
            query: String::new(),
            palette_index: 0,
            tier_dialog: false,
            apply_dialog: None,
            focus: cx.focus_handle(),
        }
    }

    /// How many runs a previous session left behind.
    pub fn restored_runs(&self) -> usize {
        self.restored
    }

    pub fn state(&self) -> &AppState {
        &self.state
    }

    /// U11: both themes, switched at runtime.
    pub fn toggle_theme(&mut self, cx: &mut Context<Self>) {
        self.theme = self.theme.toggled();
        cx.notify();
    }

    pub fn appearance(&self) -> Appearance {
        self.theme.appearance
    }

    /// Start a sweep over the selected target.
    ///
    /// Returns immediately: `Engine::start` registers the run and everything
    /// after that arrives as an event.
    pub fn sweep(&mut self, cx: &mut Context<Self>) {
        let Some(target) = self.state.selected_target().map(|t| t.id.clone()) else {
            return;
        };
        let sink: Arc<dyn EventSink> = Arc::new(ChannelSink(self.sender.clone()));
        match self.engine.start(Request::Sweep { target }, sink) {
            Ok(run) => self.active = Some(run),
            Err(error) => {
                // A refusal is a fact worth showing, not a silent no-op.
                self.state.apply(EngineEvent::Failed {
                    run: RunId("rejected".into()),
                    error: error.to_string(),
                });
            }
        }
        cx.notify();
    }

    /// Attribute the selected target's bytes (`F1.2`-`F1.4`).
    pub fn attribute_size(&mut self, cx: &mut Context<Self>) {
        let Some(target) = self.state.selected_target().map(|t| t.id.clone()) else {
            return;
        };
        let sink: Arc<dyn EventSink> = Arc::new(ChannelSink(self.sender.clone()));
        match self.engine.start(Request::AttributeSize { target }, sink) {
            Ok(run) => self.active = Some(run),
            Err(error) => {
                // A target that cannot be attributed says so rather than
                // silently doing nothing.
                self.state.apply(EngineEvent::Failed {
                    run: RunId("size".into()),
                    error: error.to_string(),
                });
            }
        }
        cx.notify();
    }

    /// Cancel the run in flight, keeping everything it has measured.
    pub fn cancel(&mut self, cx: &mut Context<Self>) {
        if let Some((_, cancellation)) = &self.active {
            cancellation.cancel();
        }
        cx.notify();
    }

    pub fn is_running(&self) -> bool {
        self.active.is_some()
    }

    pub fn select_view(&mut self, view: View, cx: &mut Context<Self>) {
        if self.state.select_view(view) {
            cx.notify();
        }
    }

    /// Open one configuration in the Profile Lab's selected panel.
    pub fn select_configuration(&mut self, id: impl Into<String>, cx: &mut Context<Self>) {
        self.selected_configuration = Some(id.into());
        cx.notify();
    }

    pub fn select_tab(&mut self, tab: Tab, cx: &mut Context<Self>) {
        self.tab = tab;
        cx.notify();
    }

    /// Re-run every probe. This is what makes a missing tool recoverable
    /// without restarting the application.
    pub fn recheck_environment(&mut self, cx: &mut Context<Self>) {
        self.state.set_probes(self.engine.probe_environment());
        cx.notify();
    }

    /// Everything the interface can ask for, handled in one place.
    ///
    /// A click and a key binding produce the same `Action`, and the command
    /// palette is a list of them — which is what makes `U12`'s "keyboard
    /// reaches every action" a property of the design rather than a promise to
    /// keep re-checking.
    pub fn act(&mut self, action: Action, cx: &mut Context<Self>) {
        match action {
            Action::SelectView(view) => {
                self.state.select_view(view);
            }
            Action::SelectTarget(id) => {
                if self.state.select_target(&id) {
                    // A different target's sweep is a different sweep.
                    self.selected_configuration = None;
                }
            }
            Action::SelectConfiguration(id) => self.selected_configuration = Some(id),
            Action::SelectFinding(id) => {
                self.state.select_finding(&id);
            }
            Action::SelectTab(tab) => self.tab = tab,
            Action::StartSweep => return self.sweep(cx),
            Action::AttributeSize => return self.attribute_size(cx),
            Action::Cancel => return self.cancel(cx),
            Action::ToggleTheme => self.theme = self.theme.toggled(),
            Action::RecheckEnvironment => return self.recheck_environment(cx),
            Action::TogglePalette => {
                self.palette = !self.palette;
                self.query.clear();
                self.palette_index = 0;
            }
            Action::ClosePalette => self.palette = false,
            Action::PaletteInput(text) => {
                self.query.push_str(&text);
                // A narrowed list invalidates the old position, and leaving
                // the highlight where it was selects something the user never
                // looked at.
                self.palette_index = 0;
            }
            Action::PaletteBackspace => {
                self.query.pop();
                self.palette_index = 0;
            }
            Action::PaletteMove(by) => {
                let count = self.state.commands(&self.query).len();
                if count > 0 {
                    // Wrapping, because a list that stops at the end makes the
                    // last item harder to reach than the first.
                    let position = self.palette_index as i32 + by;
                    self.palette_index = position.rem_euclid(count as i32) as usize;
                }
            }
            Action::PaletteConfirm => {
                let commands = self.state.commands(&self.query);
                let Some(command) = commands.get(self.palette_index).cloned() else {
                    return;
                };
                self.palette = false;
                // The palette runs the same action a click produces. There is
                // no second code path to keep in step.
                return self.act(command.action, cx);
            }
            Action::ExportSession => {
                let Some(target) = self.state.selected_target().map(|t| t.id.clone()) else {
                    return;
                };
                // Reported through the run log, so the path and what was
                // redacted land where every other outcome does rather than in
                // a toast that scrolls away.
                let run = RunId("export".into());
                match self.engine.export_session(&target) {
                    Ok((path, redacted)) => self.state.apply(EngineEvent::Finished {
                        run,
                        summary: format!("Exported to {} — {redacted}", path.display()),
                    }),
                    Err(error) => self.state.apply(EngineEvent::Failed {
                        run,
                        error: format!("the session could not be exported: {error}"),
                    }),
                }
            }
            Action::FlowNext => self.stage = self.stage.next(),
            Action::FlowBack => {
                if let Some(previous) = self.stage.previous() {
                    self.stage = previous;
                }
            }
            // Every step is skippable. A first-run flow that gates the product
            // on answering it is a tool people close.
            Action::FlowSkip => self.stage = Stage::Ready,
            Action::OpenApplyDialog(id) => self.apply_dialog = Some(id),
            Action::ApplyConfiguration(id) => {
                self.apply_dialog = None;
                let run = RunId(format!("apply-{id}"));
                // Make the proposal, then ask for it to be written. The engine
                // refuses below Tune, and refuses a configuration its gates
                // rejected — the interface does not decide either.
                let outcome = self
                    .sweep_configuration(&id)
                    .ok_or_else(|| {
                        binmap_core::Error::Other(format!(
                            "`{id}` was not measured in this session"
                        ))
                    })
                    .and_then(|configuration| self.engine.propose_configuration(&configuration))
                    .and_then(|proposal| {
                        let sink: Arc<dyn EventSink> = Arc::new(ChannelSink(self.sender.clone()));
                        self.engine.start(Request::Apply { proposal: proposal.id }, sink)
                    });
                if let Err(error) = outcome {
                    self.state.apply(EngineEvent::Failed { run, error: error.to_string() });
                }
            }
            Action::OpenTierDialog => self.tier_dialog = true,
            Action::SetTier(tier) => {
                // U9: raising a tier is always deliberate, and it is the
                // engine's state, not the frame's.
                self.engine.set_trust_tier(tier);
                self.tier_dialog = false;
            }
            Action::CloseDialogs => {
                self.tier_dialog = false;
                self.apply_dialog = None;
                self.palette = false;
            }
        }
        cx.notify();
    }

    /// The configuration one of this session's measured rows was built under.
    fn sweep_configuration(
        &self,
        id: &str,
    ) -> Option<binmap_core::configuration::BuildConfiguration> {
        let measurement =
            self.engine.sweeps().into_iter().flat_map(|s| s.measured).find(|m| m.id == id)?;
        // Rebuilt from the axes the measurement recorded, so the dialog and
        // the write describe the same thing the sweep measured.
        let mut configuration = binmap_core::configuration::BuildConfiguration::default();
        configuration.apply_settings(&measurement.settings);
        Some(configuration)
    }

    /// The dispatcher every view is handed.
    fn dispatcher(&self, cx: &mut Context<Self>) -> Dispatch {
        let this = cx.entity().downgrade();
        std::rc::Rc::new(move |action: Action, _window: &mut Window, cx: &mut App| {
            let _ = this.update(cx, |app: &mut Binmap, cx| app.act(action, cx));
        })
    }

    pub fn palette_is_open(&self) -> bool {
        self.palette
    }
}

impl Render for Binmap {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // The frame and the flow are different shapes, so both are erased.
        let theme = self.theme;
        let dispatch = self.dispatcher(cx);
        let c = theme.colours;

        let commit = self
            .state
            .probes()
            .iter()
            .find(|probe| probe.name == "git")
            .map(|probe| probe.detail.clone());
        let dirty = commit.as_deref().is_some_and(|d| d.contains("uncommitted"));
        let short =
            commit.as_deref().and_then(|d| d.split(',').next()).unwrap_or_default().to_string();

        let selected = self.state.selected_finding().cloned();
        let evidence =
            selected.as_ref().map(|finding| self.engine.evidence(finding.id())).unwrap_or_default();

        if self.stage != Stage::Ready {
            let flow = crate::views::flow::FirstRun::new(
                self.stage,
                &self.state,
                self.project.clone(),
                self.engine.benchmark_command(),
                theme,
                &dispatch,
            );
            return div()
                .id("binmap-flow")
                .size_full()
                .bg(c.surface_app)
                .text_color(c.text_body)
                .font_family("IBM Plex Sans")
                .child(flow)
                .into_any_element();
        }

        div()
            .id("binmap")
            .key_context("Binmap")
            .track_focus(&self.focus)
            .on_action(
                cx.listener(|this, _: &TogglePalette, _, cx| this.act(Action::TogglePalette, cx)),
            )
            .on_action(cx.listener(|this, _: &Sweep, _, cx| this.act(Action::StartSweep, cx)))
            .on_action(cx.listener(|this, _: &Export, _, cx| this.act(Action::ExportSession, cx)))
            .on_action(
                cx.listener(|this, _: &AttributeSize, _, cx| this.act(Action::AttributeSize, cx)),
            )
            .on_action(
                cx.listener(|this, _: &ToggleTheme, _, cx| this.act(Action::ToggleTheme, cx)),
            )
            // Escape dismisses what is open; with nothing open it cancels the
            // run, because that is what Escape means to someone watching a
            // sweep they want to stop.
            // The palette's keyboard. GPUI routes named actions, but a
            // palette needs the characters themselves, so it reads key events
            // directly — and only while it is open, so nothing else in the
            // frame loses its keys to it.
            .on_key_down(cx.listener(|this: &mut Binmap, event: &gpui_kit::KeyDownEvent, _, cx| {
                if !this.palette {
                    return;
                }
                let keystroke = &event.keystroke;
                let action = match keystroke.key.as_str() {
                    "backspace" => Some(Action::PaletteBackspace),
                    "down" => Some(Action::PaletteMove(1)),
                    "up" => Some(Action::PaletteMove(-1)),
                    "enter" => Some(Action::PaletteConfirm),
                    _ => keystroke
                        .key_char
                        .as_ref()
                        // A modifier chord is a command, not text. Without
                        // this, ⌘K would type "k" into the box it just opened.
                        .filter(|_| {
                            !keystroke.modifiers.control
                                && !keystroke.modifiers.platform
                                && !keystroke.modifiers.alt
                        })
                        .filter(|text| !text.chars().any(char::is_control))
                        .map(|text| Action::PaletteInput(text.clone())),
                };
                if let Some(action) = action {
                    cx.stop_propagation();
                    this.act(action, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &Dismiss, _, cx| {
                let action = if this.palette || this.tier_dialog || this.apply_dialog.is_some() {
                    Action::CloseDialogs
                } else {
                    Action::Cancel
                };
                this.act(action, cx);
            }))
            .flex()
            .flex_col()
            .size_full()
            .bg(c.surface_app)
            .text_color(c.text_body)
            .font_family("IBM Plex Sans")
            .child(
                TitleBar::new(self.project.clone(), self.engine_tier(), theme)
                    .at_commit(short, dirty)
                    .dispatching(&dispatch),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h_0()
                    .child(NavRail::new(self.state.nav_entries(), self.state.view(), theme))
                    .child(TargetList::of(&self.state, theme))
                    .child(
                        div().flex().flex_1().min_w_0().bg(c.surface_app).child(
                            match self.state.view() {
                                Some(View::Environment) => EnvironmentPanel::of(&self.state, theme)
                                    .dispatching(&dispatch)
                                    .into_any_element(),
                                Some(View::Size) => crate::views::size::SizeExplorer::new(
                                    self.engine.attribution(),
                                    theme,
                                )
                                .dispatching(&dispatch)
                                .into_any_element(),
                                Some(View::Tune) => ProfileLab::new(
                                    // The most recent sweep for the selected
                                    // target. The engine derives the frontier;
                                    // this only draws what it is told.
                                    self.engine.sweeps().into_iter().rfind(|sweep| {
                                        self.state
                                            .selected_target()
                                            .is_none_or(|t| sweep.target == t.id)
                                    }),
                                    self.selected_configuration.clone(),
                                    theme,
                                )
                                .dispatching(&dispatch)
                                .into_any_element(),
                                _ => TargetView::of(&self.state, self.root.clone(), theme)
                                    .dispatching(&dispatch)
                                    .into_any_element(),
                            },
                        ),
                    )
                    .child(
                        Inspector::new(selected, evidence, self.tab, self.state.findings(), theme)
                            .dispatching(&dispatch),
                    ),
            )
            .child(StatusBar::of(&self.state, theme).dispatching(&dispatch))
            // U12, and U9's dialog. Overlays last so they sit above the frame.
            .when(self.palette, |d| {
                d.child(
                    crate::views::palette::Palette::new(
                        self.state.commands(&self.query),
                        self.query.clone(),
                        theme,
                        &dispatch,
                    )
                    .highlighting(self.palette_index),
                )
            })
            .when_some(self.apply_dialog.clone(), |d, id| {
                let measurement =
                    self.engine.sweeps().into_iter().flat_map(|s| s.measured).find(|m| m.id == id);
                let Some(measurement) = measurement else { return d };
                let gates = measurement
                    .gates
                    .outcomes
                    .iter()
                    .map(|outcome| {
                        (
                            outcome.qualified_label(),
                            outcome.detail.clone(),
                            outcome.result == binmap_core::gate::GateResult::Passed,
                        )
                    })
                    .collect();
                d.child(crate::views::palette::ApplyDialog::new(
                    id,
                    measurement.flags.clone(),
                    self.root
                        .as_ref()
                        .map(|root| format!("{root}/Cargo.toml  ·  [profile.release]"))
                        .unwrap_or_else(|| "Cargo.toml  ·  [profile.release]".into()),
                    gates,
                    self.engine_tier(),
                    theme,
                    &dispatch,
                ))
            })
            .when(self.tier_dialog, |d| {
                d.child(crate::views::palette::TierDialog::new(
                    self.engine_tier(),
                    theme,
                    &dispatch,
                ))
            })
            .into_any_element()
    }
}

impl Binmap {
    /// The tier the engine is at.
    ///
    /// Read through the facade rather than held here, so the title bar cannot
    /// show a tier the engine is not actually running at.
    fn engine_tier(&self) -> binmap_core::config::TrustTier {
        self.engine.trust_tier()
    }
}

/// Open the window.
pub fn run(engine: Arc<dyn Engine>, project: String, root: Option<String>) {
    gpui_kit::application().run(move |cx: &mut App| {
        gpui_kit::init(cx);
        bind_keys(cx);

        cx.spawn(async move |cx| {
            let bounds = cx.update(|cx| {
                gpui_kit::Bounds::centered(
                    None,
                    gpui_kit::size(gpui_kit::px(1440.), gpui_kit::px(900.)),
                    cx,
                )
            });
            let options = gpui_kit::WindowOptions {
                window_bounds: Some(gpui_kit::WindowBounds::Windowed(bounds)),
                ..Default::default()
            };

            cx.open_window(options, |window, cx| {
                let app: Entity<Binmap> = cx.new(|cx| Binmap::new(engine, project, root, cx));
                cx.new(|cx| {
                    gpui_kit::component::Root::new(gpui_kit::AnyView::from(app), window, cx)
                })
            })
            .expect("Binmap could not open a window");
        })
        .detach();
    });
}

/// The frame's fixed dimensions, exposed so a test can assert the layout adds
/// up without opening a window.
pub const FRAME: (gpui_kit::Pixels, gpui_kit::Pixels, gpui_kit::Pixels) =
    (space::NAVRAIL_W, space::INSPECTOR_W, space::TITLEBAR_H);
