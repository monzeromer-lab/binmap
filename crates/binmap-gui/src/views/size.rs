//! The Size Explorer (`U1.1`, `U2`).
//!
//! The plan is explicit about the order: "Virtualized symbol table first, then
//! the treemap — the table is a genuine fallback if squarification and hit
//! testing run long." This is the table, and it is not a placeholder for the
//! treemap. A table answers "what is the largest thing, and what is it"
//! precisely; a treemap answers "what shape is this binary" at a glance. Most
//! of the questions a user actually arrives with are the first kind.
//!
//! What the view will not do is as important as what it will:
//!
//! - **It will not present an inferred size as a measured one.** ELF symbol
//!   sizes are frequently zero, and where one was derived from a neighbour the
//!   attribution says what fraction rests on that, in the header, where it
//!   cannot be missed.
//! - **It will not imply that collapsing a generic is free.** The saving is an
//!   upper bound and is labelled "at most".
//! - **It will not claim grouping works on a binary where it does not.** Legacy
//!   mangling loses the generic arguments, and the view says so rather than
//!   showing a shorter list as though it were the answer.

use crate::dispatch::{Dispatch, ignore};
use crate::theme::{Theme, radius, space, type_scale};
use crate::widgets::{Badge, Section, Tone, eyebrow};
use binmap_core::attribution::{Attribution, Driver};
use gpui_kit::prelude::*;
use gpui_kit::{App, Window, div, px, relative};

/// A path cut to fit its column, from the front.
///
/// The tail of a generic path is the part that identifies it —
/// `core::slice::sort::stable::quicksort` and
/// `core::slice::sort::stable::drift` share everything but their last
/// segment — so an ellipsis goes at the *start*, not the end.
fn shorten(path: &str, limit: usize) -> String {
    let count = path.chars().count();
    if count <= limit {
        return path.to_string();
    }
    let tail: String = path.chars().skip(count - (limit - 1)).collect();
    format!("…{tail}")
}

fn human(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
    let mut amount = bytes as f64;
    let mut unit = 0;
    while amount >= 1024.0 && unit + 1 < UNITS.len() {
        amount /= 1024.0;
        unit += 1;
    }
    if unit == 0 { format!("{bytes} B") } else { format!("{amount:.1} {}", UNITS[unit]) }
}

/// A stable colour per category, indexed rather than hashed — so a category
/// does not change colour because another one appeared.
fn driver_colour(driver: Driver, theme: Theme) -> gpui_kit::Rgba {
    use crate::theme::palette::CATEGORICAL;
    let index = match driver {
        Driver::Yours => 0,
        Driver::Dependency => 1,
        Driver::Formatting => 2,
        Driver::Panic => 3,
        Driver::Unwinding => 4,
        Driver::DropGlue => 5,
        Driver::Vtable => 6,
        Driver::StaticData => 7,
        Driver::StandardLibrary => 1,
        Driver::Runtime => 7,
    };
    let _ = theme;
    gpui_kit::rgb(CATEGORICAL[index])
}

#[derive(IntoElement)]
pub struct SizeExplorer {
    attribution: Option<Attribution>,
    theme: Theme,
    dispatch: Dispatch,
}

impl SizeExplorer {
    pub fn new(attribution: Option<Attribution>, theme: Theme) -> Self {
        Self { attribution, theme, dispatch: ignore() }
    }

    pub fn dispatching(mut self, dispatch: &Dispatch) -> Self {
        self.dispatch = std::rc::Rc::clone(dispatch);
        self
    }
}

impl RenderOnce for SizeExplorer {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let c = self.theme.colours;
        let theme = self.theme;

        let Some(attribution) = self.attribution else {
            return empty_state(theme).into_any_element();
        };

        let total = attribution.attributed_bytes.max(1);
        let yours = attribution
            .drivers
            .iter()
            .find(|(driver, _)| *driver == Driver::Yours)
            .map(|(_, bytes)| *bytes)
            .unwrap_or(0);

        div()
            .flex()
            .flex_col()
            .size_full()
            .gap(space::S12)
            .p(space::S16)
            .overflow_hidden()
            // The header, and the two caveats that qualify everything below it.
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
                            .child("Size Explorer"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(type_scale::FS_12)
                            .text_color(c.text_muted)
                            .child(format!(
                                "{} attributed across {} crates · {} is your code",
                                human(attribution.attributed_bytes),
                                attribution.crates.len(),
                                human(yours)
                            )),
                    )
                    // Where the numbers rest on inference, say so where it
                    // cannot be missed rather than in a footnote.
                    .when(attribution.inferred_fraction > 0.05, |d| {
                        d.child(
                            Badge::new(
                                format!(
                                    "{:.0}% of sizes inferred",
                                    attribution.inferred_fraction * 100.0
                                ),
                                Tone::Warn,
                                theme,
                            )
                            .caps(),
                        )
                    })
                    .when(!attribution.generic_arguments_available, |d| {
                        d.child(Badge::new("legacy mangling", Tone::Warn, theme).caps())
                    }),
            )
            // The treemap first. "What shape is this binary" is the question a
            // reader arrives with; the tables answer "and exactly how much"
            // once they have one.
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(space::S12)
                    .flex_none()
                    .h(px(240.))
                    .child(
                        div().flex_1().min_w_0().child(
                            Section::titled("Where the bytes are", theme)
                                .child(crate::views::treemap::Treemap::of(&attribution, theme)),
                        ),
                    )
                    .child(div().w(px(330.)).flex_none().child(crates(&attribution, theme))),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(space::S12)
                    .flex_none()
                    .child(div().flex_1().min_w_0().child(drivers(&attribution, total, theme))),
            )
            .child(div().flex().flex_1().min_h_0().child(monomorphizations(&attribution, theme)))
            .into_any_element()
    }
}

fn empty_state(theme: Theme) -> impl IntoElement {
    let c = theme.colours;
    div()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .size_full()
        .gap(space::S12)
        .p(space::S32)
        .child(
            div()
                .text_size(type_scale::FS_18)
                .text_color(c.text_primary)
                .child("Where the bytes went"),
        )
        .child(
            div().max_w(px(560.)).text_size(type_scale::FS_13).text_color(c.text_secondary).child(
                "Binmap reads the symbol table of the binary you ship and attributes every byte \
                 it can account for — to the crate it came from, to the category of cost it \
                 represents, and to the generic it was instantiated from. Nothing here is \
                 guessed: a size is what the table declared, or it is marked as derived from a \
                 neighbour.",
            ),
        )
        .child(
            div()
                .text_size(type_scale::FS_11)
                .text_color(c.text_muted)
                .child("Run a size analysis from the command palette."),
        )
}

/// Where the bytes go, as proportions. A bar is more legible than a number
/// for "how much of the binary is this", which is the question the category
/// breakdown answers.
fn drivers(attribution: &Attribution, total: u64, theme: Theme) -> impl IntoElement {
    let c = theme.colours;

    Section::titled("By category", theme).children(attribution.drivers.iter().map(
        move |(driver, bytes)| {
            let share = *bytes as f32 / total as f32;
            div()
                .flex()
                .flex_col()
                .gap(space::S2)
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(space::S8)
                        .child(
                            div()
                                .flex_none()
                                .w(px(8.))
                                .h(px(8.))
                                .rounded(radius::CHIP)
                                .bg(driver_colour(*driver, theme)),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_size(type_scale::FS_12)
                                .text_color(c.text_body)
                                .child(driver.label()),
                        )
                        .child(
                            div()
                                .flex_none()
                                .font_family("JetBrains Mono")
                                .text_size(type_scale::FS_11)
                                .text_color(c.text_primary)
                                .child(human(*bytes)),
                        )
                        .child(
                            div()
                                .flex_none()
                                .w(px(42.))
                                .font_family("JetBrains Mono")
                                .text_size(type_scale::FS_11)
                                .text_color(c.text_muted)
                                .child(format!("{:.1}%", share * 100.0)),
                        ),
                )
                .child(
                    div().h(px(3.)).w_full().rounded(radius::CHIP).bg(c.surface_active).child(
                        div()
                            .h_full()
                            .w(relative(share.clamp(0.0, 1.0)))
                            .rounded(radius::CHIP)
                            .bg(driver_colour(*driver, theme)),
                    ),
                )
        },
    ))
}

fn crates(attribution: &Attribution, theme: Theme) -> impl IntoElement {
    let c = theme.colours;

    Section::titled("By crate", theme).children(attribution.crates.iter().take(12).map(
        move |group| {
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(space::S8)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .font_family("JetBrains Mono")
                        .text_size(type_scale::FS_11)
                        .text_color(c.text_body)
                        .child(group.key.clone()),
                )
                .child(
                    div()
                        .flex_none()
                        .font_family("JetBrains Mono")
                        .text_size(type_scale::FS_11)
                        .text_color(c.text_primary)
                        .child(human(group.bytes)),
                )
                .child(
                    div()
                        .flex_none()
                        .w(px(52.))
                        .text_size(type_scale::FS_11)
                        .text_color(c.text_disabled)
                        .child(format!("{} sym", group.symbols)),
                )
        },
    ))
}

/// The generics worth collapsing, ranked by what collapsing would save.
fn monomorphizations(attribution: &Attribution, theme: Theme) -> impl IntoElement {
    let c = theme.colours;

    let header = div()
        .flex()
        .flex_row()
        .items_center()
        .flex_none()
        .w_full()
        .h(px(26.))
        .px(space::S12)
        .gap(space::S8)
        .border_b_1()
        .border_color(c.border_subtle)
        .child(div().w(px(330.)).flex_none().child(eyebrow("Generic", theme)))
        .child(div().flex_1().min_w_0())
        .child(div().w(px(64.)).flex_none().child(eyebrow("Copies", theme)))
        .child(div().w(px(80.)).flex_none().child(eyebrow("Total", theme)))
        .child(div().w(px(110.)).flex_none().child(eyebrow("Saves at most", theme)));

    if attribution.monomorphizations.is_empty() {
        return Section::titled("Generic instantiations", theme).child(
            div().text_size(type_scale::FS_12).text_color(c.text_muted).child(
                if attribution.generic_arguments_available {
                    "Nothing here is instantiated more than once."
                } else {
                    "This binary was built with legacy symbol mangling, which does not carry \
                     generic arguments. Build with -Csymbol-mangling-version=v0 to group \
                     instantiations."
                },
            ),
        );
    }

    Section::titled("Generic instantiations", theme).flush().child(header).child(
        div().flex().flex_col().flex_1().min_h_0().w_full().overflow_hidden().children(
            attribution.monomorphizations.iter().take(40).map(move |m| {
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .flex_none()
                    .w_full()
                    .h(space::ROW_H)
                    .px(space::S12)
                    .gap(space::S8)
                    .border_b_1()
                    .border_color(c.border_subtle)
                    .font_family("JetBrains Mono")
                    .text_size(type_scale::FS_11)
                    .child(
                        div()
                            .w(px(330.))
                            .flex_none()
                            .overflow_hidden()
                            .text_color(c.text_body)
                            .child(shorten(&m.generic_path, 44)),
                    )
                    .child(div().flex_1().min_w_0())
                    .child(
                        div()
                            .w(px(64.))
                            .flex_none()
                            .text_color(c.text_muted)
                            .child(m.instantiations.to_string()),
                    )
                    .child(
                        div()
                            .w(px(80.))
                            .flex_none()
                            .text_color(c.text_primary)
                            .child(human(m.total_bytes)),
                    )
                    // "At most", because the largest instantiation has to stay
                    // and collapsing usually costs an indirection.
                    .child(
                        div()
                            .w(px(110.))
                            .flex_none()
                            .text_color(c.delta_improve)
                            .child(format!("−{}", human(m.collapsible_bytes()))),
                    )
            }),
        ),
    )
}
