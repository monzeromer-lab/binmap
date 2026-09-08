//! The design system, in Rust.
//!
//! Ported from the token files the design is built on — `colors.css`,
//! `semantic.css`, `typography.css`, `spacing.css`, `radius.css` — rather than
//! eyeballed from screenshots. Every value here has a counterpart there, and
//! the token name is kept so the two can be diffed by a person.
//!
//! Three rules the tokens encode, worth stating because they are easy to
//! violate later:
//!
//! - **Dark is the default and light is real, not an afterthought** (`U11`).
//!   Both palettes are defined in full; nothing computes one from the other.
//! - **Provenance colour is load-bearing.** `DESIGN.md` §7.1 calls it the most
//!   important colour decision in the product, so measured, derived and
//!   inferred each get their own hue and they are never reused for anything
//!   else.
//! - **Elevation in dark is surface value plus a hairline, not blur.** Shadows
//!   appear only on things that genuinely float.

use binmap_core::finding::{Confidence, Provenance};
use gpui_kit::{Hsla, Pixels, Rgba, px, rgb, rgba};

/// Which palette is in force.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Appearance {
    #[default]
    Dark,
    Light,
}

impl Appearance {
    pub fn label(self) -> &'static str {
        match self {
            Appearance::Dark => "Dark",
            Appearance::Light => "Light",
        }
    }

    pub fn toggled(self) -> Self {
        match self {
            Appearance::Dark => Appearance::Light,
            Appearance::Light => Appearance::Dark,
        }
    }
}

/// The base palette, sampled from the SilverKey mark: slate #3D4658 and
/// orange #FF6F15. Shared by both themes — only the semantic aliases change.
pub mod palette {
    use gpui_kit::{Rgba, rgb};

    pub const SLATE_950: u32 = 0x0E1116;
    pub const SLATE_900: u32 = 0x14181F;
    pub const SLATE_850: u32 = 0x1A1F27;
    pub const SLATE_800: u32 = 0x222834;
    pub const SLATE_750: u32 = 0x282F3C;
    pub const SLATE_700: u32 = 0x2F3745;
    pub const SLATE_650: u32 = 0x363F4F;
    /// The brand slate — the logo wordmark.
    pub const SLATE_600: u32 = 0x3D4658;
    pub const SLATE_500: u32 = 0x4E596F;
    pub const SLATE_400: u32 = 0x6E7891;
    pub const SLATE_350: u32 = 0x828CA3;
    pub const SLATE_300: u32 = 0x98A0B3;
    pub const SLATE_200: u32 = 0xC2C8D4;
    pub const SLATE_100: u32 = 0xE1E5EC;
    pub const SLATE_050: u32 = 0xF2F4F7;
    pub const WHITE: u32 = 0xFFFFFF;

    pub const ORANGE_800: u32 = 0x7A3105;
    pub const ORANGE_700: u32 = 0xB24A08;
    pub const ORANGE_600: u32 = 0xD95C0C;
    /// The brand orange — the logo chevron. Used sparingly and always with
    /// intent.
    pub const ORANGE_500: u32 = 0xFF6F15;
    pub const ORANGE_400: u32 = 0xFF8A42;
    pub const ORANGE_300: u32 = 0xFFA870;
    pub const ORANGE_200: u32 = 0xFFC9A3;
    pub const ORANGE_100: u32 = 0xFFE7D6;
    pub const ORANGE_050: u32 = 0xFFF4EC;

    // Semantic hues, desaturated to sit beside slate without shouting.
    pub const GREEN_600: u32 = 0x2E8C69;
    pub const GREEN_500: u32 = 0x46C08D;
    pub const GREEN_300: u32 = 0x8ADCBB;
    pub const GREEN_TINT: u32 = 0x12251E;

    pub const RED_600: u32 = 0xB33C41;
    pub const RED_500: u32 = 0xE5565B;
    pub const RED_300: u32 = 0xF09499;
    pub const RED_TINT: u32 = 0x2A1417;

    pub const AMBER_600: u32 = 0xB3801F;
    pub const AMBER_500: u32 = 0xE5A93C;
    pub const AMBER_300: u32 = 0xF2CD8A;
    pub const AMBER_TINT: u32 = 0x291F0F;

    pub const BLUE_600: u32 = 0x3F6D96;
    pub const BLUE_500: u32 = 0x5B8FB9;
    pub const BLUE_300: u32 = 0x9CC0DA;
    pub const BLUE_TINT: u32 = 0x111C26;

    /// Categorical palette — treemap regions, flamegraph groups, sweep series.
    /// Ordered as the design lists them; index by category, never by hash.
    pub const CATEGORICAL: [u32; 8] = [
        0xFF6F15, // yours
        0x5B8FB9, // deps
        0x46C08D, // formatting machinery
        0xE5565B, // panic
        0xA98BD1, // unwinding
        0xE5A93C, // Drop glue
        0x5FBFB4, // vtables
        0x8E9AB5, // static data
    ];

    pub fn colour(value: u32) -> Rgba {
        rgb(value)
    }
}

use palette as p;

/// Every semantic colour the interface uses.
///
/// Named after the CSS custom properties one-for-one, so a change in the
/// design system can be applied here without translation.
#[derive(Debug, Clone, Copy)]
pub struct Colours {
    // Surfaces
    pub surface_app: Rgba,
    pub surface_panel: Rgba,
    pub surface_raised: Rgba,
    pub surface_sunken: Rgba,
    pub surface_input: Rgba,
    pub surface_hover: Rgba,
    pub surface_active: Rgba,
    pub surface_selected: Rgba,
    pub surface_canvas: Rgba,
    pub surface_overlay: Rgba,
    pub surface_code: Rgba,

    // Text
    pub text_primary: Rgba,
    pub text_body: Rgba,
    pub text_secondary: Rgba,
    pub text_muted: Rgba,
    pub text_disabled: Rgba,
    pub text_inverse: Rgba,
    pub text_accent: Rgba,

    // Borders
    pub border_subtle: Rgba,
    pub border_default: Rgba,
    pub border_strong: Rgba,
    pub border_accent: Rgba,
    pub divider: Rgba,

    // Accent and actions
    pub accent: Rgba,
    pub accent_hover: Rgba,
    pub accent_press: Rgba,
    pub accent_quiet: Hsla,
    pub on_accent: Rgba,
    pub focus_ring: Rgba,

    // Status
    pub status_pass: Rgba,
    pub status_pass_bg: Rgba,
    pub status_fail: Rgba,
    pub status_fail_bg: Rgba,
    pub status_warn: Rgba,
    pub status_warn_bg: Rgba,
    pub status_info: Rgba,
    pub status_info_bg: Rgba,
    pub status_neutral: Rgba,
    pub status_running: Rgba,

    // Provenance — DESIGN §7.1, the most load-bearing colour decision here.
    pub prov_measured: Rgba,
    pub prov_measured_bg: Rgba,
    pub prov_derived: Rgba,
    pub prov_derived_bg: Rgba,
    pub prov_inferred: Rgba,
    pub prov_inferred_bg: Hsla,

    // Confidence — Certain and High read as measurement, Probable and
    // Speculative as claim.
    pub confidence_certain: Rgba,
    pub confidence_high: Rgba,
    pub confidence_probable: Rgba,
    pub confidence_speculative: Rgba,

    // Metric deltas
    pub delta_improve: Rgba,
    pub delta_regress: Rgba,
    pub delta_flat: Rgba,

    // Trust tiers
    pub tier_observe: Rgba,
    pub tier_propose: Rgba,
    pub tier_tune: Rgba,
    pub tier_autonomous: Rgba,
}

impl Colours {
    pub fn dark() -> Self {
        Self {
            surface_app: rgb(p::SLATE_950),
            surface_panel: rgb(p::SLATE_900),
            surface_raised: rgb(p::SLATE_850),
            surface_sunken: rgb(0x0A0D11),
            surface_input: rgb(p::SLATE_850),
            surface_hover: rgb(p::SLATE_800),
            surface_active: rgb(p::SLATE_750),
            surface_selected: rgb(0x1E2733),
            surface_canvas: rgb(0x0B0E13),
            surface_overlay: rgb(p::SLATE_850),
            surface_code: rgb(0x0C0F14),

            text_primary: rgb(p::SLATE_050),
            text_body: rgb(p::SLATE_200),
            text_secondary: rgb(p::SLATE_300),
            text_muted: rgb(p::SLATE_400),
            text_disabled: rgb(p::SLATE_500),
            text_inverse: rgb(p::SLATE_950),
            text_accent: rgb(p::ORANGE_400),

            border_subtle: rgb(p::SLATE_800),
            border_default: rgb(p::SLATE_700),
            border_strong: rgb(p::SLATE_600),
            border_accent: rgb(p::ORANGE_500),
            divider: rgb(p::SLATE_800),

            accent: rgb(p::ORANGE_500),
            accent_hover: rgb(p::ORANGE_400),
            accent_press: rgb(p::ORANGE_600),
            accent_quiet: rgba(0xFF6F1524).into(),
            on_accent: rgb(0x1A0C02),
            focus_ring: rgb(p::ORANGE_400),

            status_pass: rgb(p::GREEN_500),
            status_pass_bg: rgb(p::GREEN_TINT),
            status_fail: rgb(p::RED_500),
            status_fail_bg: rgb(p::RED_TINT),
            status_warn: rgb(p::AMBER_500),
            status_warn_bg: rgb(p::AMBER_TINT),
            status_info: rgb(p::BLUE_500),
            status_info_bg: rgb(p::BLUE_TINT),
            status_neutral: rgb(p::SLATE_400),
            status_running: rgb(p::ORANGE_400),

            prov_measured: rgb(p::GREEN_500),
            prov_measured_bg: rgb(p::GREEN_TINT),
            prov_derived: rgb(p::SLATE_300),
            prov_derived_bg: rgb(p::SLATE_800),
            prov_inferred: rgb(p::ORANGE_500),
            prov_inferred_bg: rgba(0xFF6F151F).into(),

            confidence_certain: rgb(p::GREEN_500),
            confidence_high: rgb(p::GREEN_300),
            confidence_probable: rgb(p::ORANGE_400),
            confidence_speculative: rgb(p::ORANGE_200),

            delta_improve: rgb(p::GREEN_500),
            delta_regress: rgb(p::RED_500),
            delta_flat: rgb(p::SLATE_400),

            tier_observe: rgb(p::SLATE_300),
            tier_propose: rgb(p::BLUE_500),
            tier_tune: rgb(p::AMBER_500),
            tier_autonomous: rgb(p::RED_500),
        }
    }

    /// The light theme, defined in full rather than derived.
    ///
    /// Deriving it from the dark one is how light themes end up looking like
    /// an inverted photograph; the design specifies every alias separately and
    /// so does this.
    pub fn light() -> Self {
        Self {
            surface_app: rgb(p::SLATE_050),
            surface_panel: rgb(p::WHITE),
            surface_raised: rgb(p::WHITE),
            surface_sunken: rgb(p::SLATE_100),
            surface_input: rgb(p::WHITE),
            surface_hover: rgb(p::SLATE_050),
            surface_active: rgb(p::SLATE_100),
            surface_selected: rgb(p::ORANGE_050),
            surface_canvas: rgb(p::WHITE),
            surface_overlay: rgb(p::WHITE),
            surface_code: rgb(p::SLATE_050),

            text_primary: rgb(p::SLATE_900),
            text_body: rgb(p::SLATE_600),
            text_secondary: rgb(p::SLATE_500),
            text_muted: rgb(p::SLATE_400),
            text_disabled: rgb(p::SLATE_300),
            text_inverse: rgb(p::WHITE),
            text_accent: rgb(p::ORANGE_700),

            border_subtle: rgb(p::SLATE_100),
            border_default: rgb(p::SLATE_200),
            border_strong: rgb(p::SLATE_300),
            border_accent: rgb(p::ORANGE_500),
            divider: rgb(p::SLATE_100),

            accent: rgb(p::ORANGE_500),
            accent_hover: rgb(p::ORANGE_600),
            accent_press: rgb(p::ORANGE_700),
            accent_quiet: rgba(0xFF6F1518).into(),
            on_accent: rgb(p::WHITE),
            focus_ring: rgb(p::ORANGE_600),

            status_pass: rgb(p::GREEN_600),
            status_pass_bg: rgb(0xE8F6F0),
            status_fail: rgb(p::RED_600),
            status_fail_bg: rgb(0xFCECEC),
            status_warn: rgb(p::AMBER_600),
            status_warn_bg: rgb(0xFDF4E3),
            status_info: rgb(p::BLUE_600),
            status_info_bg: rgb(0xEBF2F8),
            status_neutral: rgb(p::SLATE_400),
            status_running: rgb(p::ORANGE_600),

            prov_measured: rgb(p::GREEN_600),
            prov_measured_bg: rgb(0xE8F6F0),
            prov_derived: rgb(p::SLATE_500),
            prov_derived_bg: rgb(p::SLATE_100),
            prov_inferred: rgb(p::ORANGE_600),
            prov_inferred_bg: rgba(0xFF6F1516).into(),

            confidence_certain: rgb(p::GREEN_600),
            confidence_high: rgb(p::GREEN_500),
            confidence_probable: rgb(p::ORANGE_600),
            confidence_speculative: rgb(p::ORANGE_400),

            delta_improve: rgb(p::GREEN_600),
            delta_regress: rgb(p::RED_600),
            delta_flat: rgb(p::SLATE_400),

            tier_observe: rgb(p::SLATE_500),
            tier_propose: rgb(p::BLUE_600),
            tier_tune: rgb(p::AMBER_600),
            tier_autonomous: rgb(p::RED_600),
        }
    }

    /// The colour a provenance badge is drawn in. There is one place this is
    /// decided, so a badge cannot be drawn in the wrong hue by accident.
    pub fn provenance(&self, provenance: &Provenance) -> (Rgba, Hsla) {
        match provenance {
            Provenance::Measured => (self.prov_measured, self.prov_measured_bg.into()),
            Provenance::Derived { .. } => (self.prov_derived, self.prov_derived_bg.into()),
            Provenance::InferredNatively { .. } | Provenance::InferredExternally { .. } => {
                (self.prov_inferred, self.prov_inferred_bg)
            }
        }
    }

    pub fn confidence(&self, confidence: Confidence) -> Rgba {
        match confidence {
            Confidence::Certain => self.confidence_certain,
            Confidence::High => self.confidence_high,
            Confidence::Probable => self.confidence_probable,
            Confidence::Speculative => self.confidence_speculative,
        }
    }

    pub fn tier(&self, tier: binmap_core::config::TrustTier) -> Rgba {
        use binmap_core::config::TrustTier;
        match tier {
            TrustTier::Observe => self.tier_observe,
            TrustTier::Propose => self.tier_propose,
            TrustTier::Tune => self.tier_tune,
            TrustTier::Autonomous => self.tier_autonomous,
        }
    }

    /// Green for an improvement, red for a regression, neutral for no change.
    ///
    /// A result inside the noise floor is `None` and must be drawn in
    /// `delta_flat` — colouring it either way is the thing §6 forbids.
    pub fn delta(&self, improvement: Option<bool>) -> Rgba {
        match improvement {
            Some(true) => self.delta_improve,
            Some(false) => self.delta_regress,
            None => self.delta_flat,
        }
    }
}

/// The 4px grid, and the fixed dimensions of the application frame.
pub mod space {
    use gpui_kit::{Pixels, px};

    pub const S2: Pixels = px(2.);
    pub const S4: Pixels = px(4.);
    pub const S6: Pixels = px(6.);
    pub const S8: Pixels = px(8.);
    pub const S10: Pixels = px(10.);
    pub const S12: Pixels = px(12.);
    pub const S16: Pixels = px(16.);
    pub const S20: Pixels = px(20.);
    pub const S24: Pixels = px(24.);
    pub const S32: Pixels = px(32.);
    pub const S40: Pixels = px(40.);

    /// The frame. These are fixed by the design, not chosen per view.
    pub const TITLEBAR_H: Pixels = px(40.);
    pub const STATUSBAR_H: Pixels = px(28.);
    pub const NAVRAIL_W: Pixels = px(72.);
    pub const INSPECTOR_W: Pixels = px(340.);
    pub const GUTTER: Pixels = px(12.);

    pub const CONTROL_H_SM: Pixels = px(24.);
    pub const CONTROL_H_MD: Pixels = px(30.);
    pub const CONTROL_H_LG: Pixels = px(36.);
    /// A dense data-table row.
    pub const ROW_H: Pixels = px(28.);
    pub const ROW_H_LG: Pixels = px(34.);
}

/// Corner radii. Each has one job; picking by eye is how a design system
/// stops being one.
pub mod radius {
    use gpui_kit::{Pixels, px};

    /// Provenance markers and inline chips.
    pub const CHIP: Pixels = px(2.);
    /// Inputs and table selection.
    pub const INPUT: Pixels = px(3.);
    /// Buttons and tags — the default control radius.
    pub const CONTROL: Pixels = px(5.);
    /// Panels, cards and dialogs.
    pub const PANEL: Pixels = px(8.);
    /// Dialog and palette shells.
    pub const SHELL: Pixels = px(12.);
}

/// The type scale. 13px is the UI base and nothing goes below 11px.
pub mod type_scale {
    use gpui_kit::{Pixels, px};

    pub const FS_11: Pixels = px(11.);
    pub const FS_12: Pixels = px(12.);
    /// The UI base.
    pub const FS_13: Pixels = px(13.);
    pub const FS_14: Pixels = px(14.);
    pub const FS_16: Pixels = px(16.);
    pub const FS_18: Pixels = px(18.);
    pub const FS_22: Pixels = px(22.);
    pub const FS_28: Pixels = px(28.);

    /// Section eyebrows and table headers are tracked out.
    pub const TRACKING_CAPS: f32 = 0.08;
}

/// Everything a view needs to draw itself consistently.
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub appearance: Appearance,
    pub colours: Colours,
}

impl Default for Theme {
    fn default() -> Self {
        // Dark by default, per U11.
        Self::of(Appearance::Dark)
    }
}

impl Theme {
    pub fn of(appearance: Appearance) -> Self {
        let colours = match appearance {
            Appearance::Dark => Colours::dark(),
            Appearance::Light => Colours::light(),
        };
        Self { appearance, colours }
    }

    pub fn toggled(self) -> Self {
        Self::of(self.appearance.toggled())
    }

    pub fn is_dark(&self) -> bool {
        self.appearance == Appearance::Dark
    }

    /// The hairline that carries elevation in dark. In light, a border does
    /// the same job at a different value — which is why this asks the theme
    /// rather than hard-coding one.
    pub fn hairline(&self) -> (Pixels, Rgba) {
        (px(1.), self.colours.border_subtle)
    }
}

#[cfg(test)]
mod tests;
