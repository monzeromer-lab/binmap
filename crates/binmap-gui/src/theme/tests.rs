use super::*;

/// Relative luminance, WCAG 2.x. Used to check the two themes are genuinely
/// different rather than one being a tinted copy of the other.
fn luminance(colour: Rgba) -> f32 {
    let channel = |c: f32| if c <= 0.03928 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) };
    0.2126 * channel(colour.r) + 0.7152 * channel(colour.g) + 0.0722 * channel(colour.b)
}

fn contrast(a: Rgba, b: Rgba) -> f32 {
    let (x, y) = (luminance(a), luminance(b));
    let (lighter, darker) = if x > y { (x, y) } else { (y, x) };
    (lighter + 0.05) / (darker + 0.05)
}

#[test]
fn dark_is_the_default_because_u11_says_so() {
    assert_eq!(Theme::default().appearance, Appearance::Dark);
    assert!(Theme::default().is_dark());
}

#[test]
fn the_two_themes_are_genuinely_opposite_not_one_tinted() {
    let dark = Colours::dark();
    let light = Colours::light();
    assert!(
        luminance(dark.surface_app) < 0.05,
        "the dark app surface should be dark"
    );
    assert!(
        luminance(light.surface_app) > 0.8,
        "the light app surface should be light"
    );
    // And text follows the surface rather than staying put.
    assert!(luminance(dark.text_primary) > luminance(dark.surface_app));
    assert!(luminance(light.text_primary) < luminance(light.surface_app));
}

#[test]
fn body_text_is_readable_on_its_own_surface_in_both_themes() {
    // 4.5:1 is WCAG AA for body text. A measurement instrument whose numbers
    // are hard to read is a worse instrument.
    for (name, colours) in [("dark", Colours::dark()), ("light", Colours::light())] {
        let ratio = contrast(colours.text_body, colours.surface_panel);
        assert!(ratio >= 4.5, "{name}: body text on panel is {ratio:.2}:1, below AA");

        let primary = contrast(colours.text_primary, colours.surface_app);
        assert!(primary >= 4.5, "{name}: primary text on app is {primary:.2}:1, below AA");
    }
}

#[test]
fn the_three_provenances_are_told_apart_by_hue_not_only_by_glyph() {
    // DESIGN §7.1 calls this the most load-bearing colour decision in the
    // product, so no two provenances may share a colour.
    for (name, colours) in [("dark", Colours::dark()), ("light", Colours::light())] {
        let measured = colours.provenance(&Provenance::Measured).0;
        let derived = colours.provenance(&Provenance::Derived { rule: "r".into() }).0;
        let inferred =
            colours.provenance(&Provenance::InferredNatively { model: "m".into() }).0;

        assert_ne!(measured, derived, "{name}: measured and derived share a colour");
        assert_ne!(derived, inferred, "{name}: derived and inferred share a colour");
        assert_ne!(measured, inferred, "{name}: measured and inferred share a colour");
    }
}

#[test]
fn an_external_claim_is_drawn_like_an_internal_one_and_distinguished_by_its_badge() {
    // Both are Inferred, so both carry the inferred hue. The external-origin
    // distinction is a badge (AI.10), not a colour — colouring it differently
    // would imply a different kind of claim rather than a weaker guarantee.
    let colours = Colours::dark();
    let native = colours.provenance(&Provenance::InferredNatively { model: "m".into() }).0;
    let external =
        colours.provenance(&Provenance::InferredExternally { agent: "a".into() }).0;
    assert_eq!(native, external);
}

#[test]
fn a_result_inside_the_noise_floor_is_drawn_flat_not_green_and_not_red() {
    // §6's rule, expressed where it can be broken: the delta colour for an
    // inconclusive result is neither improvement nor regression.
    let colours = Colours::dark();
    assert_eq!(colours.delta(None), colours.delta_flat);
    assert_ne!(colours.delta(None), colours.delta_improve);
    assert_ne!(colours.delta(None), colours.delta_regress);
}

#[test]
fn every_tier_is_visually_distinct_so_the_dial_is_readable_at_a_glance() {
    use binmap_core::config::TrustTier;
    for (name, colours) in [("dark", Colours::dark()), ("light", Colours::light())] {
        let tiers: Vec<Rgba> = TrustTier::ALL.iter().map(|t| colours.tier(*t)).collect();
        for (i, a) in tiers.iter().enumerate() {
            for b in tiers.iter().skip(i + 1) {
                assert_ne!(a, b, "{name}: two tiers share a colour");
            }
        }
    }
}

#[test]
fn confidence_reads_as_measurement_at_the_top_and_as_claim_at_the_bottom() {
    let colours = Colours::dark();
    // Certain and High are the green end; Probable and Speculative the orange.
    assert_eq!(colours.confidence(Confidence::Certain), colours.prov_measured);
    assert_ne!(
        colours.confidence(Confidence::Certain),
        colours.confidence(Confidence::Speculative)
    );
}

#[test]
fn the_categorical_palette_has_a_stable_order() {
    // Treemap and flamegraph colours are indexed by category, never hashed:
    // a crate must not change colour because another one was added.
    assert_eq!(palette::CATEGORICAL.len(), 8);
    assert_eq!(palette::CATEGORICAL[0], palette::ORANGE_500, "yours is the brand orange");
    let unique: std::collections::BTreeSet<u32> = palette::CATEGORICAL.into_iter().collect();
    assert_eq!(unique.len(), palette::CATEGORICAL.len(), "two categories share a colour");
}

#[test]
fn toggling_twice_returns_the_theme_it_started_from() {
    let theme = Theme::default();
    assert_eq!(theme.toggled().toggled().appearance, theme.appearance);
    assert_eq!(theme.toggled().appearance, Appearance::Light);
}

#[test]
fn the_frame_dimensions_are_the_designs() {
    assert_eq!(space::TITLEBAR_H, px(40.));
    assert_eq!(space::STATUSBAR_H, px(28.));
    assert_eq!(space::NAVRAIL_W, px(72.));
    assert_eq!(space::INSPECTOR_W, px(340.));
    assert_eq!(space::ROW_H, px(28.));
}
