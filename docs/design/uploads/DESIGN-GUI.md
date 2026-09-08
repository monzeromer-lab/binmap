# Lodestone — GUI Design (GPUI)

> Supersedes `DESIGN.md` §6-7 on technology. View design in `DESIGN.md` §7 remains valid;
> this document covers how it is built.

---

## 1. The decision

**GPUI, with `gpui-component` (via the `gpui-kit` umbrella) for widgets.**

GPUI is a hybrid immediate-and-retained-mode, GPU-accelerated UI framework written in Rust,
developed alongside the Zed editor. It is now published on crates.io rather than only
available as a git dependency, and supports macOS and Linux.

The original Tauri recommendation rested on one claim: that the web ecosystem had mature
solutions for data-dense code UI that Rust GUI frameworks lacked. That claim is no longer
true for this specific set of widgets, which is what changes the answer.

### What `gpui-component` already provides

This library is the reason the decision flips. It offers a large cross-platform component set
built on GPUI, and three of its components map almost exactly onto Lodestone's hardest views:

- **A virtualized data table.** Built for large datasets with virtual scrolling, resizable
  and fixed columns, sorting, filtering, infinite loading, custom cell rendering, context
  menus, and keyboard navigation across row, column, and cell selection modes — described as
  handling hundreds of thousands of rows. Lodestone's symbol table is tens of thousands of
  rows. This is the Phase 1 view, already built.

- **A code editor.** Stable at around 200K lines, with tree-sitter syntax highlighting and
  LSP-backed diagnostics, completion, and hover. Lodestone needs a *viewer*, not an editor,
  which is strictly easier. This was the component I assumed did not exist in the Rust
  ecosystem, and it was the single strongest argument for the web platform.

- **A dock layout.** Resizable panels, draggable tabs, nested splits, edge docks, and
  serializable freeform tiles. This is the debugger IDE shell, and serializable layouts mean
  saved workspaces come almost free.

It also ships charts and plots, markdown rendering, theming with a semantic colour system,
and a component set modelled on shadcn/ui with Lucide icons. It has been used to build a
shipped commercial desktop application, which matters more than the component count.

### What GPUI provides directly

- **Custom painting.** The `canvas()` element gives access to the low-level paint API without
  writing a full custom `Element`, and `paint_quad` / `paint_path` with `PathBuilder`,
  `StrokeOptions`, and gradients cover arbitrary 2D drawing. This is what the treemap and
  flamegraph need, and it lands on the GPU rather than in a software canvas.
- **An entity system** for shared, observable application state.
- **Actions** — user-defined structs that map keystrokes to logical operations, which is the
  right primitive for a keyboard-first tool.
- **An async executor integrated with the platform event loop**, so long analyses are
  ordinary background tasks rather than a threading problem.
- **A test framework**: a `#[gpui::test]` macro and a `TestAppContext` that can simulate
  platform input.

### What this buys

| Dimension | Tauri + TS | GPUI |
|---|---|---|
| Languages | 2 | 1 |
| Process boundary | IPC, JSON serialisation both ways | None; direct calls |
| Runtime dependency | System webview (`webkit2gtk` on Linux) | None |
| Type sharing | Duplicated in TS, or generated | Same types, compiler-checked |
| Distribution | Binary + webview version matrix | One binary |
| Debug story | Two debuggers, two ecosystems | One |

The `webkit2gtk` point deserves emphasis for a Linux-first developer tool. Tauri's Linux
webview dependency produces version-skew bugs that vary by distribution and are miserable to
support. Removing it removes an entire support category.

There is also a credibility argument that is not merely aesthetic. A tool whose headline
feature is making Rust binaries smaller and faster, shipped as a Rust binary with no runtime,
is coherent. The same tool shipped with a bundled browser engine invites an obvious question.

### What it costs

Stated plainly, because these are real:

1. **Pre-1.0 with frequent breaking changes**, by the maintainers' own description. You will
   spend time on upgrades. §7 covers the mitigation.
2. **A fragmented fork landscape.** See §2 — this is the decision that needs making now.
3. **No d3.** Treemap squarification and flamegraph layout get written by hand. Both are
   well-documented algorithms in the low hundreds of lines. This is the smallest of the three
   costs and I would not weight it heavily.
4. **A smaller hiring and contributor pool.** More Rust developers know React than know GPUI.
   Partially offset by the fact that your contributors are systems programmers who would
   rather not write TypeScript.
5. **Documentation is thin.** The framework's own guidance points people toward reading the
   Zed source and asking in Discord. Budget for source-reading as a normal activity.

---

## 2. The fork question — decide this first

GPUI has splintered, and the split matters because your widget library must match your
framework version.

| Fork | Package | Position |
|---|---|---|
| **Zed mainline** | `gpui` | The original, developed as part of Zed. Features outside Zed's needs get rejected. |
| **GPUI-CE** | `gpui-ce` | Community fork. Explicitly a drop-in, adopted via a `[patch.crates-io]` block, tracking upstream and treating API mismatches as bugs. Zed maintainers reportedly redirect out-of-scope contributions here. |
| **Open GPUI** | `open-gpui` | An independent Apache-2.0 fork that separates the framework from Zed's workspace and package names. Pre-1.0, in active cleanup, with its own font and screen-capture forks. |
| **WGPUI** | — | Diverged further; oriented to its own consuming projects. |

**Recommendation: start on Zed mainline `gpui` plus `gpui-kit`.**

Reasoning:

- `gpui-kit` exists specifically to pin a matching GPUI release and re-export every layer, so
  an application depends on one crate and never on GPUI directly. That is exactly the version
  coupling you want managed for you.
- The forks' selling point is features Zed rejects — custom shaders and similar. Lodestone
  does not need those. The `canvas` + `paint_quad` + `paint_path` surface is in mainline.
- A community fork that had drifted several hundred commits behind upstream at one point is
  a bad bet for a solo maintainer, who cannot afford to debug framework divergence on top of
  DWARF.
- Switching later is cheap by design: GPUI-CE is adopted through a `[patch.crates-io]` entry,
  not a code change. Keep that escape hatch documented in the README and take it only if you
  hit a mainline wall.

**Record this as an ADR.** It is the kind of decision that gets silently reversed by a
contributor at 2am, and the reasoning above is what a reviewer needs.

---

## 3. GPUI mental model

Enough to read the rest of this document. GPUI blends immediate and retained modes: you
describe elements each frame, but state lives in retained entities.

```rust
// State lives in an Entity, owned by GPUI, accessed through a handle.
struct SizeExplorer {
    tree: Arc<SizeTree>,
    focus: NodeId,
    zoom: ZoomState,
    selection: Option<FindingId>,
}

// A view is an entity that knows how to render itself.
impl Render for SizeExplorer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(self.toolbar(cx))
            .child(self.treemap_canvas(cx))
    }
}
```

Four things to internalise:

1. **Entities, not props.** `Entity<T>` is a handle to GPUI-owned state, similar in feel to
   an `Rc`. Views observe entities and re-render when they change. Shared state — the current
   session, the selected finding — lives in entities that multiple views observe, which is
   how the Findings Inspector stays synchronised with every other pane without prop-drilling.

2. **Styling is Tailwind-shaped.** `div().flex().gap_2().p_4().bg(theme.surface)`. Reads
   closely enough to CSS that the earlier layout sketches translate directly.

3. **Actions carry keybindings.** Define an action struct, bind a key, handle it. This is the
   mechanism behind the command palette and the keyboard-first requirement (`DESIGN.md` U12).

4. **The executor is the concurrency story.** `cx.background_executor().spawn(...)` for work,
   then update entities on the foreground. No channels-and-mutexes design needed.

---

## 4. Threading and the engine boundary

Dropping IPC removed the thing that *structurally* prevented a view from calling an analysis
engine directly. Dropping the CLI removed the second guardrail — there is no longer a shipped
headless surface that would break if the engine grew a dependency on the UI. Two deliberate
mechanisms replace both.

**One — the Cargo dependency graph.**

```
lode-gui  ──depends on──>  lode-core (types + Engine trait)
                           lode-session (read sessions/artifacts)

lode-gui  ──MUST NOT depend on──>  lode-analyze, lode-agent, lode-acp, lode-binary
```

Enforced in CI with `cargo-deny`'s dependency bans, not by convention.

**Two — the headless eval harness.** `lode-eval` drives every engine feature without a window.
A feature that only works when the UI is running fails the eval suite. This is the substitute
for the forcing function the CLI used to provide, and it is the reason `lode-eval` exists at
all despite never being shipped. A view that
wants an analysis calls `Engine::start_analysis` and receives events.

### Long-running work

Every analysis is minutes-long and must stream.

```rust
impl ProfileLab {
    fn start_sweep(&mut self, cx: &mut Context<Self>) {
        let engine = self.engine.clone();
        let opts = self.sweep_options.clone();

        cx.spawn(async move |this, cx| {
            let run = engine.start_analysis(AnalysisKind::ConfigSweep, opts).await?;
            let mut events = engine.subscribe(run);

            while let Some(ev) = events.next().await {
                this.update(cx, |lab, cx| {
                    match ev {
                        EngineEvent::Finding(f)   => lab.push_finding(f),
                        EngineEvent::Progress(p)  => lab.progress = p,
                        EngineEvent::Measurement(m) => lab.points.push(m),
                        EngineEvent::Done(v)      => lab.finish(v),
                    }
                    cx.notify();   // request re-render
                })?;
            }
            Ok(())
        })
        .detach_and_log_err(cx);
    }
}
```

Three rules:

- **Never block the foreground.** Builds, `perf` runs, model calls, and DWARF parsing all go
  to the background executor.
- **`cx.notify()` is the repaint request.** Forgetting it produces a UI that silently stops
  updating, which is the most common GPUI bug.
- **Every run is cancellable.** Hold the task handle; drop it on cancel. A config sweep the
  user cannot stop is a hostile tool.

### Data volume

A parsed binary's symbol table and DWARF index can be hundreds of megabytes. Do not clone it
into view state. Views hold `Arc<AnalysisResult>` and index into it; the table component
receives a row-range callback and pulls only what is visible.

---

## 5. Component mapping

What is bought versus what is built, per view.

| View | From `gpui-component` | Built by hand |
|---|---|---|
| App shell | Dock layout, tabs, splits, title bar, theme | Panel registry, layout persistence |
| Symbol table (Phase 1) | DataTable: virtual scroll, sort, filter, resize, context menu | Grouping model, byte formatting, provenance cell |
| Size Explorer | Toolbar, breadcrumb, context menu | **Treemap canvas** (§6.1) |
| Profile Lab | Chart/plot module | **Pareto scatter overlay + frontier line** (§6.3) |
| Source pane | Code editor, tree-sitter highlighting | Inline value decorations, gutter markers |
| Disassembly pane | Code editor as substrate | **x86 highlighter, address gutter, scroll sync** (§6.4) |
| Flamegraph | — | **Full custom canvas** (§6.2) |
| Findings Inspector | List, accordion, badge, markdown, dialog | Provenance visual language (§6.5) |
| Agent Transcript | List, accordion, markdown, code block | Step card, replay control |
| Timeline (Phase 4) | Slider, scroll | Track rendering, watchpoint markers |
| Agent panel | List, accordion, markdown, code block, dialog | Reasoner picker, transcript cards, permission prompts, cost meter |
| Settings | Form, select, switch, input | Provider probe, keyring access, ACP agent discovery |

Roughly two-thirds bought. The custom third is the part that makes the product distinctive,
which is the correct place for hand-written code to live.

---

## 6. Custom-painted views

### 6.1 Treemap

A binary produces tens of thousands of leaves. GPU-painted quads handle this comfortably; the
constraint is layout computation, not rendering.

**Algorithm:** squarified treemap (Bruls, Huizing, van Wijk). Produces near-square rectangles,
which matters because aspect-ratio distortion makes area comparison unreliable, and area
comparison is the entire point.

**Structure:**

```rust
struct TreemapNode {
    id: NodeId,
    label: SharedString,
    bytes: u64,
    category: SizeCategory,   // drives colour
    children: Vec<TreemapNode>,
}

struct TreemapLayout {
    rects: Vec<(NodeId, Bounds<Pixels>)>,   // computed in prepaint
    depth_of: HashMap<NodeId, u8>,
}
```

**Paint:**

```rust
canvas(
    // prepaint: compute layout once per frame at known bounds
    move |bounds, _window, _cx| squarify(&tree, bounds, max_depth),
    // paint: emit quads
    move |_bounds, layout: TreemapLayout, window, cx| {
        let theme = cx.theme();
        for (id, rect) in &layout.rects {
            let node = tree.get(*id);
            window.paint_quad(quad(
                *rect,
                px(2.),                                  // corner radius
                theme.category_color(node.category),
                px(1.),                                  // border width
                theme.border,
                Default::default(),
            ));
            if rect.size.width > px(48.) && rect.size.height > px(16.) {
                paint_label(window, rect, &node.label, theme.on_surface);
            }
        }
    },
)
```

**Performance discipline:**

- Cache the layout, keyed by `(tree_version, bounds, focus_node, depth_limit)`. Recomputing
  squarification every frame during a resize drag will be visible.
- Cull below a pixel threshold. Rectangles under roughly 2×2 px are aggregated into a
  synthetic "other" node rather than painted individually; they convey nothing and cost
  everything.
- Cap the depth by available pixels rather than a fixed number.
- Hit testing is a spatial index over `layout.rects`, not a linear scan, once you exceed a
  few thousand rectangles.

**Diff mode** paints a second quad pass with a diverging colour scale over the delta, and
adds a hatched fill for nodes present in only one of the two builds.

### 6.2 Flamegraph

Layout is trivial — a tree where each node's width is proportional to its sample count and
depth sets the row. Painting is the same quad-and-label loop.

The Lodestone-specific part is **inline frame expansion**. A physical stack frame in an
optimized Rust binary may represent six logical frames after inlining. The flamegraph must
show the logical structure, since "this frame did not physically exist" is exactly the
confusion the product exists to resolve. Frames recovered from `DW_TAG_inlined_subroutine`
are painted with a distinct treatment — lighter fill, dotted upper border — so the physical
versus logical distinction stays visible rather than being silently smoothed over.

Differential mode colours by delta against a baseline profile.

### 6.3 Pareto scatter

`gpui-component` ships charts, which may cover this outright. If its scatter support is not
sufficient, this is the smallest custom view in the product: a few hundred points, each a
quad or circle, plus a polyline for the frontier and an axis pass. Evaluate the built-in
first; do not write it speculatively.

Interaction is what matters more than rendering. Hovering a point previews its flags; clicking
selects it and populates the Inspector; rejected configurations paint greyed with a marker
for the failing gate.

### 6.4 Source and disassembly panes

Both use the code editor component as substrate, in read-only mode.

**Source pane** needs inline decorations at the faulting line showing reconstructed variable
values, and gutter markers for findings. Check whether the editor's decoration API is
sufficient; if not, an overlay element positioned from the editor's line-position mapping is
the fallback.

**Disassembly pane** needs a highlighter for x86 assembly. Options in order of preference:
the tree-sitter x86 grammar if the editor accepts an arbitrary grammar, otherwise a small
hand-written token classifier — mnemonic, register, immediate, memory operand, symbol
reference — which is a genuinely small amount of work for AT&T or Intel syntax and gives full
control over rendering resolved call targets as clickable symbols.

**Scroll synchronisation** is driven by the DWARF line table, not by proportional position.
Selecting a source line highlights every address range that maps to it — often several,
non-contiguous, after optimization. That visual — one source line lighting up four scattered
instruction blocks — is one of the clearest demonstrations of what the tool does, and is
worth building carefully.

### 6.5 The provenance visual language

`DESIGN.md` P2 requires that a claim never appears without its evidence. In GPUI this is one
shared component used everywhere:

```rust
#[derive(IntoElement)]
struct ProvenanceBadge { provenance: Provenance, confidence: Confidence }
```

- **Measured** (deterministic tool output) — filled marker, neutral hue
- **Derived** (our own heuristic) — half-filled marker
- **Inferred** (model output) — outlined marker, distinct hue

Enforce this by construction: the Inspector renders `Vec<Evidence>` through a component that
always emits a badge. There is no code path that renders a claim without one. Clicking a badge
opens the verbatim tool output with its command line and digest.

Theme colours come from `gpui-component`'s semantic theme system rather than literals, so
light mode and high-contrast variants work without a second pass.

---

## 7. Version pinning and upgrade policy

GPUI is pre-1.0 with frequent breaking changes. Treat that as a scheduled cost.

1. **Pin exact versions.** `=0.5.1`, not `^0.5`. Commit `Cargo.lock`. Prefer `gpui-kit` so
   the framework/component version pair is managed as a unit.
2. **Isolate the surface.** All GPUI usage lives in `lode-gui`. No other crate imports it.
   Every custom widget goes in `lode-gui::widgets`, so a framework API change touches one
   module rather than every view.
3. **Schedule upgrades.** One deliberate upgrade per phase, at the *start* of the phase, never
   mid-feature. Upgrading GPUI and debugging DWARF in the same week is how a solo project
   loses a month.
4. **Screenshot-test the custom views.** Framework rendering changes are otherwise silent.
5. **Keep the fork escape hatch documented** — the `[patch.crates-io]` block, commented out,
   in `Cargo.toml`.

---

## 8. Testing

GPUI ships a `#[gpui::test]` macro and a `TestAppContext` that can simulate platform input,
which makes view logic genuinely testable rather than theoretically testable.

| Layer | Approach |
|---|---|
| Layout algorithms | Plain unit tests. `squarify()` is a pure function: no GPU, no context. Property-test that areas sum correctly and aspect ratios stay bounded. |
| View state | `#[gpui::test]` with a fake `Engine` returning scripted `EngineEvent`s |
| Interaction | `TestAppContext` simulated input; assert selection and Inspector state |
| Custom painting | Screenshot comparison on a fixed fixture, tolerance-based |
| Provenance invariant | Assert that every rendered finding element contains a badge — this is the P2 guarantee, and it deserves a test that fails loudly |

Keep layout algorithms as free functions taking data and returning geometry. They are then
testable, benchmarkable, and reusable by a future SVG or PNG exporter for sharing a
treemap in a bug report.

---

## 9. Build order

The GUI is the only shipped surface, so it can no longer follow a released CLI. The ordering
discipline moves inside each phase: **engine first, proven against the headless harness, then
UI.** The GUI never leads a phase, and engine work must never block on UI work.

**Phase 0 — the shell (~3 weeks, after the Phase 0 engine passes the headless harness)**
Window, theme, dock layout, project view, Profile Lab reading `lodestone.json`, and the
Findings Inspector. Deliberately no custom painting: use the built-in chart, prove the
`Engine` trait and the streaming event model, and learn the framework on easy views.

**Phase 1 — the first real view (~3 weeks)**
Symbol DataTable, then the treemap. This is where you write squarification and learn
`canvas`, `paint_quad`, and hit testing. The table gives a fallback view if the treemap takes
longer than expected.

**Phase 2 — the debugger shell (~4 weeks)**
Stack pane, source pane, disassembly pane, scroll sync. The hardest UI work in the project,
and correctly placed after two phases of GPUI experience.

**Phase 1 also — the Agent panel (~2 weeks)**
Reasoner picker, transcript, evidence links, cost meter. Built against Mode A only. The ACP
work in Phase 2 reuses this same transcript view, because the internal event enum deliberately
mirrors ACP's `session/update` shape (`DESIGN-AI.md` §9.2).

**Phase 2 also — permission prompts (~1 week)**
Inline approval UI wired to the trust ladder.

**Phase 3 — flamegraph (~2 weeks)**
Mostly a variation on the treemap machinery, plus inline-frame treatment.

**Phase 4 — timeline (~3 weeks)**
Scrubber, watchpoint markers, and wiring every pane to a replay position.

---

## 10. Open questions

1. **Does the code editor component expose enough decoration API** for inline variable values
   at a specific column? Prototype this in week one of Phase 2, not week four. If not, the
   overlay fallback needs designing.
2. **Are the built-in charts sufficient for the Pareto scatter**, including hover, selection,
   and a frontier polyline? Determines whether §6.3 is bought or built.
3. **Can the editor accept an arbitrary tree-sitter grammar** for x86 assembly, or is a
   hand-written highlighter required?
4. **Wayland versus X11 behaviour** for file dialogs, clipboard, and multi-window. The Linux
   platform layer supports both, but they should be smoke-tested separately from day one
   rather than discovered later.
5. **How large can the DataTable go in practice?** Documented for very large row counts;
   confirm on a real symbol table of a large binary before committing the Phase 1 design.
6. **Accessibility.** GPUI's screen reader story is weaker than the web's. Decide whether
   this is an accepted v1 limitation and say so publicly, rather than leaving it unstated.

---

## Appendix — Prototype before committing

Before Phase 0's GUI work begins, build one throwaway spike, in about two days:

- A window with a dock layout and two panels
- A DataTable of 100,000 synthetic rows, scrolled hard
- A `canvas` element painting 20,000 quads with hit testing
- One background task streaming events into a view

**This spike is now mandatory rather than advisory** (PRD R4). With the CLI removed, the GUI is
on the critical path from week one, and a framework that disappoints on your hardware is a
plan-changing discovery you want in week one rather than month four.

If all four feel good, the architecture in this document holds. If the table or the canvas
disappoints on your hardware, you learn it in two days rather than in month four. This spike
is cheap insurance against the one risk that would be expensive to discover late.
