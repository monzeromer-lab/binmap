//! The first-run flow (`U0.1`, `U0.4`, `AI.1`).
//!
//! Three steps, run when a project is opened for the first time and skipped
//! when a previous session restores — you configured it once, and being walked
//! through it again is a tool that does not remember you.
//!
//! Every step is skippable. None of what it asks blocks a sweep: a missing
//! tool makes the axes that need it unavailable, a missing benchmark makes
//! runtime not an objective, and a missing reasoner is the Phase 0 default.
//! A first-run flow that gates the product on answering it is a tool people
//! close.

use crate::dispatch::{Dispatch, clickable};
use crate::state::{Action, AppState, Stage};
use crate::theme::{Theme, radius, space, type_scale};
use crate::widgets::{Badge, Section, Tone, eyebrow};
use binmap_core::facade::ProbeStatus;
use gpui_kit::prelude::*;
use gpui_kit::{App, SharedString, Window, div, px};

#[derive(IntoElement)]
pub struct FirstRun {
    stage: Stage,
    state: FlowFacts,
    theme: Theme,
    dispatch: Dispatch,
}

/// What the flow needs to know, lifted out of `AppState` so the view does not
/// hold a borrow across its own construction.
struct FlowFacts {
    project: String,
    targets: Vec<(String, String)>,
    selected_target: Option<String>,
    benchmark: Option<String>,
    unmet: Vec<(String, ProbeStatus, String, Option<String>)>,
    probes_checked: usize,
}

impl FirstRun {
    pub fn new(
        stage: Stage,
        state: &AppState,
        project: impl Into<String>,
        benchmark: Option<String>,
        theme: Theme,
        dispatch: &Dispatch,
    ) -> Self {
        let unmet = state
            .probes()
            .iter()
            .filter(|probe| probe.needs_attention())
            .map(|probe| {
                (probe.name.clone(), probe.status, probe.detail.clone(), probe.remedy.clone())
            })
            .collect();

        Self {
            stage,
            state: FlowFacts {
                project: project.into(),
                targets: state
                    .targets()
                    .iter()
                    .map(|t| (t.id.clone(), t.capabilities.sentence()))
                    .collect(),
                selected_target: state.selected_target().map(|t| t.id.clone()),
                benchmark,
                unmet,
                probes_checked: state.probes().len(),
            },
            theme,
            dispatch: std::rc::Rc::clone(dispatch),
        }
    }
}

impl RenderOnce for FirstRun {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let c = self.theme.colours;
        let theme = self.theme;
        let stage = self.stage;
        let dispatch = self.dispatch;

        let body = match stage {
            Stage::Environment => environment(&self.state, theme, &dispatch).into_any_element(),
            Stage::Configure => configure(&self.state, theme, &dispatch).into_any_element(),
            Stage::Reasoner => reasoner(theme).into_any_element(),
            Stage::Ready => div().into_any_element(),
        };

        div()
            .flex()
            .flex_col()
            .items_center()
            .size_full()
            .bg(c.surface_app)
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(space::S12)
                    .w(px(760.))
                    .max_w_full()
                    .pt(px(64.))
                    .px(space::S24)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(space::S6)
                            .py(space::S4)
                            .when_some(stage.label(), |d, label| d.child(eyebrow(label, theme)))
                            .child(
                                div()
                                    .text_size(type_scale::FS_22)
                                    .text_color(c.text_primary)
                                    .child(stage.heading()),
                            )
                            .child(
                                div()
                                    .max_w(px(620.))
                                    .text_size(type_scale::FS_13)
                                    .text_color(c.text_secondary)
                                    .child(stage.blurb()),
                            ),
                    )
                    .child(body)
                    .child(footer(stage, theme, &dispatch)),
            )
    }
}

/// Step 1: only what needs attention. A list of everything that is fine is a
/// list nobody reads.
fn environment(facts: &FlowFacts, theme: Theme, dispatch: &Dispatch) -> impl IntoElement {
    let c = theme.colours;

    if facts.unmet.is_empty() {
        return Section::titled("Nothing needs attention", theme).child(
            div().text_size(type_scale::FS_12).text_color(c.text_secondary).child(format!(
                "All {} checks passed. Every axis of the sweep is available.",
                facts.probes_checked
            )),
        );
    }

    let dispatch = std::rc::Rc::clone(dispatch);
    Section::titled("Needs attention", theme)
        .with_trailing(Badge::new(facts.unmet.len().to_string(), Tone::Warn, theme).mono())
        .children(facts.unmet.iter().cloned().map(move |(name, status, detail, remedy)| {
            let (label, tone) = match status {
                ProbeStatus::Missing => ("missing", Tone::Fail),
                _ => ("warn", Tone::Warn),
            };
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(space::S10)
                .min_h(px(34.))
                .child(div().w(px(66.)).flex_none().child(Badge::new(label, tone, theme).caps()))
                .child(
                    div()
                        .flex_none()
                        .w(px(200.))
                        .font_family("JetBrains Mono")
                        .text_size(type_scale::FS_12)
                        .text_color(c.text_primary)
                        .child(name.clone()),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(type_scale::FS_11)
                        .text_color(c.text_muted)
                        .child(detail),
                )
                // The fix, as a command. Not a description of one.
                .when_some(remedy, |d, remedy| {
                    d.child(
                        div()
                            .flex_none()
                            .font_family("JetBrains Mono")
                            .text_size(type_scale::FS_11)
                            .text_color(c.status_warn)
                            .child(remedy),
                    )
                })
                .child(
                    clickable(
                        div().id(SharedString::from(format!("flow-recheck-{name}"))),
                        &dispatch,
                        Action::RecheckEnvironment,
                    )
                    .flex()
                    .flex_none()
                    .items_center()
                    .h(space::CONTROL_H_SM)
                    .px(space::S8)
                    .rounded(radius::CONTROL)
                    .border_1()
                    .border_color(c.border_default)
                    .hover(|d| d.bg(c.surface_hover))
                    .text_size(type_scale::FS_11)
                    .text_color(c.text_secondary)
                    .child("Re-check"),
                )
        }))
}

/// Step 2: what was discovered, and the one thing that cannot be.
fn configure(facts: &FlowFacts, theme: Theme, dispatch: &Dispatch) -> impl IntoElement {
    let c = theme.colours;
    let selected = facts.selected_target.clone();
    let dispatch = std::rc::Rc::clone(dispatch);

    Section::titled(
        format!(
            "{} · {} target{} discovered",
            facts.project,
            facts.targets.len(),
            if facts.targets.len() == 1 { "" } else { "s" }
        ),
        theme,
    )
    .child(eyebrow("Target", theme))
    .children(facts.targets.iter().cloned().map(move |(id, capabilities)| {
        let active = selected.as_deref() == Some(id.as_str());
        clickable(
            div().id(SharedString::from(format!("flow-target-{id}"))),
            &dispatch,
            Action::SelectTarget(id.clone()),
        )
        .flex()
        .flex_row()
        .items_center()
        .gap(space::S8)
        .p(space::S8)
        .rounded(radius::INPUT)
        .when(active, |d| d.bg(c.surface_selected).border_1().border_color(c.accent))
        .when(!active, |d| d.hover(|d| d.bg(c.surface_hover)))
        .child(
            div()
                .flex_none()
                .w(px(10.))
                .text_color(if active { c.accent } else { c.text_disabled })
                .child(if active { "●" } else { "○" }),
        )
        .child(
            div()
                .flex_none()
                .font_family("JetBrains Mono")
                .text_size(type_scale::FS_12)
                .text_color(c.text_primary)
                .child(id),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(type_scale::FS_11)
                .text_color(c.text_muted)
                .child(capabilities),
        )
    }))
    .child(div().h(space::S4))
    .child(eyebrow("Benchmark command", theme))
    .child(match &facts.benchmark {
        Some(command) => {
            div()
                .flex()
                .flex_col()
                .gap(space::S4)
                .child(
                    div()
                        .p(space::S8)
                        .rounded(radius::INPUT)
                        .bg(c.surface_code)
                        .font_family("JetBrains Mono")
                        .text_size(type_scale::FS_12)
                        .text_color(c.text_body)
                        .child(command.clone()),
                )
                .child(div().text_size(type_scale::FS_11).text_color(c.text_muted).child(
                    "Measured with hyperfine. Runtime becomes an objective on the frontier.",
                ))
        }
        // Not an error, and not a blocker. Runtime is simply not an objective,
        // which is a different thing from being fast.
        None => div()
            .flex()
            .flex_col()
            .gap(space::S4)
            .child(
                div()
                    .text_size(type_scale::FS_12)
                    .text_color(c.text_secondary)
                    .child("None declared, so runtime is not an objective."),
            )
            .child(
                div()
                    .p(space::S8)
                    .rounded(radius::INPUT)
                    .bg(c.surface_code)
                    .font_family("JetBrains Mono")
                    .text_size(type_scale::FS_11)
                    .text_color(c.text_accent)
                    .child(
                        "# binmap.toml\n[benchmark]\nprogram = \"{artifact}\"\n\
                         arguments = [\"--bench\"]",
                    ),
            ),
    })
}

/// Step 3: the reasoner. In Phase 0 there is exactly one honest answer.
fn reasoner(theme: Theme) -> impl IntoElement {
    let c = theme.colours;

    // `R3`: a deterministic tool with an optional layer. No reasoner is a
    // listed choice, not a hidden flag — so it is listed, and selected, and
    // the rest say why they are unavailable rather than being absent.
    let option = |name: &'static str, detail: &'static str, available: bool| {
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(space::S8)
            .p(space::S8)
            .rounded(radius::INPUT)
            .when(available, |d| d.bg(c.surface_selected).border_1().border_color(c.accent))
            .child(
                div()
                    .flex_none()
                    .w(px(10.))
                    .text_color(if available { c.accent } else { c.text_disabled })
                    .child(if available { "●" } else { "○" }),
            )
            .child(
                div()
                    .flex_none()
                    .w(px(180.))
                    .text_size(type_scale::FS_12)
                    .text_color(if available { c.text_primary } else { c.text_disabled })
                    .child(name),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(type_scale::FS_11)
                    .text_color(c.text_muted)
                    .child(detail),
            )
    };

    Section::titled("Reasoner", theme)
        .child(option(
            "No reasoner",
            "Binmap measures, derives and reports. Everything it says is a tool's output or a \
             named rule applied to one.",
            true,
        ))
        .child(option(
            "Local runner",
            "Not in this build. The model layer is a null backend in Phase 0: the tool registry \
             and evidence store exist, and nothing calls a model.",
            false,
        ))
        .child(option(
            "External agent over ACP",
            "Not in this build. Arrives in Phase 2, behind the same airlock — a claim citing \
             evidence we never issued is refused whoever made it.",
            false,
        ))
        .child(
            div()
                .pt(space::S4)
                .text_size(type_scale::FS_11)
                .text_color(c.text_muted)
                .child("You can change this at any time from the title bar."),
        )
}

/// Back, skip, and on. Every step is skippable.
fn footer(stage: Stage, theme: Theme, dispatch: &Dispatch) -> impl IntoElement {
    let c = theme.colours;
    let last = stage.next() == Stage::Ready;

    let secondary = |id: &'static str, label: &'static str, action: Action, d: &Dispatch| {
        clickable(div().id(id), d, action)
            .flex()
            .flex_none()
            .items_center()
            .h(space::CONTROL_H_MD)
            .px(space::S12)
            .rounded(radius::CONTROL)
            .border_1()
            .border_color(c.border_default)
            .hover(|d| d.bg(c.surface_hover))
            .text_size(type_scale::FS_12)
            .text_color(c.text_secondary)
            .child(label)
    };

    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(space::S8)
        .pt(space::S8)
        .when(stage.previous().is_some(), |d| {
            d.child(secondary("flow-back", "Back", Action::FlowBack, dispatch))
        })
        .child(div().flex_1())
        .child(secondary("flow-skip", "Skip setup", Action::FlowSkip, dispatch))
        .child(
            clickable(div().id("flow-next"), dispatch, Action::FlowNext)
                .flex()
                .flex_none()
                .items_center()
                .h(space::CONTROL_H_MD)
                .px(space::S16)
                .rounded(radius::CONTROL)
                .bg(c.accent)
                .hover(|d| d.bg(c.accent_hover))
                .text_size(type_scale::FS_12)
                .text_color(c.on_accent)
                .child(if last { "Open the project" } else { "Continue" }),
        )
}
