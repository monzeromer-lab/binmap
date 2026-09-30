//! The Agent panel (`U1.3`, `U6`, `DESIGN-AI §9`).
//!
//! "The primary trust surface in a GUI-only product", which decides what this
//! panel has to show: not just what a model said, but how it was reached and
//! what was refused. A panel that showed only conclusions would be a panel that
//! made a model look more reliable than it is.
//!
//! Four things, in the order the design lists them:
//!
//! - The **reasoner picker** (`§9.1`), spanning both modes, because from the
//!   user's point of view they are choosing who is going to think about this.
//!   "None" is listed beside the others rather than hidden in settings.
//! - The **transcript** (`§9.2`), one card per entry, rendering an event enum
//!   shaped like ACP's so Phase 2's external agents reuse this view.
//! - The **cost meter** (`§9.3`), which says "free" for a local model rather
//!   than "$0.00", because those mean different things.
//! - The **gate tally**, naming what was refused. `§5` makes the rejection rate
//!   a measurement of our own tool descriptions, and a rejection nobody sees
//!   cannot be one.

use crate::dispatch::{Dispatch, clickable, ignore};
use crate::state::{Action, AppState};
use crate::theme::{Theme, radius, space, type_scale};
use crate::widgets::{Badge, Section, Tone};
use binmap_core::reasoner::{Mode, Reasoner};
use binmap_core::transcript::{CallStatus, Origin, SessionCost, StopReason, TranscriptEvent};
use gpui_kit::prelude::*;
use gpui_kit::{App, SharedString, Window, div, px};

#[derive(IntoElement)]
pub struct AgentPanel {
    reasoners: Vec<Reasoner>,
    selected: String,
    refusal: Option<String>,
    events: Vec<TranscriptEvent>,
    cost: SessionCost,
    accepted: usize,
    rejected: usize,
    can_reason: bool,
    running: bool,
    theme: Theme,
    dispatch: Dispatch,
}

impl AgentPanel {
    pub fn of(state: &AppState, theme: Theme) -> Self {
        Self {
            reasoners: state.reasoner().available.clone(),
            selected: state.reasoner().selected.clone(),
            refusal: state.reasoner_refusal().map(str::to_string),
            events: state.transcript().events().to_vec(),
            cost: state.session_cost(),
            accepted: state.transcript().accepted(),
            rejected: state.transcript().rejected(),
            can_reason: state.can_reason(),
            running: state.transcript().stop_reason().is_none() && !state.transcript().is_empty(),
            theme,
            dispatch: ignore(),
        }
    }

    pub fn dispatching(mut self, dispatch: &Dispatch) -> Self {
        self.dispatch = std::rc::Rc::clone(dispatch);
        self
    }
}

/// The glyph for a transcript entry, matching the provenance vocabulary where
/// one applies.
fn glyph(event: &TranscriptEvent) -> &'static str {
    match event {
        TranscriptEvent::Started { .. } => "▸",
        TranscriptEvent::Hypothesis { .. } => "?",
        TranscriptEvent::Message { .. } => "·",
        TranscriptEvent::Thought { .. } => "◌",
        TranscriptEvent::ToolCall { .. } => "→",
        TranscriptEvent::ToolResult { status, .. } => match status {
            CallStatus::Succeeded => "←",
            CallStatus::Failed => "✕",
            _ => "…",
        },
        TranscriptEvent::ClaimRejected { .. } => "✕",
        TranscriptEvent::ClaimAccepted { .. } => "◆",
        TranscriptEvent::Finished { .. } => "▪",
    }
}

fn tone_of(event: &TranscriptEvent) -> Tone {
    match event {
        TranscriptEvent::ClaimAccepted { .. } => Tone::Pass,
        TranscriptEvent::ClaimRejected { .. } => Tone::Fail,
        TranscriptEvent::ToolResult { status: CallStatus::Failed, .. } => Tone::Fail,
        TranscriptEvent::Hypothesis { .. } => Tone::Accent,
        TranscriptEvent::Finished { reason, .. } => match reason {
            StopReason::Concluded => Tone::Pass,
            StopReason::Failed { .. } => Tone::Fail,
            _ => Tone::Warn,
        },
        _ => Tone::Neutral,
    }
}

/// The second line of a card, where an entry has more to say than its headline.
fn detail_of(event: &TranscriptEvent) -> Option<String> {
    match event {
        // The refutation is the half that makes it a hypothesis, so it is
        // always shown rather than hidden behind an expander.
        TranscriptEvent::Hypothesis { refuted_by, .. } => Some(format!("Refuted by: {refuted_by}")),
        TranscriptEvent::ToolCall { arguments, .. } => {
            let rendered = arguments.to_string();
            (rendered != "{}").then_some(rendered)
        }
        TranscriptEvent::ToolResult { summary, .. } => Some(summary.clone()),
        TranscriptEvent::ClaimRejected { reason, .. } => Some(reason.clone()),
        TranscriptEvent::Message { text, .. } | TranscriptEvent::Thought { text, .. } => {
            // The headline already holds the first line; a second line only
            // earns its space when there is more than that.
            (text.lines().count() > 1).then(|| text.lines().skip(1).collect::<Vec<_>>().join(" "))
        }
        _ => None,
    }
}

impl RenderOnce for AgentPanel {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let c = self.theme.colours;
        let theme = self.theme;
        let dispatch = self.dispatch;
        let start = std::rc::Rc::clone(&dispatch);

        let selected_label = self
            .reasoners
            .iter()
            .find(|reasoner| reasoner.id == self.selected)
            .map(|reasoner| reasoner.describe())
            .unwrap_or_else(|| "None (deterministic only)".to_string());

        div()
            .flex()
            .flex_col()
            .size_full()
            .gap(space::S12)
            .p(space::S16)
            .overflow_hidden()
            // -- header, and the one action ------------------------------
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(space::S8)
                    .flex_none()
                    .child(
                        div()
                            .text_size(type_scale::FS_18)
                            .text_color(c.text_primary)
                            .child("Agent"),
                    )
                    .child(
                        div().flex_1().min_w_0().text_size(type_scale::FS_12).text_color(
                            c.text_muted,
                        ).child(
                            "Every claim a model makes is checked against the evidence it cites. \
                             What was refused is shown beside what was kept.",
                        ),
                    )
                    .when(self.rejected > 0, |d| {
                        d.child(
                            Badge::new(format!("{} refused", self.rejected), Tone::Fail, theme)
                                .caps(),
                        )
                    })
                    .when(self.accepted > 0, |d| {
                        d.child(
                            Badge::new(format!("{} kept", self.accepted), Tone::Pass, theme).caps(),
                        )
                    })
                    .child({
                        // Disabled rather than absent when there is nothing to
                        // ask: a button that vanishes teaches nobody why.
                        let enabled = self.can_reason && !self.running;
                        let button = div()
                            .id("start-reasoning")
                            .flex()
                            .flex_none()
                            .items_center()
                            .h(space::CONTROL_H_SM)
                            .px(space::S8)
                            .rounded(radius::CONTROL)
                            .border_1()
                            .border_color(if enabled { c.border_default } else { c.border_subtle })
                            .text_size(type_scale::FS_11)
                            .text_color(if enabled { c.text_secondary } else { c.text_muted })
                            .child(if self.running { "Working…" } else { "Ask" });
                        if enabled {
                            clickable(button, &start, Action::StartReasoning)
                                .hover(|d| d.bg(c.surface_hover))
                                .into_any_element()
                        } else {
                            button.into_any_element()
                        }
                    }),
            )
            // -- the picker (§9.1) ---------------------------------------
            .child({
                let dispatch = std::rc::Rc::clone(&dispatch);
                Section::titled("Reasoner", theme).flush().child(
                    div()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .px(space::S12)
                                .py(space::S6)
                                .text_size(type_scale::FS_11)
                                .text_color(c.text_muted)
                                .child(SharedString::from(format!("Chosen: {selected_label}"))),
                        )
                        // The refusal from the last attempt, if there was one.
                        // Saying nothing would make the click look ignored.
                        .when_some(self.refusal.clone(), |d, refusal| {
                            d.child(
                                div()
                                    .px(space::S12)
                                    .py(space::S6)
                                    .text_size(type_scale::FS_11)
                                    .text_color(c.status_warn)
                                    .child(SharedString::from(refusal)),
                            )
                        })
                        .children(self.reasoners.into_iter().map(move |reasoner| {
                            let dispatch = std::rc::Rc::clone(&dispatch);
                            let chosen = reasoner.id == self.selected;
                            let selectable = reasoner.is_selectable();
                            let id = reasoner.id.clone();

                            let row = div()
                                .id(SharedString::from(format!("reasoner-{id}")))
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(space::S8)
                                .flex_none()
                                .px(space::S12)
                                .py(space::S6)
                                .border_b_1()
                                .border_color(c.border_subtle)
                                .when(chosen, |d| d.bg(c.surface_hover))
                                .child(
                                    div()
                                        .flex_none()
                                        .w(px(14.))
                                        .text_size(type_scale::FS_11)
                                        .text_color(c.text_accent)
                                        .child(if chosen { "●" } else { "○" }),
                                )
                                .child(
                                    div()
                                        .flex_none()
                                        .w(px(44.))
                                        .child(
                                            Badge::new(
                                                reasoner.mode.label(),
                                                match reasoner.mode {
                                                    Mode::Native => Tone::Info,
                                                    // §1: an external agent's
                                                    // grounding is weaker, and
                                                    // the badge is how the UI
                                                    // says so at the point of
                                                    // choice.
                                                    Mode::External => Tone::Warn,
                                                    Mode::None => Tone::Neutral,
                                                },
                                                theme,
                                            )
                                            .caps(),
                                        ),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .text_size(type_scale::FS_12)
                                        .text_color(if selectable {
                                            c.text_body
                                        } else {
                                            c.text_muted
                                        })
                                        .child(SharedString::from(reasoner.display.clone())),
                                )
                                // §11: whether choosing this sends code off the
                                // machine, at the point of choice rather than
                                // in a settings page.
                                .when(reasoner.cloud, |d| {
                                    d.child(
                                        div()
                                            .flex_none()
                                            .child(Badge::new("cloud", Tone::Warn, theme).caps()),
                                    )
                                })
                                .child(
                                    div()
                                        .flex_none()
                                        .w(px(150.))
                                        .text_size(type_scale::FS_11)
                                        .text_color(c.text_muted)
                                        .child(SharedString::from(
                                            reasoner
                                                .unavailable
                                                .as_ref()
                                                .map(|reason| reason.describe())
                                                .unwrap_or_else(|| reasoner.price_label()),
                                        )),
                                );

                            if selectable {
                                clickable(row, &dispatch, Action::SelectReasoner(id))
                                    .hover(|d| d.bg(c.surface_hover))
                                    .into_any_element()
                            } else {
                                // Listed but not clickable — §6.3's "not merely
                                // hidden but unselectable, with the reason
                                // shown".
                                row.into_any_element()
                            }
                        })),
                )
            })
            // -- the cost meter (§9.3) ------------------------------------
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(space::S8)
                    .flex_none()
                    .px(space::S12)
                    .py(space::S6)
                    .rounded(radius::CONTROL)
                    .border_1()
                    .border_color(c.border_subtle)
                    .child(
                        div()
                            .flex_none()
                            .text_size(type_scale::FS_11)
                            .text_color(c.text_muted)
                            .child("Budget"),
                    )
                    // A bar rather than only a number, because "18 of 25" is
                    // read as a fraction and drawn as one.
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .h(px(4.))
                            .rounded(radius::CONTROL)
                            .bg(c.border_subtle)
                            .child(
                                div()
                                    .h_full()
                                    .w(gpui_kit::relative(self.cost.step_fraction()))
                                    .rounded(radius::CONTROL)
                                    .bg(if self.cost.step_fraction() > 0.8 {
                                        c.status_warn
                                    } else {
                                        c.text_accent
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .flex_none()
                            .font_family("JetBrains Mono")
                            .text_size(type_scale::FS_11)
                            .text_color(c.text_secondary)
                            .child(SharedString::from(self.cost.label())),
                    ),
            )
            // -- the transcript (§9.2) -----------------------------------
            .child(
                Section::titled("Transcript", theme).flush().child(
                    div()
                        .id("transcript")
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_h_0()
                        // The transcript is the one thing here that grows
                        // without bound, so it is what scrolls.
                        .overflow_y_scroll()
                        .when(self.events.is_empty(), |d| {
                            d.child(
                                div()
                                    .px(space::S12)
                                    .py(space::S12)
                                    .text_size(type_scale::FS_11)
                                    .text_color(c.text_muted)
                                    .child(
                                        "Nothing yet. Every analysis in this phase works without \
                                         a model — a reasoner is for the questions measurement \
                                         cannot answer on its own.",
                                    ),
                            )
                        })
                        .children(self.events.into_iter().map(move |event| {
                            let tone = tone_of(&event);
                            let detail = detail_of(&event);
                            let badge = event.origin().needs_badge();
                            let headline = event.headline();
                            let mark = glyph(&event);

                            div()
                                .flex()
                                .flex_col()
                                .gap(space::S2)
                                .flex_none()
                                .px(space::S12)
                                .py(space::S6)
                                .border_b_1()
                                .border_color(c.border_subtle)
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap(space::S8)
                                        .child(
                                            div()
                                                .flex_none()
                                                .w(px(14.))
                                                .text_size(type_scale::FS_11)
                                                .text_color(match tone {
                                                    Tone::Pass => c.status_pass,
                                                    Tone::Fail => c.status_fail,
                                                    Tone::Warn => c.status_warn,
                                                    Tone::Accent => c.text_accent,
                                                    _ => c.text_muted,
                                                })
                                                .child(mark),
                                        )
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .text_size(type_scale::FS_12)
                                                .text_color(c.text_body)
                                                .child(SharedString::from(headline)),
                                        )
                                        // §1: an external claim is badged, and
                                        // the transcript kept the distinction
                                        // precisely so this is possible.
                                        .when(badge, |d| {
                                            d.child(div().flex_none().child(
                                                Badge::new("external", Tone::Warn, theme).caps(),
                                            ))
                                        }),
                                )
                                // A second row, never a squeezed column: a long
                                // detail sharing one row wraps a character per
                                // line and makes the row enormous.
                                .when_some(detail, |d, detail| {
                                    d.child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .pl(px(22.))
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .min_w_0()
                                                    .font_family("JetBrains Mono")
                                                    .text_size(type_scale::FS_11)
                                                    .text_color(c.text_muted)
                                                    .child(SharedString::from(detail)),
                                            ),
                                    )
                                })
                        })),
                ),
            )
    }
}

/// Whether a transcript entry should be badged as externally reached.
///
/// Exposed for the tests, which assert the property the design requires rather
/// than reaching into the rendered tree to find a badge.
pub fn needs_external_badge(event: &TranscriptEvent) -> bool {
    event.origin() == Origin::External
}
