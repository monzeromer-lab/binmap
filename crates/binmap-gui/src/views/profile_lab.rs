//! The Profile Lab (`U0.2`, `U3`, `F0.6`).
//!
//! The sweep's results, in three parts the design puts side by side: a Pareto
//! scatter, the configuration table including the candidates that were
//! rejected, and the selected configuration with its gate rows.
//!
//! Three rules the view exists to keep, all of which are easy to break by
//! rendering the obvious thing:
//!
//! - **Rejected candidates stay visible.** A near miss is informative, and a
//!   table that quietly drops the configurations that broke the tests teaches
//!   the user nothing about their own build. They render greyed, with the
//!   failing gate named.
//! - **The frontier is derived, not flagged.** The engine computes dominance;
//!   this view only draws what it is told. Nothing here decides that a point
//!   is interesting.
//! - **A result inside the noise floor is coloured flat.** Not green, not red.
//!   The floor is printed beside the numbers rather than kept as an internal
//!   threshold, because a user comparing two numbers deserves to know how far
//!   apart they must be before the comparison means anything.
//!
//! The scatter is custom-painted. DESIGN-GUI §6.3 says to evaluate the
//! built-in chart first and not to write one speculatively — evaluated:
//! `gpui-component` ships area, bar, candlestick, line, pie, radar and sankey,
//! and no scatter. That answers its open question 2.

use crate::theme::{Theme, radius, space, type_scale};
use crate::widgets::{Badge, Section, Tone, eyebrow};
use binmap_core::facade::{Measurement, SweepSummary};
use binmap_core::gate::{GateResult, VerificationReport};
use gpui_kit::prelude::*;
use gpui_kit::{App, Bounds, Pixels, Window, canvas, div, point, px, quad, size};

/// How many bytes, as a person reads them.
fn bytes(value: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
    let mut amount = value as f64;
    let mut unit = 0;
    while amount >= 1024.0 && unit + 1 < UNITS.len() {
        amount /= 1024.0;
        unit += 1;
    }
    if unit == 0 { format!("{value} B") } else { format!("{amount:.1} {}", UNITS[unit]) }
}

/// The flags, shortened to fit the table's column.
///
/// The design caps this column at 166px and truncates; the full string is in
/// the selected-configuration panel, which is where someone reading one
/// configuration closely is looking. Letting it flex meant five numeric
/// columns squeezed it to nothing, which shows neither.
fn short_flags(flags: &str) -> String {
    const LIMIT: usize = 26;
    if flags.chars().count() <= LIMIT {
        return flags.to_string();
    }
    let kept: String = flags.chars().take(LIMIT - 1).collect();
    // Break on a space rather than mid-token, so `lto=fa…` never appears.
    match kept.rsplit_once(' ') {
        Some((head, _)) if head.chars().count() > LIMIT / 2 => format!("{head} …"),
        _ => format!("{kept}…"),
    }
}

fn millis(nanos: u64) -> String {
    format!("{:.1} ms", nanos as f64 / 1_000_000.0)
}

/// A delta, signed, with the sign a reader expects.
fn delta_label(value: i64) -> String {
    let magnitude = bytes(value.unsigned_abs());
    if value < 0 { format!("−{magnitude}") } else { format!("+{magnitude}") }
}

#[derive(IntoElement)]
pub struct ProfileLab {
    sweep: Option<SweepSummary>,
    selected: Option<String>,
    theme: Theme,
}

impl ProfileLab {
    pub fn new(sweep: Option<SweepSummary>, selected: Option<String>, theme: Theme) -> Self {
        Self { sweep, selected, theme }
    }
}

impl RenderOnce for ProfileLab {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let c = self.theme.colours;
        let theme = self.theme;

        let Some(sweep) = self.sweep else {
            return empty_state(theme).into_any_element();
        };

        let rejected = sweep.rejected();
        let frontier = sweep.frontier().count();
        let selected = self
            .selected
            .as_ref()
            .and_then(|id| sweep.measured.iter().find(|m| &m.id == id))
            .or_else(|| sweep.frontier().next())
            .or_else(|| sweep.measured.first())
            .cloned();

        div()
            .flex()
            .flex_col()
            .size_full()
            .gap(space::S12)
            .p(space::S16)
            .overflow_hidden()
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
                            .child("Profile Lab"),
                    )
                    .child(
                        div().flex_1().text_size(type_scale::FS_12).text_color(c.text_muted).child(
                            format!(
                                "{} configuration{} swept, {rejected} rejected",
                                sweep.measured.len(),
                                if sweep.measured.len() == 1 { "" } else { "s" }
                            ),
                        ),
                    )
                    .when(!sweep.complete, |d| {
                        d.child(Badge::new("running", Tone::Accent, theme).caps())
                    }),
            )
            // The scatter and its legend.
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(space::S12)
                    .flex_none()
                    .h(px(300.))
                    .child(
                        Section::new(theme)
                            .flush()
                            .child(scatter(&sweep, selected.as_ref(), theme))
                            .into_any_element(),
                    )
                    .child(legend(&sweep, frontier, rejected, theme).into_any_element()),
            )
            // The table, and the selected configuration beside it.
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(space::S12)
                    .flex_1()
                    .min_h_0()
                    // The table flexes and the panel is fixed. A table whose
                    // columns are squeezed to nothing is unreadable, and the
                    // panel's content is the same width whatever the window
                    // does.
                    .child(div().flex_1().min_w_0().child(
                        Section::titled("Configurations", theme).flush().child(table(
                            &sweep,
                            selected.as_ref(),
                            theme,
                        )),
                    ))
                    .when_some(selected, |d, measurement| {
                        d.child(div().w(px(330.)).flex_none().child(selected_panel(
                            &sweep,
                            &measurement,
                            theme,
                        )))
                    }),
            )
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
                .child("Configurations, measured on your crate"),
        )
        .child(
            div().max_w(px(560.)).text_size(type_scale::FS_13).text_color(c.text_secondary).child(
                "Build-profile advice is folklore until someone measures it on the code in \
                     front of them. The sweep builds your workspace across opt-level, lto, \
                     codegen-units, panic and strip, then records size, runtime and build time \
                     for each — including the configurations that broke the tests, because a \
                     near miss is informative.",
            ),
        )
        .child(div().text_size(type_scale::FS_11).text_color(c.text_muted).child(
            "The sweep sets profile settings per build. Your Cargo.toml is never \
                     modified.",
        ))
}

/// The Pareto scatter: runtime across, size up, build time as area.
///
/// Points with no runtime — the common case until a benchmark is declared —
/// are spread along the axis by index rather than stacked at zero, which would
/// claim they were all instantaneous.
fn scatter(sweep: &SweepSummary, selected: Option<&Measurement>, theme: Theme) -> impl IntoElement {
    let c = theme.colours;
    let points: Vec<PlotPoint> = plot_points(sweep);
    let selected_id = selected.map(|m| m.id.clone());

    div()
        .flex()
        .flex_col()
        .size_full()
        .child(div().flex_none().px(space::S12).pt(space::S8).child(eyebrow("Binary size", theme)))
        .child(
            div().flex_1().min_h_0().child(
                canvas(
                    move |bounds, _, _| layout(&points, bounds),
                    move |_, plotted: Vec<PlottedPoint>, window, _| {
                        // The frontier line first, so the points sit on it.
                        let mut frontier: Vec<&PlottedPoint> =
                            plotted.iter().filter(|p| p.on_frontier).collect();
                        frontier.sort_by(|a, b| {
                            a.at.x.partial_cmp(&b.at.x).unwrap_or(std::cmp::Ordering::Equal)
                        });
                        for pair in frontier.windows(2) {
                            paint_segment(window, pair[0].at, pair[1].at, c.accent);
                        }

                        for plot in &plotted {
                            let colour = if !plot.eligible {
                                c.text_disabled
                            } else if plot.on_frontier {
                                c.accent
                            } else {
                                c.status_info
                            };
                            let selected = selected_id.as_deref() == Some(plot.id.as_str());
                            let radius = plot.radius + if selected { px(3.) } else { px(0.) };
                            window.paint_quad(quad(
                                Bounds {
                                    origin: point(plot.at.x - radius, plot.at.y - radius),
                                    size: size(radius * 2.0, radius * 2.0),
                                },
                                radius,
                                colour,
                                if selected { px(2.) } else { px(0.) },
                                c.text_primary,
                                Default::default(),
                            ));
                        }
                    },
                )
                .size_full(),
            ),
        )
        .child(div().flex_none().px(space::S12).pb(space::S8).child(eyebrow(
            if sweep.measured.iter().any(|m| m.runtime_nanos.is_some()) {
                "Runtime"
            } else {
                "Configuration — no benchmark declared, so runtime is not an axis"
            },
            theme,
        )))
}

struct PlotPoint {
    id: String,
    x: f64,
    y: f64,
    area: f64,
    on_frontier: bool,
    eligible: bool,
}

struct PlottedPoint {
    id: String,
    at: gpui_kit::Point<Pixels>,
    radius: Pixels,
    on_frontier: bool,
    eligible: bool,
}

fn plot_points(sweep: &SweepSummary) -> Vec<PlotPoint> {
    let has_runtime = sweep.measured.iter().any(|m| m.runtime_nanos.is_some());
    sweep
        .measured
        .iter()
        .enumerate()
        .filter_map(|(index, m)| {
            let y = m.size_bytes? as f64;
            // Without a benchmark there is no runtime axis, so points spread
            // by index. Stacking them at zero would draw every configuration
            // as instantaneous, which is a claim.
            let x = if has_runtime { m.runtime_nanos.unwrap_or(0) as f64 } else { index as f64 };
            Some(PlotPoint {
                id: m.id.clone(),
                x,
                y,
                area: m.build_time_nanos.unwrap_or(0) as f64,
                on_frontier: m.on_frontier,
                eligible: m.passed() && m.built,
            })
        })
        .collect()
}

fn layout(points: &[PlotPoint], bounds: Bounds<Pixels>) -> Vec<PlottedPoint> {
    if points.is_empty() {
        return Vec::new();
    }
    let pad = px(24.);
    let (left, top) = (bounds.origin.x + pad, bounds.origin.y + pad);
    let width = (bounds.size.width - pad * 2.0).max(px(1.));
    let height = (bounds.size.height - pad * 2.0).max(px(1.));

    let span = |values: &[f64]| -> (f64, f64) {
        let low = values.iter().cloned().fold(f64::INFINITY, f64::min);
        let high = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        if (high - low).abs() < f64::EPSILON { (low - 1.0, high + 1.0) } else { (low, high) }
    };

    let (x_low, x_high) = span(&points.iter().map(|p| p.x).collect::<Vec<_>>());
    let (y_low, y_high) = span(&points.iter().map(|p| p.y).collect::<Vec<_>>());
    let biggest_area = points.iter().map(|p| p.area).fold(0.0f64, f64::max);

    points
        .iter()
        .map(|p| {
            let fx = ((p.x - x_low) / (x_high - x_low)) as f32;
            let fy = ((p.y - y_low) / (y_high - y_low)) as f32;
            // Circle *area* is build time, not radius — area is what the eye
            // compares, and scaling the radius exaggerates by its square.
            let scale = if biggest_area > 0.0 { (p.area / biggest_area).sqrt() } else { 0.0 };
            PlottedPoint {
                id: p.id.clone(),
                at: point(left + width * fx, top + height * (1.0 - fy)),
                radius: px(3.0 + 4.0 * scale as f32),
                on_frontier: p.on_frontier,
                eligible: p.eligible,
            }
        })
        .collect()
}

/// A line between two points, drawn as a thin rotated-free rectangle chain.
///
/// GPUI paints quads; a proper polyline wants `PathBuilder`, which is Phase 1's
/// business when the treemap needs real paths. A short chain of dots reads the
/// same at this size and costs nothing to get right.
fn paint_segment(
    window: &mut Window,
    from: gpui_kit::Point<Pixels>,
    to: gpui_kit::Point<Pixels>,
    colour: gpui_kit::Rgba,
) {
    let steps = 24;
    for step in 0..=steps {
        let t = step as f32 / steps as f32;
        let x = from.x + (to.x - from.x) * t;
        let y = from.y + (to.y - from.y) * t;
        window.paint_quad(quad(
            Bounds { origin: point(x - px(1.), y - px(1.)), size: size(px(2.), px(2.)) },
            px(1.),
            colour,
            px(0.),
            colour,
            Default::default(),
        ));
    }
}

/// What the scatter means, spelled out.
fn legend(
    sweep: &SweepSummary,
    frontier: usize,
    rejected: usize,
    theme: Theme,
) -> impl IntoElement {
    let c = theme.colours;
    let total = sweep.measured.len();

    let row = |swatch: gpui_kit::Rgba, label: String, note: Option<String>| {
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(space::S8)
            .child(div().flex_none().w(px(8.)).h(px(8.)).rounded(radius::CHIP).bg(swatch))
            .child(
                div()
                    .flex_1()
                    .text_size(type_scale::FS_11)
                    .text_color(c.text_secondary)
                    .child(label),
            )
            .when_some(note, |d, note| {
                d.child(
                    div()
                        .flex_none()
                        .font_family("JetBrains Mono")
                        .text_size(type_scale::FS_11)
                        .text_color(c.text_muted)
                        .child(note),
                )
            })
    };

    Section::titled("Reading the frontier", theme)
        .child(eyebrow("Series", theme))
        .child(row(c.accent, "On the frontier".into(), Some(format!("{frontier} of {total}"))))
        .child(row(c.status_info, "Dominated by another configuration".into(), None))
        .child(row(c.text_disabled, "Rejected by a gate".into(), Some(rejected.to_string())))
        .child(row(c.text_muted, "Circle area is build time".into(), None))
        .child(div().h(space::S4))
        .child(eyebrow("Axes", theme))
        .child(div().text_size(type_scale::FS_11).text_color(c.text_muted).child(
            match sweep.noise_floor_samples {
                0 => "x  configuration — no benchmark declared".to_string(),
                n => format!("x  runtime — hyperfine, {n} runs"),
            },
        ))
        .child(
            div()
                .text_size(type_scale::FS_11)
                .text_color(c.text_muted)
                .child("y  binary size — total file"),
        )
        // The rule, printed where the numbers are read.
        .when_some(sweep.noise_floor_label(), |section, floor| {
            section.child(
                div().pt(space::S4).text_size(type_scale::FS_11).text_color(c.text_muted).child(
                    format!(
                        "Improvements below this machine's measured {floor} noise floor are \
                         reported inconclusive and never coloured as a win."
                    ),
                ),
            )
        })
}

/// The configuration table, rejected candidates included.
fn table(sweep: &SweepSummary, selected: Option<&Measurement>, theme: Theme) -> impl IntoElement {
    let c = theme.colours;
    let selected_id = selected.map(|m| m.id.clone());

    let header = div()
        .flex()
        .flex_row()
        .items_center()
        .flex_none()
        .h(px(26.))
        .px(space::S12)
        .gap(space::S8)
        .border_b_1()
        .border_color(c.border_subtle)
        .child(div().w(px(166.)).flex_none().child(eyebrow("Flags", theme)))
        .child(div().flex_1().min_w_0())
        .child(div().w(px(80.)).flex_none().child(eyebrow("Size", theme)))
        .child(div().w(px(80.)).flex_none().child(eyebrow("Δ size", theme)))
        .child(div().w(px(78.)).flex_none().child(eyebrow("Runtime", theme)))
        .child(div().w(px(64.)).flex_none().child(eyebrow("Build", theme)))
        .child(div().w(px(116.)).flex_none().child(eyebrow("Gates", theme)));

    // Frontier first, then the rest by size, then the rejected. The order a
    // reader wants: what won, what nearly won, what did not.
    let mut rows: Vec<&Measurement> = sweep.measured.iter().collect();
    rows.sort_by_key(|m| (!m.passed(), !m.on_frontier, m.size_bytes.unwrap_or(u64::MAX)));

    div().flex().flex_col().size_full().child(header).child(
        div().flex().flex_col().flex_1().min_h_0().overflow_hidden().children(
            rows.into_iter().map(move |m| {
                let is_selected = selected_id.as_deref() == Some(m.id.as_str());
                let rejected = !m.passed();

                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .flex_none()
                    .h(space::ROW_H)
                    .px(space::S12)
                    .gap(space::S8)
                    .border_b_1()
                    .border_color(c.border_subtle)
                    .when(is_selected, |d| d.bg(c.surface_selected))
                    .when(m.on_frontier && !is_selected, |d| d.bg(c.surface_raised))
                    .text_size(type_scale::FS_11)
                    .font_family("JetBrains Mono")
                    .child(
                        div()
                            .w(px(166.))
                            .flex_none()
                            .overflow_hidden()
                            // Rejected candidates stay, greyed. A near miss is
                            // informative and a table that drops them teaches
                            // the reader nothing about their own build.
                            .text_color(if rejected { c.text_disabled } else { c.text_body })
                            .child(short_flags(&m.flags)),
                    )
                    .child(div().flex_1().min_w_0())
                    .child(
                        div()
                            .w(px(80.))
                            .flex_none()
                            .text_color(c.text_body)
                            .child(m.size_bytes.map(bytes).unwrap_or_else(|| "—".into())),
                    )
                    .child(
                        div()
                            .w(px(80.))
                            .flex_none()
                            .text_color(match m.size_delta {
                                Some(d) if d < 0 => c.delta_improve,
                                Some(d) if d > 0 => c.delta_regress,
                                _ => c.delta_flat,
                            })
                            .child(m.size_delta.map(delta_label).unwrap_or_else(|| "—".into())),
                    )
                    .child(
                        div()
                            .w(px(78.))
                            .flex_none()
                            .text_color(c.text_muted)
                            .child(m.runtime_nanos.map(millis).unwrap_or_else(|| "—".into())),
                    )
                    .child(
                        div()
                            .w(px(64.))
                            .flex_none()
                            .text_color(c.text_disabled)
                            .child(m.build_time_nanos.map(millis).unwrap_or_else(|| "—".into())),
                    )
                    .child(div().w(px(116.)).flex_none().child(match m.rejected_by() {
                        // The failing gate is named. "Rejected" alone tells the
                        // reader nothing they can act on.
                        Some(gate) => Badge::new(gate.label(), Tone::Fail, theme).caps(),
                        None => Badge::new("pass", Tone::Pass, theme).caps(),
                    }))
            }),
        ),
    )
}

/// The selected configuration: its numbers, and every gate's verdict.
fn selected_panel(
    sweep: &SweepSummary,
    measurement: &Measurement,
    theme: Theme,
) -> impl IntoElement {
    let c = theme.colours;

    Section::titled("Selected configuration", theme)
        .child(
            div()
                .font_family("JetBrains Mono")
                .text_size(type_scale::FS_11)
                .text_color(c.text_accent)
                .child(measurement.flags.clone()),
        )
        .child(metric_row(sweep, measurement, theme))
        .child(div().h(space::S4))
        .child(eyebrow("Gates", theme))
        .child(gate_rows(&measurement.gates, theme))
        .when_some(sweep.noise_floor_label(), |section, floor| {
            section.child(
                div().pt(space::S4).text_size(type_scale::FS_11).text_color(c.text_muted).child(
                    format!(
                        "noise floor on this machine: {floor}, measured over {} runs",
                        sweep.noise_floor_samples
                    ),
                ),
            )
        })
}

fn metric_row(sweep: &SweepSummary, m: &Measurement, theme: Theme) -> impl IntoElement {
    let c = theme.colours;

    let stat = |label: &'static str, value: String, delta: Option<String>, tone: gpui_kit::Rgba| {
        div()
            .flex()
            .flex_col()
            .flex_1()
            .gap(space::S2)
            .p(space::S8)
            .rounded(radius::INPUT)
            .bg(c.surface_raised)
            .child(div().text_size(type_scale::FS_11).text_color(c.text_muted).child(label))
            .child(
                div()
                    .font_family("JetBrains Mono")
                    .text_size(type_scale::FS_16)
                    .text_color(c.text_primary)
                    .child(value),
            )
            .when_some(delta, |d, delta| {
                d.child(
                    div()
                        .font_family("JetBrains Mono")
                        .text_size(type_scale::FS_11)
                        .text_color(tone)
                        .child(delta),
                )
            })
    };

    let size_delta = m.size_delta.map(|d| {
        let baseline = sweep.baseline_bytes.unwrap_or(0) as f64;
        if baseline == 0.0 {
            delta_label(d)
        } else {
            format!("{} · {:+.1}%", delta_label(d), d as f64 * 100.0 / baseline)
        }
    });

    div()
        .flex()
        .flex_row()
        .gap(space::S8)
        .child(stat(
            "Size",
            m.size_bytes.map(bytes).unwrap_or_else(|| "not built".into()),
            size_delta,
            match m.size_delta {
                Some(d) if d < 0 => c.delta_improve,
                Some(d) if d > 0 => c.delta_regress,
                _ => c.delta_flat,
            },
        ))
        .child(stat(
            "Runtime",
            m.runtime_nanos.map(millis).unwrap_or_else(|| "—".into()),
            m.runtime_nanos.is_none().then(|| "no benchmark declared".to_string()),
            c.delta_flat,
        ))
}

fn gate_rows(report: &VerificationReport, theme: Theme) -> impl IntoElement {
    let c = theme.colours;

    div().flex().flex_col().gap(space::S4).children(report.outcomes.iter().map(move |outcome| {
        let (glyph, colour) = match &outcome.result {
            GateResult::Passed => ("✓", c.status_pass),
            GateResult::Failed => ("✗", c.status_fail),
            // Inconclusive is neither. Colouring it either way is the thing
            // §6 forbids.
            GateResult::Inconclusive { .. } => ("~", c.delta_flat),
            GateResult::Skipped { .. } => ("–", c.text_disabled),
        };

        div()
            .flex()
            .flex_col()
            .gap(space::S2)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(space::S6)
                    .child(div().flex_none().w(px(10.)).text_color(colour).child(glyph))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .font_family("JetBrains Mono")
                            .text_size(type_scale::FS_11)
                            .text_color(c.text_body)
                            .child(outcome.qualified_label()),
                    )
                    .child(
                        div()
                            .flex_none()
                            .text_size(type_scale::FS_11)
                            .text_color(c.text_muted)
                            .child(outcome.detail.clone()),
                    ),
            )
            // The sanitizer gap, and any other caveat, stated beside the pass
            // it qualifies rather than left for the user to infer.
            .when_some(outcome.caveat.clone(), |d, caveat| {
                d.child(
                    div()
                        .pl(px(16.))
                        .text_size(type_scale::FS_11)
                        .text_color(c.status_warn)
                        .child(caveat),
                )
            })
    }))
}
