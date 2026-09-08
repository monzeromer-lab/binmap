//! The targets sidebar and the Target view.
//!
//! Targets are grouped by language, and each states in plain words what Binmap
//! can do with it (§2.5). "not analysed" is a fact about the session, not a
//! failure, and reads as one.

use crate::state::AppState;
use crate::theme::{Theme, radius, space, type_scale};
use crate::widgets::{Badge, Section, Tone, badge::fact, eyebrow};
use binmap_core::traits::{Target, TargetFamily};
use gpui_kit::prelude::*;
use gpui_kit::{App, Window, div, px};

/// The 208px sidebar listing every target, grouped by family.
#[derive(IntoElement)]
pub struct TargetList {
    groups: Vec<(TargetFamily, Vec<Target>)>,
    selected: Option<String>,
    theme: Theme,
}

impl TargetList {
    pub fn of(state: &AppState, theme: Theme) -> Self {
        let groups = state
            .targets_by_family()
            .into_iter()
            .map(|(family, targets)| {
                (family, targets.into_iter().cloned().collect::<Vec<Target>>())
            })
            .collect();
        Self { groups, selected: state.selected_target().map(|t| t.id.clone()), theme }
    }
}

impl RenderOnce for TargetList {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let c = self.theme.colours;
        let theme = self.theme;
        let selected = self.selected;
        let total: usize = self.groups.iter().map(|(_, targets)| targets.len()).sum();

        div()
            .flex()
            .flex_col()
            .flex_none()
            .w(px(208.))
            .h_full()
            .bg(c.surface_panel)
            .border_r_1()
            .border_color(c.border_subtle)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(space::S6)
                    .flex_none()
                    .px(space::S10)
                    .pt(space::S10)
                    .pb(space::S8)
                    .child(eyebrow("Targets", theme))
                    .child(Badge::new(total.to_string(), Tone::Neutral, theme).mono()),
            )
            .children(self.groups.into_iter().map(move |(family, targets)| {
                let selected = selected.clone();
                div()
                    .flex()
                    .flex_col()
                    .flex_none()
                    .child(
                        div()
                            .flex_none()
                            .px(space::S10)
                            .py(space::S4)
                            .bg(c.surface_raised)
                            .border_t_1()
                            .border_b_1()
                            .border_color(c.border_subtle)
                            .child(eyebrow(family.label(), theme)),
                    )
                    .children(targets.into_iter().map(move |target| {
                        let active = selected.as_deref() == Some(target.id.as_str());
                        div()
                            .flex()
                            .flex_col()
                            .flex_none()
                            .gap(space::S2)
                            .px(space::S10)
                            .py(space::S6)
                            .when(active, |d| {
                                d.bg(c.surface_selected).border_l_2().border_color(c.accent)
                            })
                            .child(
                                div()
                                    .font_family("JetBrains Mono")
                                    .text_size(type_scale::FS_12)
                                    .text_color(if active { c.text_primary } else { c.text_body })
                                    .child(target.name.clone()),
                            )
                            .child(
                                div()
                                    .text_size(type_scale::FS_11)
                                    .text_color(c.text_muted)
                                    // What this target can do, in plain words —
                                    // never a capability list the reader has to
                                    // decode.
                                    .child(target.capabilities.sentence()),
                            )
                    }))
            }))
    }
}

/// The Target view: what is selected, what Binmap knows about it, and what can
/// be run against it.
#[derive(IntoElement)]
pub struct TargetView {
    target: Option<Target>,
    root: Option<String>,
    theme: Theme,
}

impl TargetView {
    pub fn of(state: &AppState, root: Option<String>, theme: Theme) -> Self {
        Self { target: state.selected_target().cloned(), root, theme }
    }
}

impl RenderOnce for TargetView {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let c = self.theme.colours;
        let theme = self.theme;

        let Some(target) = self.target else {
            return div()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .size_full()
                .gap(space::S8)
                .child(
                    div()
                        .text_size(type_scale::FS_16)
                        .text_color(c.text_secondary)
                        .child("No target selected"),
                )
                .child(
                    div()
                        .text_size(type_scale::FS_12)
                        .text_color(c.text_muted)
                        .child("This project offers nothing Binmap can measure."),
                );
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .gap(space::S12)
            .p(space::S16)
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(space::S8)
                    .flex_none()
                    .child(
                        div()
                            .font_family("JetBrains Mono")
                            .text_size(type_scale::FS_18)
                            .text_color(c.text_primary)
                            .child(target.name.clone()),
                    )
                    .child(Badge::new(target.family.label(), Tone::Info, theme))
                    .child(
                        div()
                            .text_size(type_scale::FS_12)
                            .text_color(c.text_muted)
                            .child(target.capabilities.sentence()),
                    ),
            )
            .child(
                Section::titled("Workspace", theme)
                    .child(fact("package", target.package.clone(), theme))
                    .child(fact(
                        "manifest",
                        target.manifest.display().to_string(),
                        theme,
                    ))
                    .when_some(self.root, |section, root| {
                        section.child(fact("path", root, theme))
                    })
                    .child(fact("target id", target.id.clone(), theme)),
            )
            .child(
                Section::titled("Run an analysis", theme).child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(space::S6)
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .h(space::CONTROL_H_LG)
                                .rounded(radius::CONTROL)
                                .bg(c.accent)
                                .text_color(c.on_accent)
                                .text_size(type_scale::FS_13)
                                .child("Sweep build configurations"),
                        )
                        .child(
                            div()
                                .text_size(type_scale::FS_11)
                                .text_color(c.text_muted)
                                .child(
                                    "The sweep sets profile settings per build. \
                                     Your Cargo.toml is never modified.",
                                ),
                        ),
                ),
            )
    }
}
