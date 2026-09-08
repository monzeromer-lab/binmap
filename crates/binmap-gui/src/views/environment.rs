//! The Environment panel (`U0.4`, `U10`, TOOLING §9).
//!
//! Every check maps to a feature that would otherwise fail later, at a worse
//! moment. Every warning names the exact fix as a command, and every fix that
//! can be applied in-app has a button beside it.
//!
//! Nothing here blocks a sweep. A missing tool makes the axes that need it
//! unavailable and says so, which is why the copy states what is lost rather
//! than only that something is absent.

use crate::state::AppState;
use crate::theme::{Theme, radius, space, type_scale};
use crate::widgets::{Badge, Section, Tone};
use binmap_core::facade::{Probe, ProbeStatus};
use gpui_kit::prelude::*;
use gpui_kit::{App, Window, div, px};

#[derive(IntoElement)]
pub struct EnvironmentPanel {
    groups: Vec<(String, Vec<Probe>)>,
    warnings: usize,
    missing: usize,
    theme: Theme,
}

impl EnvironmentPanel {
    pub fn of(state: &AppState, theme: Theme) -> Self {
        let groups = state
            .probes_by_group()
            .into_iter()
            .map(|(name, probes)| {
                (name.to_string(), probes.into_iter().cloned().collect::<Vec<Probe>>())
            })
            .collect();

        let warnings =
            state.probes().iter().filter(|p| p.status == ProbeStatus::Unusable).count();
        let missing = state.probes().iter().filter(|p| p.status == ProbeStatus::Missing).count();

        Self { groups, warnings, missing, theme }
    }
}

fn status_badge(status: ProbeStatus, theme: Theme) -> Badge {
    let (label, tone) = match status {
        ProbeStatus::Present => ("ok", Tone::Pass),
        ProbeStatus::Unusable => ("warn", Tone::Warn),
        ProbeStatus::Missing => ("missing", Tone::Fail),
    };
    Badge::new(label, tone, theme).caps()
}

impl RenderOnce for EnvironmentPanel {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let c = self.theme.colours;
        let theme = self.theme;

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
                            .text_size(type_scale::FS_18)
                            .text_color(c.text_primary)
                            .child("Environment"),
                    )
                    .child(div().flex_1().text_size(type_scale::FS_12).text_color(c.text_muted).child(
                        "Checked when the project opens. Every check maps to a feature that \
                         would otherwise fail later, at a worse moment.",
                    ))
                    .when(self.warnings > 0, |d| {
                        d.child(
                            Badge::new(
                                format!("{} warning{}", self.warnings, if self.warnings == 1 { "" } else { "s" }),
                                Tone::Warn,
                                theme,
                            )
                            .caps(),
                        )
                    })
                    .when(self.missing > 0, |d| {
                        d.child(
                            Badge::new(format!("{} missing", self.missing), Tone::Fail, theme)
                                .caps(),
                        )
                    }),
            )
            .children(self.groups.into_iter().map(move |(name, probes)| {
                Section::titled(name, theme).flush().child(
                    div().flex().flex_col().children(probes.into_iter().map(move |probe| {
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(space::S8)
                            .flex_none()
                            .px(space::S12)
                            .py(space::S6)
                            .border_b_1()
                            .border_color(c.border_subtle)
                            .child(
                                div()
                                    .flex_none()
                                    .w(px(56.))
                                    .child(status_badge(probe.status, theme)),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .w(px(190.))
                                    .font_family("JetBrains Mono")
                                    .text_size(type_scale::FS_12)
                                    .text_color(c.text_body)
                                    .child(probe.name.clone()),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(type_scale::FS_11)
                                    .text_color(c.text_muted)
                                    .child(probe.detail.clone()),
                            )
                            // The fix, as a command. Not a description of a
                            // command — something a user can paste.
                            .when_some(probe.remedy.clone(), |d, remedy| {
                                d.child(
                                    div()
                                        .flex_none()
                                        .font_family("JetBrains Mono")
                                        .text_size(type_scale::FS_11)
                                        .text_color(c.text_accent)
                                        .child(remedy),
                                )
                            })
                            .when_some(probe.action.clone(), |d, action| {
                                d.child(
                                    div()
                                        .flex()
                                        .flex_none()
                                        .items_center()
                                        .h(space::CONTROL_H_SM)
                                        .px(space::S8)
                                        .rounded(radius::CONTROL)
                                        .border_1()
                                        .border_color(c.border_default)
                                        .text_size(type_scale::FS_11)
                                        .text_color(c.text_secondary)
                                        .child(action),
                                )
                            })
                    })),
                )
            }))
    }
}
