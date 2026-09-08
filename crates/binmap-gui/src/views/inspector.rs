//! The Findings Inspector (`U0.3`, `U4`, `AI.6`, `AI.7`).
//!
//! Always present. The Inspector stays on screen because a claim and its
//! evidence belong on the same screen — there is no view where one is
//! available and the other is not.
//!
//! Every finding renders through [`ProvenanceBadge`], which can only be built
//! from a `Finding`. That is DESIGN-GUI §6.5's "enforce by construction": there
//! is no code path here that draws a claim without its provenance.

use crate::theme::{Theme, radius, space, type_scale};
use crate::widgets::{Badge, ProvenanceBadge, Tone, eyebrow};
use binmap_core::evidence::Evidence;
use binmap_core::finding::Finding;
use gpui_kit::prelude::*;
use gpui_kit::{App, Window, div, px};

/// Which tab of the Inspector is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Hypothesis,
    Evidence,
    Proposal,
}

impl Tab {
    pub fn label(self) -> &'static str {
        match self {
            Tab::Hypothesis => "Hypothesis",
            Tab::Evidence => "Evidence",
            Tab::Proposal => "Proposal",
        }
    }

    pub const ALL: [Tab; 3] = [Tab::Hypothesis, Tab::Evidence, Tab::Proposal];
}

#[derive(IntoElement)]
pub struct Inspector {
    finding: Option<Finding>,
    evidence: Vec<Evidence>,
    tab: Tab,
    measured: usize,
    inferred: usize,
    theme: Theme,
}

impl Inspector {
    pub fn new(
        finding: Option<Finding>,
        evidence: Vec<Evidence>,
        tab: Tab,
        all: &[Finding],
        theme: Theme,
    ) -> Self {
        let measured =
            all.iter().filter(|f| matches!(f.provenance, binmap_core::Provenance::Measured)).count();
        let inferred = all
            .iter()
            .filter(|f| {
                matches!(
                    f.provenance,
                    binmap_core::Provenance::InferredNatively { .. }
                        | binmap_core::Provenance::InferredExternally { .. }
                )
            })
            .count();
        Self { finding, evidence, tab, measured, inferred, theme }
    }
}

impl RenderOnce for Inspector {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let c = self.theme.colours;
        let theme = self.theme;

        let header = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(space::S8)
            .flex_none()
            .px(space::S12)
            .h(px(34.))
            .border_b_1()
            .border_color(c.border_subtle)
            .child(div().flex_1().child(eyebrow("Findings Inspector", theme)))
            .when(self.measured > 0, |d| {
                d.child(
                    div()
                        .text_size(type_scale::FS_11)
                        .text_color(c.prov_measured)
                        .child(format!("● {}", self.measured)),
                )
            })
            .when(self.inferred > 0, |d| {
                d.child(
                    div()
                        .text_size(type_scale::FS_11)
                        .text_color(c.prov_inferred)
                        .child(format!("◆ {}", self.inferred)),
                )
            });

        let Some(finding) = self.finding else {
            return div()
                .flex()
                .flex_col()
                .flex_none()
                .w(space::INSPECTOR_W)
                .h_full()
                .bg(c.surface_panel)
                .border_l_1()
                .border_color(c.border_subtle)
                .child(header)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(space::S8)
                        .p(space::S12)
                        .child(
                            div()
                                .text_size(type_scale::FS_12)
                                .text_color(c.text_secondary)
                                .child(
                                    "No findings in this session yet. The Inspector stays on \
                                     screen because a claim and its evidence belong on the same \
                                     screen — there is no view where one is available and the \
                                     other is not.",
                                ),
                        )
                        .child(
                            div()
                                .text_size(type_scale::FS_11)
                                .text_color(c.text_muted)
                                .child(
                                    "A finding is never constructed without at least one \
                                     evidence item.",
                                ),
                        ),
                );
        };

        let tab = self.tab;
        let evidence_count = self.evidence.len();

        div()
            .flex()
            .flex_col()
            .flex_none()
            .w(space::INSPECTOR_W)
            .h_full()
            .bg(c.surface_panel)
            .border_l_1()
            .border_color(c.border_subtle)
            .child(header)
            // Tabs.
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(space::S12)
                    .flex_none()
                    .px(space::S12)
                    .h(px(30.))
                    .items_center()
                    .border_b_1()
                    .border_color(c.border_subtle)
                    .children(Tab::ALL.into_iter().map(move |candidate| {
                        let active = candidate == tab;
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(space::S4)
                            .text_size(type_scale::FS_12)
                            .text_color(if active { c.text_primary } else { c.text_muted })
                            .when(active, |d| d.border_b_2().border_color(c.accent))
                            .child(candidate.label())
                            .when(candidate == Tab::Evidence && evidence_count > 0, |d| {
                                d.child(
                                    div()
                                        .font_family("JetBrains Mono")
                                        .text_color(c.text_muted)
                                        .child(evidence_count.to_string()),
                                )
                            })
                    })),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .gap(space::S8)
                    .p(space::S12)
                    .overflow_hidden()
                    // The badge, always. This is the P2 guarantee.
                    .child(ProvenanceBadge::of(&finding, theme).verbose())
                    .child(
                        div()
                            .text_size(type_scale::FS_14)
                            .text_color(c.text_primary)
                            .child(finding.title.clone()),
                    )
                    .map(|d| match tab {
                        Tab::Hypothesis => d.child(hypothesis(&finding, theme)),
                        Tab::Evidence => d.child(evidence_list(&self.evidence, theme)),
                        Tab::Proposal => d.child(
                            div()
                                .text_size(type_scale::FS_12)
                                .text_color(c.text_muted)
                                .child("No proposal for this finding."),
                        ),
                    }),
            )
    }
}

fn hypothesis(finding: &Finding, theme: Theme) -> impl IntoElement {
    let c = theme.colours;
    let impact = finding.impact;

    div()
        .flex()
        .flex_col()
        .gap(space::S8)
        .when(!finding.detail.is_empty(), |d| {
            d.child(
                div()
                    .text_size(type_scale::FS_12)
                    .text_color(c.text_body)
                    .child(finding.detail.clone()),
            )
        })
        .child(
            div()
                .font_family("JetBrains Mono")
                .text_size(type_scale::FS_11)
                .text_color(c.text_muted)
                .child(finding.location.describe()),
        )
        .when_some(impact.size_bytes, |d, bytes| {
            // Inside the noise floor draws flat — neither green nor red.
            let colour = if impact.within_noise_floor {
                c.delta_flat
            } else {
                c.delta(Some(bytes < 0))
            };
            d.child(
                div()
                    .flex()
                    .flex_row()
                    .gap(space::S8)
                    .items_center()
                    .child(
                        div()
                            .text_size(type_scale::FS_11)
                            .text_color(c.text_muted)
                            .child("impact"),
                    )
                    .child(
                        div()
                            .font_family("JetBrains Mono")
                            .text_size(type_scale::FS_12)
                            .text_color(colour)
                            .child(format!("{bytes:+} bytes")),
                    )
                    .when(impact.within_noise_floor, |d| {
                        d.child(Badge::new("inconclusive", Tone::Neutral, theme))
                    }),
            )
        })
}

/// Each evidence row carries the tool, its arguments, a digest and the
/// verbatim output. The output is shown, never summarised: a summary is a
/// claim, and claims need evidence of their own.
fn evidence_list(evidence: &[Evidence], theme: Theme) -> impl IntoElement {
    let c = theme.colours;

    div().flex().flex_col().gap(space::S12).children(evidence.iter().map(move |record| {
        div()
            .flex()
            .flex_col()
            .gap(space::S4)
            .child(
                div()
                    .font_family("JetBrains Mono")
                    .text_size(type_scale::FS_11)
                    .text_color(c.text_accent)
                    .child(record.invocation.command_line()),
            )
            .child(
                div()
                    .p(space::S8)
                    .rounded(radius::INPUT)
                    .bg(c.surface_code)
                    .font_family("JetBrains Mono")
                    .text_size(type_scale::FS_11)
                    .text_color(c.text_body)
                    .child(record.output.lines().take(12).collect::<Vec<_>>().join("\n")),
            )
            .child(
                div()
                    .font_family("JetBrains Mono")
                    .text_size(type_scale::FS_11)
                    .text_color(c.text_disabled)
                    .child(format!("output digest {}", &record.digest[..12.min(record.digest.len())])),
            )
    }))
}
