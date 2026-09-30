//! The Stack Pane (`U2.1`, `U2.3`).
//!
//! "A debugger that points at the wrong line is worse than one that admits it
//! cannot tell", and this pane is where that belief has to be visible rather
//! than merely held. So the caveats come *above* the frames: a reader deciding
//! whether to act on a stack needs to know the binary is unproven before they
//! read it, not after.
//!
//! Three distinctions the pane draws, each because flattening it would
//! mislead:
//!
//! - **Inlined frames are indented and marked.** They never had a
//!   machine-level frame. Showing them as though they did misrepresents what
//!   the stack looked like; hiding them loses the frames a reader most wants.
//! - **A guessed frame is badged.** A frame-pointer walk through optimised
//!   code produces plausible garbage, and plausible garbage rendered like
//!   measured truth is the failure this product exists to avoid.
//! - **Your code is highlighted.** Twenty frames of `std::rt` around one of
//!   yours is the normal shape of a Rust stack, and the one that matters is
//!   yours.

use crate::dispatch::{Dispatch, clickable, ignore};
use crate::state::{Action, AppState};
use crate::theme::{Theme, radius, space, type_scale};
use crate::widgets::{Badge, Section, Tone};
use binmap_core::crash::{CrashReport, FrameConfidence, StackEntry};
use gpui_kit::prelude::*;
use gpui_kit::{App, SharedString, Window, div, px};

#[derive(IntoElement)]
pub struct StackPane {
    report: Option<CrashReport>,
    /// Core dumps found near the project, offered rather than asked for.
    cores: Vec<std::path::PathBuf>,
    /// The crates the user wrote, so "your code" can be told from a
    /// dependency. There is no marker in a symbol name for it.
    own: Vec<String>,
    selected: Option<usize>,
    theme: Theme,
    dispatch: Dispatch,
}

impl StackPane {
    pub fn of(state: &AppState, theme: Theme) -> Self {
        Self {
            report: state.crash_report().cloned(),
            cores: state.cores().to_vec(),
            own: state.own_crates(),
            selected: state.selected_frame(),
            theme,
            dispatch: ignore(),
        }
    }

    pub fn dispatching(mut self, dispatch: &Dispatch) -> Self {
        self.dispatch = std::rc::Rc::clone(dispatch);
        self
    }
}

impl RenderOnce for StackPane {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let c = self.theme.colours;
        let theme = self.theme;
        let dispatch = self.dispatch;

        let Some(report) = self.report else {
            let cores = self.cores.clone();
            let found = !cores.is_empty();
            return div()
                .flex()
                .flex_col()
                .size_full()
                .p(space::S16)
                .gap(space::S8)
                .overflow_hidden()
                .child(div().text_size(type_scale::FS_18).text_color(c.text_primary).child("Stack"))
                .child(
                    div().flex_none().text_size(type_scale::FS_11).text_color(c.text_muted).child(
                        if found {
                            "A core is only meaningful beside the binary that produced it. These \
                             are checked against this project's own build before anything is \
                             shown, because a core from a different build gives a stack that is \
                             entirely plausible and entirely wrong."
                        } else {
                            "No core dumps found beside this project, in `cores/`, or under \
                             `target/`. A core from a different build gives a stack that is \
                             entirely plausible and entirely wrong, so one is checked against \
                             this project's binary before anything is shown."
                        },
                    ),
                )
                .when(found, |d| {
                    d.child(Section::titled("Core dumps here", theme).flush().child(
                        div().flex().flex_col().children(cores.into_iter().map(move |core| {
                            let dispatch = std::rc::Rc::clone(&dispatch);
                            let shown = core.display().to_string();
                            let size = std::fs::metadata(&core)
                                .map(|metadata| metadata.len())
                                .unwrap_or(0);
                            clickable(
                                div().id(SharedString::from(shown.clone())),
                                &dispatch,
                                Action::AnalyseCrash(core),
                            )
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(space::S8)
                            .flex_none()
                            .px(space::S12)
                            .py(space::S6)
                            .border_b_1()
                            .border_color(c.border_subtle)
                            .hover(|d| d.bg(c.surface_hover))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .font_family("JetBrains Mono")
                                    .text_size(type_scale::FS_11)
                                    .text_color(c.text_body)
                                    .child(SharedString::from(shown)),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .text_size(type_scale::FS_11)
                                    .text_color(c.text_muted)
                                    .child(format!("{} KiB", size / 1024)),
                            )
                        })),
                    ))
                })
                .into_any_element();
        };

        let own = self.own.clone();
        let selected = self.selected.or_else(|| report.first_of_yours(&own));
        let caveats = report.provenance.caveats();
        let trustworthy = report.provenance.is_trustworthy();
        let summary = report.describe();

        div()
            .flex()
            .flex_col()
            .size_full()
            .gap(space::S12)
            .p(space::S16)
            .overflow_hidden()
            // -- what happened -------------------------------------------
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
                            .child(SharedString::from(report.title.clone())),
                    )
                    .child(div().flex_none().child(
                        Badge::new(format!("signal {}", report.signal), Tone::Fail, theme).caps(),
                    ))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(type_scale::FS_11)
                            .text_color(c.text_muted)
                            .child(SharedString::from(summary)),
                    ),
            )
            .child(
                div()
                    .flex_none()
                    .text_size(type_scale::FS_12)
                    .text_color(c.text_body)
                    .child(SharedString::from(report.what_to_look_at.clone())),
            )
            // -- what to distrust, before the frames ----------------------
            .when(!trustworthy, |d| {
                d.child(Section::titled("Read this first", theme).flush().child(
                    div().flex().flex_col().children(caveats.into_iter().map(move |caveat| {
                        div()
                            .flex()
                            .flex_row()
                            .gap(space::S8)
                            .px(space::S12)
                            .py(space::S6)
                            .border_b_1()
                            .border_color(c.border_subtle)
                            .child(
                                div()
                                    .flex_none()
                                    .w(px(14.))
                                    .text_size(type_scale::FS_11)
                                    .text_color(c.status_warn)
                                    .child("!"),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_size(type_scale::FS_11)
                                    .text_color(c.text_body)
                                    .child(SharedString::from(caveat)),
                            )
                    })),
                ))
            })
            // -- the frames ----------------------------------------------
            .child(
                Section::titled("Frames", theme).flush().child(
                    div()
                        .id("stack-frames")
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .children(report.entries.into_iter().enumerate().map(
                            move |(index, entry)| {
                                let dispatch = std::rc::Rc::clone(&dispatch);
                                let yours = entry.is_probably_yours(&own);
                                let chosen = selected == Some(index);
                                frame_row(index, &entry, yours, chosen, theme, &dispatch)
                            },
                        )),
                ),
            )
            .into_any_element()
    }
}

/// One frame, or one inlined frame within one.
fn frame_row(
    index: usize,
    entry: &StackEntry,
    yours: bool,
    chosen: bool,
    theme: Theme,
    dispatch: &Dispatch,
) -> gpui_kit::AnyElement {
    let c = theme.colours;

    let row = div()
        .id(SharedString::from(format!("frame-{index}")))
        .flex()
        .flex_row()
        .items_center()
        .gap(space::S8)
        .flex_none()
        .px(space::S12)
        .py(space::S4)
        .border_b_1()
        .border_color(c.border_subtle)
        .when(chosen, |d| d.bg(c.surface_hover))
        // Inlined frames are indented under the physical frame they belong
        // to, which is the whole visual point: they are inside it.
        .when(entry.inlined, |d| d.pl(px(34.)))
        .child(
            div()
                .flex_none()
                .w(px(26.))
                .font_family("JetBrains Mono")
                .text_size(type_scale::FS_11)
                .text_color(c.text_muted)
                // Only the physical frame carries the number. Numbering an
                // inlined frame would imply it had one.
                .child(if entry.inlined { String::new() } else { format!("#{}", entry.frame) }),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(type_scale::FS_12)
                // Your own code is what a reader is looking for in a stack of
                // twenty runtime frames.
                .text_color(if yours { c.text_accent } else { c.text_body })
                .child(SharedString::from(entry.describe())),
        )
        .when(entry.inlined, |d| {
            d.child(div().flex_none().child(Badge::new("inlined", Tone::Info, theme).caps()))
        })
        // A guessed frame is badged. Plausible garbage rendered like measured
        // truth is the failure this product exists to avoid.
        .when(entry.confidence.needs_a_badge(), |d| {
            d.child(
                div()
                    .flex_none()
                    .child(Badge::new(FrameConfidence::Guessed.label(), Tone::Warn, theme).caps()),
            )
        })
        .child(
            div()
                .flex_none()
                .w(px(96.))
                .font_family("JetBrains Mono")
                .text_size(type_scale::FS_11)
                .text_color(c.text_muted)
                .child(SharedString::from(entry.module.clone().unwrap_or_else(|| "?".into()))),
        );

    // Only a frame with a source location is worth selecting: selecting one
    // without a line would scroll the source pane to nothing.
    if entry.file.is_some() {
        clickable(row, dispatch, Action::SelectFrame(index))
            .hover(|d| d.bg(c.surface_hover))
            .rounded(radius::CONTROL)
            .into_any_element()
    } else {
        row.into_any_element()
    }
}
