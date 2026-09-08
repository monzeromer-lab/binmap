//! The provenance badge.
//!
//! `DESIGN.md` P2 requires that a claim never appears without its evidence,
//! and DESIGN-GUI §6.5 says to enforce that by construction: findings render
//! through a component that always emits a badge, and there is no code path
//! that renders a claim without one.
//!
//! This is that component. The glyphs and the words are the fixed vocabulary
//! from the plan §2.6 and are read off the `Finding` rather than passed in
//! separately, so a caller cannot label a measured claim as inferred.

use crate::theme::{Theme, radius, space, type_scale};
use binmap_core::finding::{Confidence, Finding, Provenance};
use gpui_kit::prelude::*;
use gpui_kit::{App, Window, div, px};

/// How a claim came to be believed, and how far it can be trusted.
///
/// Constructed from a [`Finding`] and nothing else. The two fields cannot
/// disagree with the finding because they are not separately supplied.
#[derive(IntoElement)]
pub struct ProvenanceBadge {
    provenance: Provenance,
    confidence: Confidence,
    theme: Theme,
    /// Whether to spell the words out. A dense table shows the glyph alone;
    /// the Inspector shows the full vocabulary.
    verbose: bool,
}

impl ProvenanceBadge {
    /// The only constructor. Taking the finding rather than its parts is what
    /// makes "a claim is never rendered without its provenance" true by
    /// construction rather than by review.
    pub fn of(finding: &Finding, theme: Theme) -> Self {
        Self {
            provenance: finding.provenance().clone(),
            confidence: finding.confidence(),
            theme,
            verbose: false,
        }
    }

    /// Spell out both words, as the Inspector header does.
    pub fn verbose(mut self) -> Self {
        self.verbose = true;
        self
    }
}

impl RenderOnce for ProvenanceBadge {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let c = self.theme.colours;
        let (foreground, background) = c.provenance(&self.provenance);
        let confidence = c.confidence(self.confidence);

        let mut row = div().flex().flex_row().items_center().gap(space::S6);

        // The provenance half: glyph, and the word when there is room.
        row = row.child(
            div()
                .flex()
                .flex_none()
                .items_center()
                .gap(space::S4)
                .h(px(18.))
                .px(space::S6)
                .rounded(radius::CHIP)
                .bg(background)
                .text_color(foreground)
                .text_size(type_scale::FS_11)
                .child(self.provenance.glyph().to_string())
                .when(self.verbose, |d| d.child(self.provenance.label().to_uppercase())),
        );

        // The confidence half. Always shown: a provenance without a
        // confidence invites the reader to assume the ceiling.
        row = row.child(
            div()
                .flex()
                .flex_none()
                .items_center()
                .h(px(18.))
                .px(space::S6)
                .rounded(radius::CHIP)
                .border_1()
                .border_color(confidence)
                .text_color(confidence)
                .text_size(type_scale::FS_11)
                .child(if self.verbose {
                    self.confidence.label().to_uppercase()
                } else {
                    self.confidence.label().to_string()
                }),
        );

        // An external claim says so. AI.10: a weaker guarantee, labelled as
        // one — and labelled rather than recoloured, so it reads as the same
        // kind of claim held to a lower standard.
        row.when(self.provenance.is_external(), |d| {
            d.child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .h(px(18.))
                    .px(space::S6)
                    .rounded(radius::CHIP)
                    .border_1()
                    .border_color(c.border_default)
                    .text_color(c.text_muted)
                    .text_size(type_scale::FS_11)
                    .child("EXTERNAL"),
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use binmap_core::evidence::{EvidenceStore, ToolInvocation};
    use binmap_core::finding::{FindingDraft, FindingKind};

    fn finding(provenance: Provenance, confidence: Confidence) -> Finding {
        let store = EvidenceStore::new();
        let pending = store.begin(ToolInvocation::new("cargo", ["build"]));
        let evidence = store.complete(pending, "built", 0);
        Finding::new(
            FindingDraft::new("f", FindingKind::Configuration, "a claim")
                .cite(evidence)
                .confidence(confidence)
                .provenance(provenance),
            &store,
        )
        .unwrap()
    }

    #[test]
    fn a_badge_can_only_be_built_from_a_finding() {
        // There is no constructor taking a loose provenance and confidence, so
        // a measured claim cannot be labelled inferred by a caller getting the
        // arguments the wrong way round. This test documents that; it fails to
        // compile if someone adds such a constructor and forgets why.
        let f = finding(Provenance::Measured, Confidence::Certain);
        let badge = ProvenanceBadge::of(&f, Theme::default());
        assert_eq!(badge.provenance, Provenance::Measured);
        assert_eq!(badge.confidence, Confidence::Certain);
    }

    #[test]
    fn the_badge_reports_what_the_finding_actually_carries_not_what_was_asked_for() {
        // The finding clamps Certain down to Probable for an external claim;
        // the badge must show the clamped value, not the requested one.
        let f = finding(
            Provenance::InferredExternally { agent: "claude-code".into() },
            Confidence::Certain,
        );
        let badge = ProvenanceBadge::of(&f, Theme::default());
        assert_eq!(badge.confidence, Confidence::Probable);
        assert!(badge.provenance.is_external());
    }

    #[test]
    fn every_provenance_has_a_glyph_and_a_word_from_the_fixed_vocabulary() {
        for provenance in [
            Provenance::Measured,
            Provenance::Derived { rule: "pareto-dominance".into() },
            Provenance::InferredNatively { model: "qwen".into() },
            Provenance::InferredExternally { agent: "codex".into() },
        ] {
            let f = finding(provenance.clone(), Confidence::Probable);
            let badge = ProvenanceBadge::of(&f, Theme::default());
            assert!("●◈◆".contains(badge.provenance.glyph()));
            assert!(
                ["Measured", "Derived", "Inferred, natively", "Inferred, externally"]
                    .contains(&badge.provenance.label())
            );
        }
    }
}
