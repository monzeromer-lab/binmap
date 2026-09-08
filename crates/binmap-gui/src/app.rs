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

use crate::state::{AppState, View};
use crate::theme::{Appearance, Theme, space};
use crate::views::chrome::{NavRail, StatusBar, TitleBar};
use crate::views::environment::EnvironmentPanel;
use crate::views::inspector::{Inspector, Tab};
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

        // Drain the channel on the foreground. Every applied event notifies,
        // which is the repaint request.
        cx.spawn(async move |view, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(50))
                    .await;
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
        }
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
}

impl Render for Binmap {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let c = theme.colours;

        let commit = self
            .state
            .probes()
            .iter()
            .find(|probe| probe.name == "git")
            .map(|probe| probe.detail.clone());
        let dirty = commit.as_deref().is_some_and(|d| d.contains("uncommitted"));
        let short = commit
            .as_deref()
            .and_then(|d| d.split(',').next())
            .unwrap_or_default()
            .to_string();

        let selected = self.state.selected_finding().cloned();
        let evidence = selected
            .as_ref()
            .map(|finding| self.engine.evidence(&finding.id))
            .unwrap_or_default();

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(c.surface_app)
            .text_color(c.text_body)
            .font_family("IBM Plex Sans")
            .child(
                TitleBar::new(self.project.clone(), self.engine_tier(), theme)
                    .at_commit(short, dirty),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h_0()
                    .child(NavRail::new(
                        self.state.nav_entries(),
                        self.state.view(),
                        theme,
                    ))
                    .child(TargetList::of(&self.state, theme))
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .min_w_0()
                            .bg(c.surface_app)
                            .child(match self.state.view() {
                                Some(View::Environment) => {
                                    EnvironmentPanel::of(&self.state, theme).into_any_element()
                                }
                                _ => TargetView::of(&self.state, self.root.clone(), theme)
                                    .into_any_element(),
                            }),
                    )
                    .child(Inspector::new(
                        selected,
                        evidence,
                        self.tab,
                        self.state.findings(),
                        theme,
                    )),
            )
            .child(StatusBar::of(&self.state, theme))
    }
}

impl Binmap {
    /// The tier the engine is at.
    ///
    /// Read through the facade rather than held here, so the title bar cannot
    /// show a tier the engine is not actually running at.
    fn engine_tier(&self) -> binmap_core::config::TrustTier {
        // The facade does not expose the tier yet; until it does, the frame
        // shows the default rather than inventing one.
        binmap_core::config::TrustTier::default()
    }
}

/// Open the window.
pub fn run(engine: Arc<dyn Engine>, project: String, root: Option<String>) {
    gpui_kit::application().run(move |cx: &mut App| {
        gpui_kit::init(cx);

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
                let app: Entity<Binmap> =
                    cx.new(|cx| Binmap::new(engine, project, root, cx));
                cx.new(|cx| gpui_kit::component::Root::new(gpui_kit::AnyView::from(app), window, cx))
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
