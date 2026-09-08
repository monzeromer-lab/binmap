//! The application frame: title bar, nav rail, status bar.
//!
//! Fixed dimensions, taken from the design's spacing tokens rather than
//! chosen per view — 40px title bar, 72px nav rail, 28px status bar, 340px
//! Inspector. These are the frame; the views live inside it.

use crate::state::{AppState, NavEntry, RunPhase, View};
use crate::theme::{Theme, radius, space, type_scale};
use crate::widgets::{Badge, Tone};
use binmap_core::config::TrustTier;
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, App, SharedString, Window, div, px, relative};

/// The title bar: what is open, at what commit, and the controls that apply to
/// the whole session.
///
/// The dirty flag is not decoration. A number measured on an uncommitted tree
/// cannot be reproduced by anyone else, and the frame says so permanently
/// rather than in a notice that scrolls away.
#[derive(IntoElement)]
pub struct TitleBar {
    project: SharedString,
    commit: Option<SharedString>,
    dirty: bool,
    tier: TrustTier,
    theme: Theme,
    trailing: Vec<AnyElement>,
}

impl TitleBar {
    pub fn new(project: impl Into<SharedString>, tier: TrustTier, theme: Theme) -> Self {
        Self {
            project: project.into(),
            commit: None,
            dirty: false,
            tier,
            theme,
            trailing: Vec::new(),
        }
    }

    pub fn at_commit(mut self, commit: impl Into<SharedString>, dirty: bool) -> Self {
        self.commit = Some(commit.into());
        self.dirty = dirty;
        self
    }

    pub fn with_trailing(mut self, element: impl IntoElement) -> Self {
        self.trailing.push(element.into_any_element());
        self
    }
}

impl RenderOnce for TitleBar {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let c = self.theme.colours;
        let tier_colour = c.tier(self.tier);

        div()
            .flex()
            .flex_row()
            .items_center()
            .flex_none()
            .h(space::TITLEBAR_H)
            .px(space::S12)
            .gap(space::S8)
            .bg(c.surface_panel)
            .border_b_1()
            .border_color(c.border_subtle)
            // The mark: the brand chevron, small.
            .child(div().flex_none().text_color(c.accent).text_size(type_scale::FS_13).child("◆"))
            .child(
                div()
                    .flex_none()
                    .text_color(c.text_primary)
                    .text_size(type_scale::FS_13)
                    .child(self.project),
            )
            .when_some(self.commit, |d, commit| {
                d.child(
                    div()
                        .flex_none()
                        .font_family("JetBrains Mono")
                        .text_size(type_scale::FS_11)
                        .text_color(c.text_muted)
                        .child(commit),
                )
            })
            .when(self.dirty, |d| {
                d.child(Badge::new("dirty", Tone::Warn, self.theme).caps().mono())
            })
            .child(div().flex_1())
            // The tier is always visible and never raised silently (U9).
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(space::S4)
                    .h(space::CONTROL_H_SM)
                    .px(space::S8)
                    .rounded(radius::CONTROL)
                    .border_1()
                    .border_color(c.border_default)
                    .text_size(type_scale::FS_11)
                    .text_color(c.text_secondary)
                    .child("Trust:")
                    .child(div().text_color(tier_colour).child(self.tier.label())),
            )
            .children(self.trailing)
    }
}

/// The nav rail: which view is showing, and how many findings each holds.
///
/// Entries are supplied, not derived here — a view the target cannot support
/// is absent from the list before it reaches this widget (§2.5). That is the
/// whole reason this takes `Vec<NavEntry>` rather than the target.
#[derive(IntoElement)]
pub struct NavRail {
    entries: Vec<NavEntry>,
    selected: Option<View>,
    theme: Theme,
}

impl NavRail {
    pub fn new(entries: Vec<NavEntry>, selected: Option<View>, theme: Theme) -> Self {
        Self { entries, selected, theme }
    }
}

/// The rail's glyph for a view. Stand-ins for the design's Lucide icons until
/// an icon set is wired; the shapes carry the same distinctions.
fn glyph(view: View) -> &'static str {
    match view {
        View::Target => "◉",
        View::Size => "▦",
        View::Tune => "⚙",
        View::Failure => "◈",
        View::Perf => "◭",
        View::Load => "◷",
        View::Replay => "⟲",
        View::Agent => "◇",
        View::Environment => "⌁",
    }
}

impl RenderOnce for NavRail {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let c = self.theme.colours;
        let selected = self.selected;
        let theme = self.theme;

        div()
            .flex()
            .flex_col()
            .flex_none()
            .w(space::NAVRAIL_W)
            .h_full()
            .bg(c.surface_app)
            .border_r_1()
            .border_color(c.border_subtle)
            .children(self.entries.into_iter().map(move |entry| {
                let active = selected == Some(entry.view);
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(space::S2)
                    .flex_none()
                    .w_full()
                    .h(px(52.))
                    .when(active, |d| d.bg(c.surface_selected).border_l_2().border_color(c.accent))
                    .child(
                        div()
                            .text_size(type_scale::FS_14)
                            .text_color(if active { c.accent } else { c.text_muted })
                            .child(glyph(entry.view)),
                    )
                    .child(
                        div()
                            .text_size(type_scale::FS_11)
                            .text_color(if active { c.text_primary } else { c.text_muted })
                            .child(entry.view.label()),
                    )
                    .when(entry.findings > 0, |d| {
                        d.child(
                            Badge::new(
                                entry.findings.to_string(),
                                if active { Tone::Accent } else { Tone::Neutral },
                                theme,
                            )
                            .mono(),
                        )
                    })
            }))
    }
}

/// The status bar: what the engine is doing, and what the session costs.
///
/// A run in flight shows its progress here and nowhere else, so the views
/// never have to reserve space for a bar that is usually absent.
#[derive(IntoElement)]
pub struct StatusBar {
    message: SharedString,
    /// `Some((completed, total))` where the total is known.
    progress: Option<(usize, usize)>,
    running: bool,
    findings: usize,
    theme: Theme,
}

impl StatusBar {
    pub fn of(state: &AppState, theme: Theme) -> Self {
        // The most recent run that is still going, else the last thing that
        // happened, else the idle line.
        let run = state.runs().find(|run| run.is_running()).or_else(|| state.runs().last());

        let (message, progress, running) = match run {
            Some(run) => {
                let message: SharedString = match &run.state {
                    RunPhase::Running => {
                        if run.message.is_empty() {
                            run.description.clone().into()
                        } else {
                            run.message.clone().into()
                        }
                    }
                    RunPhase::Finished { summary } => summary.clone().into(),
                    RunPhase::Cancelled { completed } => {
                        format!("Cancelled after {completed}; everything measured was kept").into()
                    }
                    RunPhase::Failed { error } => error.clone().into(),
                };
                (message, run.total.map(|total| (run.completed, total)), run.is_running())
            }
            None => ("Ready".into(), None, false),
        };

        Self { message, progress, running, findings: state.findings().len(), theme }
    }
}

impl RenderOnce for StatusBar {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let c = self.theme.colours;

        div()
            .flex()
            .flex_row()
            .items_center()
            .flex_none()
            .h(space::STATUSBAR_H)
            .px(space::S12)
            .gap(space::S8)
            .bg(c.surface_panel)
            .border_t_1()
            .border_color(c.border_subtle)
            .text_size(type_scale::FS_11)
            .child(
                div()
                    .flex_none()
                    .text_color(if self.running { c.status_running } else { c.text_muted })
                    .child(if self.running { "◐" } else { "○" }),
            )
            .child(div().flex_none().text_color(c.text_secondary).child(self.message))
            .when_some(self.progress, |d, (completed, total)| {
                let fraction = if total == 0 { 0.0 } else { completed as f32 / total as f32 };
                d.child(
                    div()
                        .flex_none()
                        .w(px(120.))
                        .h(px(4.))
                        .rounded(radius::CHIP)
                        .bg(c.surface_active)
                        .child(
                            div()
                                .h_full()
                                .w(relative(fraction.clamp(0.0, 1.0)))
                                .rounded(radius::CHIP)
                                .bg(c.accent),
                        ),
                )
                .child(
                    div()
                        .flex_none()
                        .font_family("JetBrains Mono")
                        .text_color(c.text_muted)
                        .child(format!("{completed}/{total}")),
                )
            })
            .child(div().flex_1())
            .child(div().flex_none().text_color(c.text_muted).child(format!(
                "{} finding{}",
                self.findings,
                if self.findings == 1 { "" } else { "s" }
            )))
            .child(
                div()
                    .flex_none()
                    .font_family("JetBrains Mono")
                    .text_color(c.text_disabled)
                    .child("⌘K"),
            )
    }
}
