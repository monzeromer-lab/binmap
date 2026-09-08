//! The command palette (`U12`) and the trust-tier dialog (`U9`).
//!
//! Both are overlays, and both are lists of [`Action`](crate::state::Action) —
//! which is the point. `U12` is a Must rather than a Should because keyboard
//! access is the only accommodation left for users who live in terminals, and
//! that is most of the audience. Building the palette out of the same values
//! clicks produce is what makes "keyboard reaches every action" a property of
//! the design rather than a promise to keep re-checking.

use crate::dispatch::{Dispatch, clickable};
use crate::state::{Action, Command};
use crate::theme::{Theme, radius, space, type_scale};
use crate::widgets::eyebrow;
use binmap_core::config::TrustTier;
use gpui_kit::prelude::*;
use gpui_kit::{App, SharedString, Window, div, px};

/// A dimmed backdrop that closes what it is behind.
fn scrim(dispatch: &Dispatch, action: Action) -> gpui_kit::Stateful<gpui_kit::Div> {
    clickable(div().id("scrim"), dispatch, action)
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .bg(gpui_kit::rgba(0x080A0EAE))
        .flex()
        .flex_col()
        .items_center()
}

#[derive(IntoElement)]
pub struct Palette {
    commands: Vec<Command>,
    query: String,
    theme: Theme,
    dispatch: Dispatch,
}

impl Palette {
    pub fn new(commands: Vec<Command>, query: String, theme: Theme, dispatch: &Dispatch) -> Self {
        Self { commands, query, theme, dispatch: std::rc::Rc::clone(dispatch) }
    }
}

impl RenderOnce for Palette {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let c = self.theme.colours;
        let theme = self.theme;
        let dispatch = self.dispatch;
        let rows = std::rc::Rc::clone(&dispatch);

        // Group headings, in the order the commands arrive.
        let mut groups: Vec<(&'static str, Vec<Command>)> = Vec::new();
        for command in self.commands {
            match groups.iter_mut().find(|(name, _)| *name == command.group) {
                Some((_, entries)) => entries.push(command),
                None => groups.push((command.group, vec![command])),
            }
        }

        scrim(&dispatch, Action::ClosePalette).child(
            div()
                .mt(px(96.))
                .w(px(560.))
                .max_h(px(460.))
                .flex()
                .flex_col()
                .rounded(radius::SHELL)
                .bg(c.surface_overlay)
                .border_1()
                .border_color(c.border_default)
                .overflow_hidden()
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(space::S8)
                        .flex_none()
                        .h(px(44.))
                        .px(space::S16)
                        .border_b_1()
                        .border_color(c.border_subtle)
                        .child(div().text_color(c.text_muted).child("⌕"))
                        .child(
                            div()
                                .flex_1()
                                .text_size(type_scale::FS_14)
                                .text_color(if self.query.is_empty() {
                                    c.text_muted
                                } else {
                                    c.text_primary
                                })
                                .child(if self.query.is_empty() {
                                    "Search analyses, targets and session actions".to_string()
                                } else {
                                    self.query.clone()
                                }),
                        )
                        .child(
                            div()
                                .font_family("JetBrains Mono")
                                .text_size(type_scale::FS_11)
                                .text_color(c.text_disabled)
                                .child("Esc"),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_h_0()
                        .overflow_hidden()
                        .py(space::S4)
                        .children(groups.into_iter().map(move |(name, entries)| {
                            let rows = std::rc::Rc::clone(&rows);
                            div()
                                .flex()
                                .flex_col()
                                .child(
                                    div().px(space::S16).py(space::S4).child(eyebrow(name, theme)),
                                )
                                .children(entries.into_iter().map(move |command| {
                                    clickable(
                                        div().id(SharedString::from(format!(
                                            "cmd-{}",
                                            command.label
                                        ))),
                                        &rows,
                                        command.action.clone(),
                                    )
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(space::S8)
                                    .h(space::ROW_H_LG)
                                    .px(space::S16)
                                    .hover(|d| d.bg(c.surface_hover))
                                    .child(
                                        div()
                                            .flex_1()
                                            .text_size(type_scale::FS_13)
                                            .text_color(c.text_body)
                                            .child(command.label.clone()),
                                    )
                                    .when_some(
                                        command.shortcut,
                                        |d, shortcut| {
                                            d.child(
                                                div()
                                                    .font_family("JetBrains Mono")
                                                    .text_size(type_scale::FS_11)
                                                    .text_color(c.text_disabled)
                                                    .child(shortcut),
                                            )
                                        },
                                    )
                                }))
                        })),
                ),
        )
    }
}

/// The trust dial, and what each setting permits.
///
/// One dial with four settings, always visible and never raised silently. The
/// dialog states each tier's effect in its own words rather than a permission
/// bit, because a user deciding whether to raise a tier needs a sentence.
#[derive(IntoElement)]
pub struct TierDialog {
    current: TrustTier,
    theme: Theme,
    dispatch: Dispatch,
}

impl TierDialog {
    pub fn new(current: TrustTier, theme: Theme, dispatch: &Dispatch) -> Self {
        Self { current, theme, dispatch: std::rc::Rc::clone(dispatch) }
    }
}

impl RenderOnce for TierDialog {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let c = self.theme.colours;
        let current = self.current;
        let dispatch = self.dispatch;
        let rows = std::rc::Rc::clone(&dispatch);

        scrim(&dispatch, Action::CloseDialogs).child(
            div()
                .mt(px(120.))
                .w(px(520.))
                .flex()
                .flex_col()
                .rounded(radius::SHELL)
                .bg(c.surface_overlay)
                .border_1()
                .border_color(c.border_default)
                .overflow_hidden()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(space::S4)
                        .p(space::S16)
                        .border_b_1()
                        .border_color(c.border_subtle)
                        .child(
                            div()
                                .text_size(type_scale::FS_16)
                                .text_color(c.text_primary)
                                .child("Change trust tier"),
                        )
                        .child(div().text_size(type_scale::FS_12).text_color(c.text_muted).child(
                            "One dial with four settings, always visible and never raised \
                             silently.",
                        )),
                )
                .child(div().flex().flex_col().p(space::S8).children(
                    TrustTier::ALL.into_iter().map(move |tier| {
                        let active = tier == current;
                        clickable(
                            div().id(SharedString::from(format!("tier-{}", tier.label()))),
                            &rows,
                            Action::SetTier(tier),
                        )
                        .flex()
                        .flex_col()
                        .gap(space::S2)
                        .p(space::S10)
                        .rounded(radius::INPUT)
                        .when(active, |d| d.bg(c.surface_selected))
                        .when(!active, |d| d.hover(|d| d.bg(c.surface_hover)))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(space::S8)
                                .child(
                                    div()
                                        .w(px(8.))
                                        .h(px(8.))
                                        .rounded(radius::CHIP)
                                        .bg(c.tier(tier)),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .text_size(type_scale::FS_13)
                                        .text_color(c.text_primary)
                                        .child(tier.label()),
                                )
                                .when(active, |d| {
                                    d.child(
                                        div()
                                            .text_size(type_scale::FS_11)
                                            .text_color(c.text_muted)
                                            .child("current"),
                                    )
                                }),
                        )
                        .child(
                            div()
                                .pl(px(16.))
                                .text_size(type_scale::FS_11)
                                .text_color(c.text_secondary)
                                .child(tier.permits()),
                        )
                    }),
                )),
        )
    }
}
