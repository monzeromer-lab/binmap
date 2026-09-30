//! The treemap (`U1.2`, DESIGN-GUI §6.1).
//!
//! Layout is [`squarify`](crate::widgets::squarify), a pure function tested
//! without a GPU. This is the painting: quads on the GPU, one pass for fills
//! and one for labels, with anything too small to read left unlabelled rather
//! than smeared.
//!
//! What it deliberately does not do is guess. A rectangle is a crate's share
//! of the bytes the symbol table accounts for — not of the file, which is
//! larger, and the difference is stated rather than absorbed.

use crate::dispatch::{Dispatch, ignore};
use crate::theme::{Theme, palette, space, type_scale};
use crate::widgets::squarify::{Tile, cull, squarify};
use binmap_core::attribution::Attribution;
use gpui_kit::prelude::*;
use gpui_kit::{App, Bounds, SharedString, Window, canvas, div, point, px, quad, size};

/// One region of the map: a crate, and how much of the binary it is.
#[derive(Debug, Clone)]
struct Region {
    label: SharedString,
    bytes: u64,
    colour: gpui_kit::Rgba,
}

#[derive(IntoElement)]
pub struct Treemap {
    regions: Vec<Region>,
    /// What the symbol table accounted for, so a share can be stated.
    attributed: u64,
    theme: Theme,
    #[allow(dead_code)]
    dispatch: Dispatch,
}

impl Treemap {
    pub fn of(attribution: &Attribution, theme: Theme) -> Self {
        // Largest first, which is what makes squarification squarify.
        let regions = attribution
            .crates
            .iter()
            .enumerate()
            .map(|(index, group)| Region {
                label: group.key.clone().into(),
                bytes: group.bytes,
                // Indexed, never hashed: a crate must not change colour
                // because another one appeared.
                colour: gpui_kit::rgb(palette::CATEGORICAL[index % palette::CATEGORICAL.len()]),
            })
            .collect();

        Self { regions, attributed: attribution.attributed_bytes, theme, dispatch: ignore() }
    }

    pub fn dispatching(mut self, dispatch: &Dispatch) -> Self {
        self.dispatch = std::rc::Rc::clone(dispatch);
        self
    }
}

/// What the paint pass needs, computed once per frame in prepaint.
struct Painted {
    tiles: Vec<Tile>,
    regions: Vec<Region>,
    hidden: usize,
    origin: gpui_kit::Point<gpui_kit::Pixels>,
}

impl RenderOnce for Treemap {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let c = self.theme.colours;
        let regions = self.regions;
        let attributed = self.attributed;

        if regions.is_empty() {
            return div()
                .flex()
                .items_center()
                .justify_center()
                .size_full()
                .text_size(type_scale::FS_12)
                .text_color(c.text_muted)
                .child("Nothing has been attributed yet.")
                .into_any_element();
        }

        let values: Vec<u64> = regions.iter().map(|region| region.bytes).collect();
        let legend_regions = regions.clone();

        div()
            .flex()
            .flex_col()
            .size_full()
            .child(
                div().flex_1().min_h_0().child(
                    canvas(
                        move |bounds, _, _| {
                            let tiles = squarify(
                                &values,
                                f32::from(bounds.size.width),
                                f32::from(bounds.size.height),
                            );
                            // Below about four pixels a rectangle is a smear
                            // of borders and conveys nothing.
                            let (tiles, hidden, _) = cull(tiles, 4.0);
                            Painted {
                                tiles,
                                regions: regions.clone(),
                                hidden,
                                origin: bounds.origin,
                            }
                        },
                        move |_, painted: Painted, window, _| {
                            for tile in &painted.tiles {
                                let Some(region) = painted.regions.get(tile.index) else {
                                    continue;
                                };
                                window.paint_quad(quad(
                                    Bounds {
                                        origin: point(
                                            painted.origin.x + px(tile.x),
                                            painted.origin.y + px(tile.y),
                                        ),
                                        size: size(px(tile.width), px(tile.height)),
                                    },
                                    px(2.),
                                    region.colour,
                                    px(1.),
                                    c.surface_app,
                                    Default::default(),
                                ));
                            }
                            let _ = painted.hidden;
                        },
                    )
                    .size_full(),
                ),
            )
            // The legend carries the names, because a label inside a
            // rectangle is unreadable below about fifty pixels and most
            // rectangles are.
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap(space::S8)
                    .flex_none()
                    .pt(space::S8)
                    .children(legend_regions.into_iter().take(10).map(move |region| {
                        let share = region.bytes as f64 * 100.0 / attributed.max(1) as f64;
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(space::S4)
                            .child(div().w(px(8.)).h(px(8.)).rounded(px(2.)).bg(region.colour))
                            .child(
                                div()
                                    .font_family("JetBrains Mono")
                                    .text_size(type_scale::FS_11)
                                    .text_color(c.text_body)
                                    .child(region.label.clone()),
                            )
                            .child(
                                div()
                                    .text_size(type_scale::FS_11)
                                    .text_color(c.text_muted)
                                    .child(format!("{share:.1}%")),
                            )
                    })),
            )
            .into_any_element()
    }
}
