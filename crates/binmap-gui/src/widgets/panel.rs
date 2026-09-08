//! Panels and section headings — the boxes everything else sits in.

use crate::theme::{Theme, radius, space, type_scale};
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, App, SharedString, Window, div, px};

/// A titled panel with a hairline border.
///
/// Elevation in dark is surface value plus a hairline rather than blur, so
/// this deliberately has no shadow: shadows belong to things that float.
#[derive(IntoElement)]
pub struct Section {
    title: Option<SharedString>,
    icon: Option<SharedString>,
    /// Shown at the trailing end of the header — a count, a badge, an action.
    trailing: Option<AnyElement>,
    body: Vec<AnyElement>,
    theme: Theme,
    padded: bool,
}

impl Section {
    pub fn new(theme: Theme) -> Self {
        Self { title: None, icon: None, trailing: None, body: Vec::new(), theme, padded: true }
    }

    pub fn titled(title: impl Into<SharedString>, theme: Theme) -> Self {
        Self::new(theme).with_title(title)
    }

    pub fn with_title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn with_icon(mut self, icon: impl Into<SharedString>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    pub fn with_trailing(mut self, trailing: impl IntoElement) -> Self {
        self.trailing = Some(trailing.into_any_element());
        self
    }

    /// Turn off the body padding, for a panel whose content is a table that
    /// should meet the border.
    pub fn flush(mut self) -> Self {
        self.padded = false;
        self
    }
}

impl ParentElement for Section {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.body.extend(elements);
    }
}

impl RenderOnce for Section {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let c = self.theme.colours;
        let has_header = self.title.is_some();

        div()
            .flex()
            .flex_col()
            .rounded(radius::PANEL)
            .bg(c.surface_panel)
            .border_1()
            .border_color(c.border_subtle)
            .when_some(self.title, |d, title| {
                d.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(space::S8)
                        .px(space::S12)
                        .h(px(34.))
                        .flex_none()
                        .border_b_1()
                        .border_color(c.border_subtle)
                        .child(
                            div()
                                .flex_1()
                                .text_size(type_scale::FS_12)
                                .text_color(c.text_secondary)
                                .child(title),
                        )
                        .when_some(self.trailing, |d, trailing| d.child(trailing)),
                )
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .when(self.padded, |d| d.p(space::S12).gap(space::S8))
                    .when(!has_header && self.padded, |d| d.pt(space::S12))
                    .children(self.body),
            )
    }
}

/// A tracked-out uppercase label — the design's section eyebrow.
pub fn eyebrow(text: impl Into<SharedString>, theme: Theme) -> impl IntoElement {
    // The design tracks eyebrows out by 0.08em. GPUI's Div has no
    // letter-spacing, so the caps and the muted colour carry the distinction
    // instead; spacing out the glyphs by hand would be worse than not.
    div()
        .text_size(type_scale::FS_11)
        .text_color(theme.colours.text_muted)
        .child(text.into().to_uppercase())
}
