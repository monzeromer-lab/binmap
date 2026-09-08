//! Small status chips: the thing that appears beside a row to say how it went.

use crate::theme::{Theme, radius, space, type_scale};
use gpui_kit::prelude::*;
use gpui_kit::{App, Div, SharedString, Window, div, px};

/// What a badge is saying. The tones are the design's status set; there is no
/// "just make it grey" escape hatch, because a badge with no meaning is noise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Pass,
    Fail,
    Warn,
    Info,
    Neutral,
    Accent,
}

/// A compact label with a background, as used for gate results, target
/// capabilities, counts and window-level state.
#[derive(IntoElement)]
pub struct Badge {
    label: SharedString,
    tone: Tone,
    theme: Theme,
    /// Monospaced, for anything a user might compare character by character:
    /// counts, digests, versions.
    mono: bool,
    /// Uppercased and tracked out, as the design renders gate results.
    caps: bool,
}

impl Badge {
    pub fn new(label: impl Into<SharedString>, tone: Tone, theme: Theme) -> Self {
        Self { label: label.into(), tone, theme, mono: false, caps: false }
    }

    pub fn mono(mut self) -> Self {
        self.mono = true;
        self
    }

    pub fn caps(mut self) -> Self {
        self.caps = true;
        self
    }
}

impl RenderOnce for Badge {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let c = self.theme.colours;
        let (foreground, background) = match self.tone {
            Tone::Pass => (c.status_pass, c.status_pass_bg),
            Tone::Fail => (c.status_fail, c.status_fail_bg),
            Tone::Warn => (c.status_warn, c.status_warn_bg),
            Tone::Info => (c.status_info, c.status_info_bg),
            Tone::Neutral => (c.text_muted, c.surface_raised),
            Tone::Accent => (c.accent, c.surface_raised),
        };

        let label = if self.caps { self.label.to_uppercase() } else { self.label.to_string() };

        div()
            .flex()
            .flex_none()
            .items_center()
            .h(px(18.))
            .px(space::S6)
            .rounded(radius::CHIP)
            .bg(background)
            .text_color(foreground)
            .text_size(type_scale::FS_11)
            .when(self.mono || self.caps, |d| d.font_family("JetBrains Mono"))
            .child(label)
    }
}

/// A one-line key/value row, as the workspace facts table uses.
pub fn fact(key: impl Into<SharedString>, value: impl Into<SharedString>, theme: Theme) -> Div {
    let c = theme.colours;
    div()
        .flex()
        .flex_row()
        .gap(space::S12)
        .py(space::S2)
        .child(
            div()
                .w(px(84.))
                .flex_none()
                .text_size(type_scale::FS_12)
                .text_color(c.text_muted)
                .child(key.into()),
        )
        .child(
            div()
                .flex_1()
                .font_family("JetBrains Mono")
                .text_size(type_scale::FS_12)
                .text_color(c.text_body)
                .child(value.into()),
        )
}
