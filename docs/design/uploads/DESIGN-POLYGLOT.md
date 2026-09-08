# Binmap — Polyglot Design (Rust, C#, JS/TS)

> How the application changes when the target is a company using Rust, C#, and TypeScript
> rather than Rust alone. Written to inform UI/UX design in progress.
>
> Supersedes PRD non-goal N6. Replaces the earlier language survey, which covered
> languages Binmap is not going to support.

---

## 1. The honest starting position

An earlier survey of this question put two of your three languages outside the product:

- **C#** — only NativeAOT qualifies; CoreCLR with the JIT was Tier 4
- **JS/TS** — Tier 4 entirely

That assessment was correct **for the product as specified**, because the specification assumed
the artifact is an ELF binary and the mapping is DWARF. Held to that definition, JavaScript has
nothing to analyze.

But the definition is narrower than the idea. The actual thesis is not "binary analysis." It
is:

> **A compiled or bundled artifact, a mapping back to source, measurements of size and time, a
> searchable build configuration space, and a verifier that proves a change helped.**

Every one of those exists for TypeScript. Bundles are artifacts. **Source maps are JavaScript's
DWARF** — the same job, mapping generated output back to authored source, through the same
kinds of transformations. Bundler options are a configuration space with the same shape as
Cargo profiles. `vitest` is a test suite. Gzipped bytes are a measurement.

So the product generalizes. What does not generalize is the *implementation*, and being precise
about which parts are shared and which are per-language is the whole content of this document.

**The one thing I will not soften:** three languages, solo, is how PRD R1 kills this project.
The plan below only works if you hold one line absolutely — **the views are shared, the
backends are adapters.** The moment C# gets a bespoke panel that TypeScript doesn't have, you
are maintaining three products.

---

## 2. The reframe: artifact ↔ source bridge

| Concept | Rust / C++ / NativeAOT | JS/TS | C# CoreCLR |
|---|---|---|---|
| Artifact | ELF binary | Bundle (JS/CSS/wasm chunks) | Assemblies (.dll) + deps.json |
| Source mapping | DWARF | Source maps | Portable PDB |
| Unit of size | Symbol | Module / chunk | Type / method |
| "Monomorphization" analogue | Generic instantiations | Duplicate deps, barrel re-exports, polyfills | Generic instantiations over value types |
| Config space | Cargo profile matrix | Bundler + target + minifier options | Trimming, R2R, AOT, single-file |
| Runtime measurement | `perf` samples | V8 CPU profile | EventPipe / `dotnet-trace` |
| Failure artifact | Core dump | Minified stack + source map | Managed dump (ClrMD) |
| Verifier | build + test + bench + sanitizers | build + test + bundle size + Lighthouse/bench | build + test + bench |

Read that table column by column and the product is the same product. Read it row by row and
every cell needs different code. That tension is what the capability model (§3) resolves.

---

## 3. The core architectural change: a capability model

The current design assumes one project type, so every view always applies. That assumption
breaks immediately: a TypeScript project has no disassembly, a CoreCLR project has no core
dump, a Rust project has no gzip size.

**Replace "the project" with "targets that declare capabilities."**

```rust
pub struct Target {
    pub id: TargetId,
    pub name: String,                    // "api-gateway", "web-frontend"
    pub kind: TargetKind,                // RustBin | RustLib | DotNetAot | DotNetJit | WebBundle
    pub root: PathBuf,
    pub capabilities: Capabilities,
}

bitflags! {
    pub struct Capabilities: u32 {
        const SIZE_ATTRIBUTION   = 1 << 0;  // all
        const CONFIG_SWEEP       = 1 << 1;  // all
        const SOURCE_MAPPING     = 1 << 2;  // all (DWARF / source maps / PDB)
        const INSTANTIATION_BLOAT= 1 << 3;  // Rust, NativeAOT, (JS: duplicate deps)
        const COMPRESSED_SIZE    = 1 << 4;  // web only — gzip/brotli
        const DISASSEMBLY        = 1 << 5;  // native only
        const CORE_DUMP          = 1 << 6;  // native only
        const MANAGED_DUMP       = 1 << 7;  // .NET only
        const SAMPLE_PROFILE     = 1 << 8;  // all, different sources
        const REPLAY_DEBUG       = 1 << 9;  // native + rr only
        const LOAD_TIME_METRICS  = 1 << 10; // web only — parse/eval/TTI
    }
}
```

Everything downstream reads capabilities: which dock panels exist, which tools the agent
registry exposes, which verifier gates apply, which findings can be produced.

**This is the single most important thing to get into your UI/UX design now**, because it
changes the shell from "a fixed set of panels" to "a panel set derived from what you opened."
Retrofitting that is expensive.

### 3.1 Trait seams, revised

`DESIGN.md` §3.2 defined two traits. Polyglot needs four.

```rust
pub trait ArtifactReader   { /* parse the built thing, enumerate units + sizes */ }
pub trait SourceMapper     { /* artifact location ↔ source location, through inlining */ }
pub trait BuildSystem      { /* discover, enumerate config axes, build, test */ }
pub trait RuntimeModel     { /* frames, threads, heap — the seam a demangler cannot cover */ }
```

`SourceMapper` is the one that unifies DWARF, source maps, and Portable PDB behind a single
interface. Its method signatures should be written against the *union* of what those three can
do, and each implementation reports what it cannot answer rather than lying.

### 3.2 Non-Rust ecosystems need helper processes

Practical consequence that affects packaging and the Environment panel: some of this cannot be
done from Rust.

| Need | Approach |
|---|---|
| Bundler introspection (esbuild metafile, webpack stats, vite) | `binmap-bridge-node` — a small Node helper Binmap spawns |
| Source maps | Native Rust (`sourcemap` crate). No bridge needed. |
| Managed dumps (ClrMD) | `binmap-bridge-dotnet` — ClrMD is a .NET library; no Rust equivalent exists |
| Portable PDB | Documented format; a Rust reader is feasible, or use the bridge |
| V8 CPU profiles | JSON. Native Rust. |
| `dotnet-trace` / EventPipe | `.nettrace` format; bridge is simpler |

Each bridge is a dependency the user must have installed, which means each is a row in the
Environment panel with an exact fix. Design that panel to be grouped by target type, not one
flat list, or a Rust-only user will see a wall of irrelevant .NET warnings.

---

## 4. Per-language backends

### 4.1 Rust — unchanged
Everything in `DESIGN.md`, `TOOLING-BINARY.md`, and `TOOLING-HARNESS.md` stands. It remains the
reference implementation and the deepest capability set. Build it first; it is the only one that
exercises every capability flag.

### 4.2 JS/TS — cheaper than it looks, and worth doing early

**This is the surprise of this document: TypeScript support may be the second-cheapest thing
you can build, not the hardest.**

No DWARF. No unwinding. No disassembly. No core dumps. No `rr`. The entire hard half of the
project is absent, and what remains maps cleanly onto machinery Phase 0 and Phase 1 already
need.

**Size attribution.** esbuild's `--metafile` and webpack's `stats.json` both emit structured
JSON giving per-module input and output bytes. Where those are unavailable, source maps alone
are enough — walk the mappings, attribute each output byte range to an original source file.
That is `source-map-explorer`'s entire technique and it is a few hundred lines against the
`sourcemap` crate.

**The instantiation-bloat analogue is real and painful.** JavaScript's version of "twelve copies
of the same generic" is:

- The same dependency present at multiple versions (`lodash@4` and `lodash@3` both bundled)
- Barrel files (`export * from './everything'`) defeating tree-shaking
- Side-effectful modules that the bundler cannot drop
- Polyfill and transpilation cost from a conservative `browserslist` target
- A library imported for one function but pulled in whole

Each is detectable deterministically, each has a known remedy, and each is exactly the kind of
finding the AI layer is good at explaining and proposing a patch for. This is a **differentiated
feature**, not a `webpack-bundle-analyzer` reimplementation, because those tools stop at the
treemap.

**The config sweep ports almost perfectly.** The axes: minifier choice and settings, `target`
ES level, tree-shaking and `sideEffects` configuration, code-splitting strategy, source-map
mode, `browserslist`, compression level. Sweep, build, measure gzipped and brotli bytes, run
`vitest`, verify.

**Compressed size is the metric that matters**, and it is not proportional to raw size — a
change can reduce raw bytes and increase gzipped bytes by disrupting compression locality. That
is a genuinely counterintuitive result your tool can surface and no one currently measures
during a sweep. Show raw, gzip, and brotli side by side.

**Performance.** V8 CPU profiles (`node --cpu-prof`, or Chrome DevTools Protocol) are JSON with
line and column positions. Map them through source maps back to TypeScript, and the flamegraph
view works unchanged.

**Crash analysis** becomes stack-trace de-minification: a production error stack against the
source map, plus the surrounding source. Far shallower than a core dump, but a real and
frequently felt pain.

### 4.3 C# — two backends, not one

Be explicit in the UI about which is in play, because the capability difference is dramatic.

**NativeAOT** — the strong case. Native ELF, size is a headline concern for the audience,
value-type generics are genuinely monomorphized, and the ILC configuration space is a rich,
folklore-driven set of size/functionality trade-offs that nobody currently measures
systematically: trimming mode, optimization preference, invariant globalization, resource-key
substitution, event-source and stack-trace support, reflection metadata. That is a Phase 0
sweep with real, unexplored search space.

**Correction since this section was drafted:** do not reverse-engineer NativeAOT's symbol
mangling. ILC can emit its own size accounting (`.mstat`) and, more importantly, its dependency
graph (`.dgml`) explaining *why* each item survived trimming — a question the native Rust
backend cannot answer at all. See `TOOLING-DOTNET.md` §2. This also de-risks the DWARF
assumption: if ILC's DWARF turns out weak, C# loses crash analysis, not size attribution.

Trim and AOT warnings (`IL2xxx`, `IL3xxx`) are a second deterministic findings source with no
Rust or web equivalent — each names a specific site where reflection or dynamic code blocks
trimming. See `TOOLING-DOTNET.md` §5.

**CoreCLR (JIT)** — the weak case, and it needs a different engine rather than an adapter.
There is no static native artifact. What exists instead:

- **Deployment size analysis** at the assembly and type level, plus trimming analysis
  (`illink` warnings tell you what trimming *cannot* remove and why — that is a findings source)
- **A real config space** that is arguably more consequential than the native one:
  framework-dependent vs self-contained, `PublishTrimmed`, `PublishReadyToRun`,
  `PublishSingleFile`, and NativeAOT itself as one point in the space. "Should we go AOT?"
  becomes a measured answer rather than an argument.
- **Managed dumps** via ClrMD through the bridge — heap, threads, managed stacks
- **EventPipe traces** for profiling, plus `perf` with the perf-map for JIT'd frames

**Verdict: build NativeAOT support as a Rust-backend variant. Treat CoreCLR as a separate,
later, shallower backend.** And note the genuinely valuable framing — for a company with C#
services, "what would AOT actually save us, measured, with tests passing" is a question worth
the tool's existence on its own.

### 4.4 Everything else is out of scope

Three languages. Not four, not "and C++ later if it's easy."

C++ would be the natural fourth — Itanium demangling is mature, template bloat is the flagship
use case, and it reuses the entire native backend. It is recorded here and nowhere else, so
that it stays a decision rather than a temptation.

Everything else — Go, Java, Python, Swift, Zig — is **not on the roadmap**. The trait seams in
§3.1 mean none of them are *precluded*, and that is the only accommodation they get. Adding a
language is a phase of work, not a weekend, and PRD R1 is the risk most likely to end this
project.

If a contributor arrives wanting to add a backend, the answer is the four traits and a warning
about the 80/20 rule in §5.5.

---

## 5. UI/UX implications

The part that matters for the design work in progress.

### 5.1 The shell becomes multi-target

The biggest change. A polyglot company repo is not one crate; it is a dozen services and a
frontend. The current single-project shell does not survive contact with that.

```
┌──────────────────────────────────────────────────────────────────────┐
│ ▾ acme-platform @ a3f9c21     [Trust: Propose ▾]  [Claude Code ▾] ⚙ │
├──────────────┬───────────────────────────────────┬───────────────────┤
│ TARGETS      │                                   │  Findings         │
│              │       Main view                   │  Inspector        │
│ ▾ Rust  (4)  │                                   │                   │
│   api-gw   ● │                                   │                   │
│   indexer    │                                   │                   │
│ ▾ .NET  (3)  │                                   │                   │
│   billing ᴬᴼᵀ│                                   │                   │
│   reports ᴶᴵᵀ│                                   │                   │
│ ▾ Web   (2)  │                                   │                   │
│   console    │                                   │                   │
│   admin      │                                   │                   │
└──────────────┴───────────────────────────────────┴───────────────────┘
```

Design decisions this forces:

- **A target sidebar**, grouped by kind, with a capability badge (AOT vs JIT is a capability
  difference the user must see at a glance).
- **Multi-select for comparison.** "Which of our services grew this week" is the question a
  polyglot org actually has, and it is a portfolio view that does not exist in the current
  design. Arguably your strongest internal-adoption feature.
- **The current target drives the visible panel set.** Selecting the TypeScript console hides
  Disassembly and Crash Analysis and shows Load-Time Metrics.

### 5.2 Panels appear and disappear — handle this deliberately

The worst outcome is a greyed-out panel with no explanation. Three rules:

1. **Hide, don't disable**, for capabilities the target fundamentally lacks. A TypeScript
   project should not display a Disassembly tab at all.
2. **Disable with a reason**, for capabilities the target *could* have but currently doesn't —
   "Crash analysis needs a core dump. None loaded." with a button to load one. The distinction
   between "impossible here" and "not yet" must be visible.
3. **The panel set is part of saved layout state**, so switching targets and switching back
   restores what you had.

`gpui-component`'s dock supports serializable layouts, so this is configuration rather than
new machinery.

### 5.3 Size is now three numbers

Rust reports bytes. Web reports raw, gzip, and brotli, and the ordering between two builds can
differ across those three. Any UI element showing "size" needs a metric selector, and the
treemap needs to recolour when it changes.

Make the selector global and persistent, not per-panel. A user comparing a Rust service and a
web bundle should not have to reason about which number each panel is showing.

### 5.4 Terminology must go neutral now

Cheap today, expensive after the copy is written and the types are named.

| Don't say | Say |
|---|---|
| Binary | **Artifact** |
| Symbol | **Unit** (a symbol, a module, a type — whatever the backend enumerates) |
| Section | **Region** |
| Binary size | **Artifact size** |
| Debug info | **Source mapping** |
| Crash | **Failure** (a core dump, a managed dump, or an error stack) |
| Monomorphization | **Duplication** in shared UI; the specific term only inside a Rust-specific finding |

Rename the types too — `Location::Address` should not imply a machine address, since a source
map position and an IL offset both live there.

### 5.5 What is genuinely shared

Reassuring, and the reason this is tractable at all:

- **Treemap** — works identically for symbols, modules, and types. Zero change.
- **Pareto scatter** — a config sweep is a config sweep. Zero change.
- **Findings Inspector** — provenance and evidence are language-agnostic. Zero change.
- **Flamegraph** — perf samples, V8 profiles, and EventPipe all reduce to a weighted call tree.
- **Agent panel** — completely unchanged. Same registry, more tools.
- **Verifier** — same gates, different commands behind `BuildSystem`.

Roughly **80% of the UI is shared.** The native-only views (Disassembly, Timeline) and the
web-only views (Load-Time Metrics) are the exception, and they are the last things you build.

### 5.6 UX risk: the shallow-target trap

A user who opens a CoreCLR service and finds three working panels out of nine will conclude the
product is broken, not that their target is limited. Guard against it:

- State capability up front, on target selection, before they go hunting
- Give shallow targets a genuinely good default view rather than an emptier version of the deep
  one
- Never show a Rust-shaped empty state to a TypeScript user

---

## 6. Revised phase plan

Sequenced for a company that needs all three, by a solo maintainer.

| Phase | Scope | Weeks |
|---|---|---|
| **0** | Rust config sweep + shell + Profile Lab + Findings Inspector | 8-10 |
| **0.5** | **Capability model + multi-target shell.** Refactor before it is expensive. | 2-3 |
| **1** | Rust size attribution, monomorphization, treemap, Mode A AI | 9-12 |
| **1.5** | **JS/TS backend.** Source maps, bundle attribution, bundler sweep, duplicate-dep findings. Reuses every view from Phase 1. | 5-7 |
| **2** | Rust crash analysis (DWARF) + ACP | 11-14 |
| **2.5** | **C# NativeAOT** as a native-backend variant + the ILC config sweep | 4-6 |
| **3** | Perf attribution — Rust `perf`, then V8 profiles | 7-9 |
| **3.5** | C# CoreCLR: deployment sizing, trimming analysis, the AOT-migration question | 6-8 |
| **4** | `rr` replay (Rust/native only) | open |

**Two deliberate choices in that ordering.**

**Phase 0.5 exists because refactoring to multi-target after Phase 1 is far more expensive than
before it.** Two to three weeks now saves considerably more later, and it lands while the
codebase is still small.

**JS/TS at 1.5, before crash analysis.** It is cheap, it reuses everything Phase 1 built, and it
triples your company's internal user base at a fraction of Phase 2's cost. If internal adoption
matters — and for a solo project it is often what sustains motivation — this is the highest
value-per-week block on the list.

---

## 7. What this changes in the existing docs

| Document | Change |
|---|---|
| `PRD.md` | N6 lifted. Add C#/JS personas. Capability model in requirements. Timelines extend by ~18 weeks. |
| `DESIGN.md` §3.2 | Two traits become four. `Location` types go artifact-neutral. |
| `DESIGN.md` §4 | `Finding`, `Location`, `Evidence` lose native-specific assumptions. |
| `DESIGN-GUI.md` | Multi-target shell, capability-driven panels, size-metric selector. |
| `DESIGN-AI.md` | Tool registry filtered by capability. Otherwise unchanged. |
| New | `TOOLING-WEB.md` and `TOOLING-DOTNET.md` — written. |

---

## 8. Risks specific to this decision

| # | Risk | Mitigation |
|---|---|---|
| P1 | **Three languages, solo — the R1 scope risk, tripled.** | The 80/20 shared-view rule is a hard architectural constraint, enforced in review. Any per-language UI needs written justification. |
| P2 | **Shallow backends damage the product's reputation.** A user judges by the weakest target they open. | Capability transparency (§5.6). Ship a backend only when it has a genuinely good default view. |
| P3 | **Bridge processes multiply install friction.** Node and .NET SDK become dependencies. | Bridges are lazy — only required when a target of that type is opened. Environment panel grouped by target type. |
| P4 | **Platform assumption breaks.** A company using C# and TypeScript very likely has Windows and macOS developers, but the product is Linux x86-64. | **See §9. This is the most under-examined decision in the plan.** |
| P5 | **Internal tool versus open source pull in different directions.** Your company wants the three languages it uses; the community wants depth in one. | Decide which this is for. The answer changes the roadmap, not just the marketing. |
| P6 | **NativeAOT DWARF quality on Linux is assumed, not verified.** | Spike it in an afternoon before committing Phase 2.5. |

---

## 9. The platform question you now have to answer

The PRD assumes Linux x86-64, justified because `rr` only runs there.

That justification covers Phase 4 alone. Phases 0 through 3 do not need `rr`. And a company
using C# and TypeScript almost certainly has developers on Windows and macOS — quite possibly
the majority of the people you would want using this.

The pieces:

- **GPUI** supports macOS and Linux. Windows support exists in Zed but the standalone crate's
  Windows story should be verified before planning around it.
- **JS/TS backend is fully cross-platform.** No native dependencies at all.
- **.NET backend is fully cross-platform.**
- **Rust size and config sweep** are cross-platform in principle; the artifact readers are not
  (Mach-O and PE alongside ELF, and `object` already handles all three).
- **Core dumps and `rr`** are Linux-only, and always will be.

So the honest shape is: **most of the polyglot product is cross-platform, and the native
debugging half is not.** That is a capability difference, and you already have a capability
model to express it.

**This deserves a decision before UI/UX work goes much further**, because "Linux-only tool" and
"cross-platform tool with Linux-only debugging" are different products with different navigation,
different empty states, and different install stories. I would not let this one drift.
