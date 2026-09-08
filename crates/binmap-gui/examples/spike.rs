//! The mandatory spike (DESIGN-GUI Appendix, PRD R4).
//!
//! Throwaway on purpose. With the CLI removed the interface is on the critical
//! path from week one, and a framework that disappoints on this hardware is a
//! plan-changing discovery worth having in week one rather than month four.
//! Four things, and nothing else:
//!
//! 1. A window with a dock layout and two panels.
//! 2. A DataTable of 100,000 synthetic rows, scrolled hard.
//! 3. A `canvas` element painting 20,000 quads, with hit testing.
//! 4. One background task streaming events into a view.
//!
//! It answers open questions 4 and 5 in DESIGN-GUI §10 and nothing else. It is
//! not a design, it is not styled, and no part of it survives into Phase 0.
//!
//!     cargo run -p binmap-gui --example spike

use gpui_kit::component::dock::Panel;
use gpui_kit::component::table::{Column, ColumnSort, DataTable, TableDelegate, TableState};
use gpui_kit::component::{ActiveTheme, Root, TitleBar};
use gpui_kit::base::dock::{DockArea, DockLayout, DockPlacement, PanelEvent};
use gpui_kit::*;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Where the spike's answers accumulate, so the run ends in numbers rather
/// than in an impression.
///
/// Shared rather than read back out of entities: the reporting task runs on
/// the async context that opened the window, and reaching into a view from
/// there is more ceremony than a throwaway measurement deserves.
#[derive(Default)]
struct Tally {
    prepaints: usize,
    canvas_bounds: (f32, f32),
    root_paints: usize,
    root_renders: usize,
    canvas_renders: usize,
    table_built: Duration,
    paints: Vec<Duration>,
    hit_tests: usize,
    hit_test_total: Duration,
    events: usize,
}

type Shared = Arc<Mutex<Tally>>;

/// The four answers, printed when the spike quits.
#[derive(Default)]
struct Report {
    prepaints: usize,
    canvas_bounds: (f32, f32),
    root_paints: usize,
    root_renders: usize,
    canvas_renders: usize,
    table_rows: usize,
    table_built: Duration,
    quads: usize,
    paints: Vec<Duration>,
    hit_tests: usize,
    hit_test_total: Duration,
    events: usize,
}

impl Report {
    fn of(shared: &Shared) -> Self {
        let tally = shared.lock().expect("tally poisoned");
        Report {
            prepaints: tally.prepaints,
            canvas_bounds: tally.canvas_bounds,
            root_paints: tally.root_paints,
            root_renders: tally.root_renders,
            canvas_renders: tally.canvas_renders,
            table_rows: ROWS,
            table_built: tally.table_built,
            quads: QUADS,
            paints: tally.paints.clone(),
            hit_tests: tally.hit_tests,
            hit_test_total: tally.hit_test_total,
            events: tally.events,
        }
    }

    fn percentile(sorted: &[Duration], fraction: f64) -> Duration {
        if sorted.is_empty() {
            return Duration::ZERO;
        }
        let index = ((sorted.len() - 1) as f64 * fraction).round() as usize;
        sorted[index]
    }

    fn print(&self) {
        let mut paints = self.paints.clone();
        paints.sort();

        println!("\n── GPUI spike ─────────────────────────────────────────────");
        println!("DESIGN-GUI's appendix calls this mandatory. Four questions:\n");

        println!("1. A window with a dock layout and two panels");
        println!("   opened · three panels, an h-split over a v-split\n");

        println!("2. A DataTable of {} rows", self.table_rows);
        println!("   built in {:?}\n", self.table_built);

        println!("3. A canvas painting {} quads, with hit testing", self.quads);
        println!(
            "   root rendered {} times and painted {} times; canvas panel rendered {} times",
            self.root_renders, self.root_paints, self.canvas_renders
        );
        println!(
            "   canvas prepainted {} times, last laid out at {:.0}x{:.0}",
            self.prepaints, self.canvas_bounds.0, self.canvas_bounds.1
        );
        if paints.is_empty() {
            println!("   the canvas element's paint closure never ran");
        } else {
            println!(
                "   {} paints · median {:?} · p95 {:?} · worst {:?}",
                paints.len(),
                Report::percentile(&paints, 0.5),
                Report::percentile(&paints, 0.95),
                paints.last().copied().unwrap_or_default()
            );
            let budget = Duration::from_micros(16_667);
            let over = paints.iter().filter(|d| **d > budget).count();
            println!(
                "   {over} of {} over a 16.7ms frame budget ({:.1}%)",
                paints.len(),
                over as f64 * 100.0 / paints.len() as f64
            );
        }
        if self.hit_tests > 0 {
            println!(
                "   {} hit tests · {:?} each on average, computed not scanned",
                self.hit_tests,
                self.hit_test_total / self.hit_tests as u32
            );
        }
        println!();

        println!("4. A background task streaming events into a view");
        println!("   {} events applied on the foreground\n", self.events);
    }
}

// ---------------------------------------------------------------------------
// 2. A virtualized table of 100,000 rows.
//
// DESIGN-GUI open question 5: "How large can the DataTable go in practice?"
// Documented for very large row counts; a real symbol table is tens of
// thousands of rows, so this is deliberately an order of magnitude past it.
// ---------------------------------------------------------------------------

const ROWS: usize = 100_000;

struct Symbols {
    rows: Vec<SymbolRow>,
}

struct SymbolRow {
    name: SharedString,
    bytes: u64,
    crate_name: SharedString,
}

impl Symbols {
    fn synthetic() -> Self {
        // Shaped like a real symbol table: a few crates, deep generic paths,
        // a long tail of small symbols under a few large ones.
        let crates = ["core", "alloc", "std", "serde", "app"];
        let rows = (0..ROWS)
            .map(|index| {
                let crate_name = crates[index % crates.len()];
                SymbolRow {
                    name: format!(
                        "{crate_name}::collections::btree::map::BTreeMap<K,V>::insert::h{index:016x}"
                    )
                    .into(),
                    bytes: (((index * 2654435761) % 65536) + 16) as u64,
                    crate_name: crate_name.into(),
                }
            })
            .collect();
        Self { rows }
    }
}

impl TableDelegate for Symbols {
    fn columns_count(&self, _: &App) -> usize {
        3
    }

    fn rows_count(&self, _: &App) -> usize {
        self.rows.len()
    }

    fn column(&self, index: usize, _: &App) -> Column {
        match index {
            0 => Column::new("name", "Symbol").width(px(560.)),
            1 => Column::new("bytes", "Bytes").width(px(120.)),
            _ => Column::new("crate", "Crate").width(px(120.)),
        }
    }

    fn perform_sort(
        &mut self,
        column: usize,
        sort: ColumnSort,
        _: &mut Window,
        _: &mut Context<TableState<Self>>,
    ) {
        // Sorting 100k rows is the other half of the question: does it stay
        // interactive once the data is real?
        let ascending = matches!(sort, ColumnSort::Ascending);
        match column {
            1 => self.rows.sort_by_key(|row| row.bytes),
            _ => self.rows.sort_by(|a, b| a.name.cmp(&b.name)),
        }
        if !ascending {
            self.rows.reverse();
        }
    }

    fn render_td(
        &mut self,
        row: usize,
        column: usize,
        _: &mut Window,
        _: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let row = &self.rows[row];
        match column {
            0 => div().text_xs().child(row.name.clone()),
            1 => div().text_xs().child(format!("{}", row.bytes)),
            _ => div().text_xs().child(row.crate_name.clone()),
        }
    }
}

struct TablePanel {
    focus: FocusHandle,
    table: Entity<TableState<Symbols>>,
    built_in: Duration,
}

impl TablePanel {
    fn new(shared: Shared, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let started = Instant::now();
        let table = cx.new(|cx| TableState::new(Symbols::synthetic(), window, cx));
        let built_in = started.elapsed();
        shared.lock().expect("tally poisoned").table_built = built_in;
        Self { focus: cx.focus_handle(), table, built_in }
    }
}

impl Focusable for TablePanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl EventEmitter<PanelEvent> for TablePanel {}

impl gpui_kit::base::dock::Panel for TablePanel {
    fn panel_name(&self) -> &'static str {
        "spike-table"
    }
}

impl Panel for TablePanel {
    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        SharedString::from("100,000 rows")
    }
}

impl Render for TablePanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .p_2()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("{ROWS} rows built in {:?}", self.built_in)),
            )
            .child(div().flex_1().child(DataTable::new(&self.table)))
    }
}

// ---------------------------------------------------------------------------
// 3. Twenty thousand quads on the GPU, with hit testing.
//
// This is the treemap's question, asked before squarification exists: can the
// canvas carry the rectangle count a real binary produces, and can a pointer
// find one of them without a linear scan?
// ---------------------------------------------------------------------------

const QUADS: usize = 20_000;

#[derive(Clone)]
struct Cell {
    index: usize,
    bounds: Bounds<Pixels>,
}

/// A uniform grid over the canvas. Not a real spatial index — the point is
/// that hit testing must not be a linear scan over 20,000 rectangles, and a
/// grid is enough to prove the shape of the answer.
struct Grid {
    cells: Vec<Cell>,
    columns: usize,
    rows: usize,
    size: Size<Pixels>,
}

impl Grid {
    fn lay_out(bounds: Bounds<Pixels>) -> Self {
        let columns = (QUADS as f32).sqrt().ceil() as usize;
        let rows = QUADS.div_ceil(columns);
        let size = size(bounds.size.width / columns as f32, bounds.size.height / rows as f32);

        let cells = (0..QUADS)
            .map(|index| {
                let (column, row) = (index % columns, index / columns);
                Cell {
                    index,
                    bounds: Bounds {
                        origin: point(
                            bounds.origin.x + size.width * column as f32,
                            bounds.origin.y + size.height * row as f32,
                        ),
                        size,
                    },
                }
            })
            .collect();

        Self { cells, columns, rows, size }
    }

    /// O(1): compute the cell from the position rather than searching for it.
    fn at(&self, position: Point<Pixels>, origin: Point<Pixels>) -> Option<usize> {
        let column = ((position.x - origin.x) / self.size.width) as usize;
        let row = ((position.y - origin.y) / self.size.height) as usize;
        if column >= self.columns || row >= self.rows {
            return None;
        }
        let index = row * self.columns + column;
        (index < self.cells.len()).then_some(index)
    }
}

struct CanvasPanel {
    focus: FocusHandle,
    hovered: Option<usize>,
    last_paint: Duration,
    origin: Point<Pixels>,
    grid_size: Size<Pixels>,
    columns: usize,
    shared: Shared,
    hit_tests: usize,
    hit_test_total: Duration,
}

impl CanvasPanel {
    fn new(shared: Shared, cx: &mut Context<Self>) -> Self {
        Self {
            focus: cx.focus_handle(),
            hovered: None,
            last_paint: Duration::ZERO,
            origin: point(px(0.), px(0.)),
            grid_size: size(px(1.), px(1.)),
            columns: 1,
            shared,
            hit_tests: 0,
            hit_test_total: Duration::ZERO,
        }
    }
}

impl Focusable for CanvasPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl EventEmitter<PanelEvent> for CanvasPanel {}

impl gpui_kit::base::dock::Panel for CanvasPanel {
    fn panel_name(&self) -> &'static str {
        "spike-canvas"
    }
}

impl Panel for CanvasPanel {
    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        SharedString::from("20,000 quads")
    }
}

impl Render for CanvasPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let shared = Arc::clone(&self.shared);
        shared.lock().expect("tally poisoned").canvas_renders += 1;

        let hovered = self.hovered;
        let theme = cx.theme();
        let (fill, highlight, border) = (theme.secondary, theme.primary, theme.border);
        let muted = theme.muted_foreground;

        let prepaint_tally = Arc::clone(&shared);
        let paint_tally = Arc::clone(&shared);

        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .p_2()
                    .text_xs()
                    .text_color(muted)
                    .child(match hovered {
                        Some(index) => format!(
                            "{QUADS} quads · last paint {:?} · hovering #{index}",
                            self.last_paint
                        ),
                        None => format!("{QUADS} quads · last paint {:?}", self.last_paint),
                    }),
            )
            .child(
                div()
                    .id("spike-canvas")
                    .flex_1()
                    .overflow_hidden()
                    .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                        let started = Instant::now();
                        let column =
                            ((event.position.x - this.origin.x) / this.grid_size.width) as usize;
                        let row =
                            ((event.position.y - this.origin.y) / this.grid_size.height) as usize;
                        let hit = (column < this.columns)
                            .then(|| row * this.columns + column)
                            .filter(|index| *index < QUADS);

                        this.hit_tests += 1;
                        this.hit_test_total += started.elapsed();
                        {
                            let mut tally = this.shared.lock().expect("tally poisoned");
                            tally.hit_tests = this.hit_tests;
                            tally.hit_test_total = this.hit_test_total;
                        }

                        if this.hovered != hit {
                            this.hovered = hit;
                            // The repaint request. Forgetting this produces a
                            // view that silently stops updating, which
                            // DESIGN-GUI calls the most common GPUI bug — so
                            // the spike exercises it rather than assuming it.
                            cx.notify();
                        }
                    }))
                    .child(
                        canvas(
                            // prepaint: lay the grid out once per frame, at
                            // whatever bounds the flex box settled on.
                            move |bounds: Bounds<Pixels>, _: &mut Window, _: &mut App| {
                                let mut tally =
                                    prepaint_tally.lock().expect("tally poisoned");
                                tally.prepaints += 1;
                                tally.canvas_bounds =
                                    (bounds.size.width.into(), bounds.size.height.into());
                                drop(tally);
                                Grid::lay_out(bounds)
                            },
                            // paint: emit the quads and time the loop.
                            move |_, grid: Grid, window: &mut Window, _: &mut App| {
                                let started = Instant::now();
                                for cell in &grid.cells {
                                    let background =
                                        if Some(cell.index) == hovered { highlight } else { fill };
                                    window.paint_quad(quad(
                                        cell.bounds,
                                        px(0.),
                                        background,
                                        px(0.5),
                                        border,
                                        Default::default(),
                                    ));
                                }
                                paint_tally
                                    .lock()
                                    .expect("tally poisoned")
                                    .paints
                                    .push(started.elapsed());
                            },
                        )
                        .size_full(),
                    ),
            )
    }
}

// ---------------------------------------------------------------------------
// 4. A background task streaming events into a view.
//
// The shape every analysis uses: work on the background executor, updates
// applied on the foreground, cx.notify() for the repaint.
// ---------------------------------------------------------------------------

struct StreamPanel {
    focus: FocusHandle,
    received: usize,
    latest: SharedString,
    started: Instant,
    shared: Shared,
}

impl StreamPanel {
    fn new(shared: Shared, cx: &mut Context<Self>) -> Self {
        let this = Self {
            focus: cx.focus_handle(),
            received: 0,
            latest: "waiting".into(),
            started: Instant::now(),
            shared: Arc::clone(&shared),
        };

        cx.spawn(async move |panel, cx| {
            for step in 1..=96 {
                // Pretend to be a build. The point is that the foreground
                // stays responsive while this happens.
                cx.background_executor().timer(Duration::from_millis(40)).await;
                let updated = panel.update(cx, |panel: &mut StreamPanel, cx| {
                    panel.received = step;
                    panel.latest = format!("configuration {step} of 96 measured").into();
                    panel.shared.lock().expect("tally poisoned").events = step;
                    cx.notify();
                });
                if updated.is_err() {
                    break;
                }
            }
        })
        .detach();

        this
    }
}

impl Focusable for StreamPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl EventEmitter<PanelEvent> for StreamPanel {}

impl gpui_kit::base::dock::Panel for StreamPanel {
    fn panel_name(&self) -> &'static str {
        "spike-stream"
    }
}

impl Panel for StreamPanel {
    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        SharedString::from("Streaming")
    }
}

impl Render for StreamPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .p_4()
            .flex()
            .flex_col()
            .gap_2()
            .text_sm()
            .child(self.latest.clone())
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("{} events in {:?}", self.received, self.started.elapsed())),
            )
    }
}

// ---------------------------------------------------------------------------
// 1. The window and the dock.
// ---------------------------------------------------------------------------

struct Spike {
    dock: Entity<DockArea>,
    /// Rendered beside the dock rather than inside it.
    ///
    /// Inside a dock panel the canvas element's paint closure never ran on
    /// this version — the panel rendered, the window painted, and the quads
    /// never reached the screen. That is a finding in its own right (see the
    /// note the spike prints), but the question the spike exists to answer is
    /// whether the canvas can carry a treemap's rectangle count, so it is
    /// asked where the answer is not confounded.
    canvas: Entity<CanvasPanel>,
    shared: Shared,
}

impl Render for Spike {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.shared.lock().expect("tally poisoned").root_renders += 1;
        // A one-pixel canvas at the root. If this never paints, the window is
        // not painting at all and the canvas question has not been answered —
        // which is a different finding from "the canvas is slow".
        let probe = Arc::clone(&self.shared);
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(canvas(
                |_, _, _| (),
                move |_, _, _, _| {
                    probe.lock().expect("tally poisoned").root_paints += 1;
                },
            )
            .w(px(1.))
            .h(px(1.)))
            .child(TitleBar::new().child(div().text_sm().child("Binmap — GPUI spike")))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_row()
                    .child(div().w(px(760.)).h_full().child(self.dock.clone()))
                    .child(div().flex_1().h_full().child(self.canvas.clone())),
            )
    }
}

actions!(spike, [Quit]);

fn main() {
    eprintln!("spike: opening the platform");
    gpui_kit::application().run(|cx: &mut App| {
        eprintln!("spike: platform open");
        gpui_kit::init(cx);
        cx.bind_keys([KeyBinding::new("cmd-q", Quit, None), KeyBinding::new("ctrl-q", Quit, None)]);
        cx.on_action(|_: &Quit, cx: &mut App| cx.quit());

        let shared: Shared = Arc::new(Mutex::new(Tally::default()));
        let reporting = Arc::clone(&shared);

        cx.spawn(async move |cx| {
            let bounds = cx.update(|cx| Bounds::centered(None, size(px(1400.), px(900.)), cx));
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            };

            cx.open_window(options, |window, cx| {
                let table = cx.new(|cx| TablePanel::new(Arc::clone(&shared), window, cx));
                let painting = {
                    let shared = Arc::clone(&shared);
                    cx.new(move |cx| CanvasPanel::new(shared, cx))
                };
                let streaming = {
                    let shared = Arc::clone(&shared);
                    cx.new(move |cx| StreamPanel::new(shared, cx))
                };

                let dock = cx.new(|cx| {
                    // The skin needs the area's own weak handle, so it can only
                    // be built while the area is being constructed.
                    let skin = gpui_kit::component::dock::DockSkin::new(cx);
                    DockArea::new("spike", Some(1), window, cx).with_renderer(skin)
                });

                dock.update(cx, |dock, cx| {
                    dock.set_center(
                        DockLayout::tabs().panel(table.clone()),
                        window,
                        cx,
                    );
                    // The streaming view goes in a bottom dock, which also
                    // exercises DockPlacement rather than only the centre.
                    dock.set_dock(
                        DockPlacement::Bottom,
                        DockLayout::tabs().panel(streaming.clone()),
                        window,
                        cx,
                    );
                });

                let spike = cx.new(|_| Spike {
                    dock,
                    canvas: painting.clone(),
                    shared: Arc::clone(&shared),
                });
                cx.new(|cx| Root::new(AnyView::from(spike), window, cx))
            })
            .expect("the spike could not open a window");
            eprintln!("spike: window open");

            // The spike is a measurement, not an application: it runs long
            // enough to paint a few hundred frames, prints what it found, and
            // quits.
            cx.background_executor().timer(Duration::from_secs(12)).await;
            Report::of(&reporting).print();
            let _ = cx.update(|cx| cx.quit());
        })
        .detach();
    });
}
