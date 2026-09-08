# Binmap Design System

A design system for **Binmap**, the desktop application described in the attached product
documents. Its palette and typographic character were **extracted from the supplied SilverKey
Technologies logo and poster**; the logo itself is not used anywhere in the system or the product.

---

## 1. Context

### The brand

**Extracted, not applied.** The supplied material — a logo lockup (`uploads/sk-logo.svg`) and
one event poster (`uploads/image.png`, a talk announcement: "BUILDING A BACKEND LIKE AN
OPERATING SYSTEM", Monzer Omar, Senior backend engineer) — was used as a *source* for colour
and typographic character only. Neither the mark nor the wordmark appears in this design system
or in any recreated screen. What was taken from those two files:

- Two colours, sampled directly from the mark: **slate `#3D4658`** (the wordmark and the "S")
  and **orange `#FF6F15`** (the "K" chevron).
- A geometric sans wordmark, and a poster set in heavy geometric caps on white with flat
  grey-blue line illustrations.
- No gradients, no glows, no ornament anywhere in the source material.

There is no brand guidelines document, no website, and no codebase in the supplied sources.
Where this system had to make a call the reasoning is written down beside it.

### The product

**Binmap** (CLI binary `binmap`) — "a source-aware binary analysis tool for Rust." A
**desktop application** that reads a project's source and its compiled binary at the same
time and answers three questions: why is this wrong, why is this slow, why is this so big.

- Deterministic analysis engine with an **optional** language-model layer. `--no-ai` is a
  first-class mode.
- Every model claim is displayed next to the tool output that supports it.
- Local by default. Nothing is uploaded.
- Autonomy is one visible dial with four settings: Observe → Propose (default) → Tune →
  Autonomous.
- Target platform v1: Linux x86-64, Rust only. Pre-alpha; nothing ships yet.

**Surfaces in scope**

| Surface | Status here | Source |
|---|---|---|
| Desktop app (Tauri 2 + TypeScript frontend) | Recreated — `ui_kits/binmap-desktop/` | DESIGN §6, §7; PRD §10.1 |
| CLI (`binmap tune / size / analyze / profile / ci`) | Recreated — `ui_kits/binmap-cli/` | README §Usage; PRD §10.2 |
| CI mode (`binmap ci`) | Covered inside the CLI kit | PRD §10.3 |
| Marketing website, docs site, slide template | **Do not exist** in the sources — not invented | — |

### Sources given

| Source | Path as supplied | Notes |
|---|---|---|
| Logo lockup | `uploads/sk-logo.svg` | 167×38, two-colour. Read for its two colours; not used as an asset. |
| Event poster | `uploads/image.png` | 1:1 social/event card. Sole typographic reference. |
| Product README | `uploads/README.md` | Scope, trust tiers, usage, non-goals. |
| PRD | `uploads/PRD.md` | Personas, phases, requirements F0–F4, UI requirements U1–U12, risks. |
| Technical design | `uploads/DESIGN.md` | Architecture, `Finding`/`Evidence` model, agent design, GUI §6–§7. |

No Figma file, repository, or design system was provided. Screen recreations are built from
the prose and ASCII layout diagrams in DESIGN §7.1–§7.3 — the closest thing to a
specification that exists — not from screenshots.

---

## 2. Index

**Root**

| File | What it is |
|---|---|
| `readme.md` | This document: context, content fundamentals, visual foundations, iconography, index. |
| `SKILL.md` | Agent Skills front matter, for use as a portable skill. |
| `styles.css` | The single entry point consumers link. `@import` lines only. |
| `thumbnail.html` | Homepage tile. |
| `tokens/` | `fonts` · `colors` · `typography` · `spacing` · `radius` · `elevation` · `motion` · `semantic` · `theme-light` · `base` |
| `guidelines/` | 23 foundation specimen cards (Brand, Colors, Type, Spacing). |

**Components** — 40, grouped by concern. Each directory has one `@dsCard` HTML showing its states.

| Group | Components |
|---|---|
| `components/core/` | `Icon` · `Button` · `IconButton` · `Input` · `Select` · `Checkbox` · `Switch` · `SegmentedControl` · `Tabs` · `Panel` · `Dialog` · `Tooltip` · `Badge` · `Tag` |
| `components/data/` | `DataTable` · `MetricStat` · `DeltaValue` · `ProgressBar` · `CodeBlock` |
| `components/analysis/` | `ProvenanceBadge` · `ConfidenceBadge` · `EvidenceItem` · `FindingCard` · `ProposalBlock` · `GateResult` · `TrustTierControl` · `TranscriptStep` · `StackFrameList` |
| `components/charts/` | `Treemap` · `ParetoScatter` · `Flamegraph` |
| `components/chrome/` | `TitleBar` · `NavRail` · `StatusBar` · `CommandPalette` · `EmptyState` |
| `components/agents/` | `AcpAgentCard` · `ProviderCard` · `ApiKeyField` · `ModelPicker` |

**UI kits**

| Kit | Entry | Screens |
|---|---|---|
| `ui_kits/binmap-desktop/` | `index.html` | Project view · Size Explorer · Profile Lab · Crash Analysis · Perf Lab · Agent Transcript · Agents & backends, inside the full app frame |
| `ui_kits/binmap-cli/` | `index.html` | `binmap tune` · `binmap size` · `binmap analyze` · `binmap ci` |

### Component inventory: how it was decided

No source defined a component library, so the inventory was derived from the interface
requirements rather than from a generic checklist. `components/analysis/` and
`components/charts/` exist because PRD U2–U7 and DESIGN §7.1–§7.2 name those exact surfaces;
`components/core/` is the smallest set of primitives those surfaces need.

**Intentional additions** (not named in the sources, added with reason):

- `Icon` — a wrapper over the substituted Lucide set, so glyph size and stroke weight stay
  consistent and no screen hand-rolls SVG.
- `SegmentedControl` — DESIGN §7.2 requires "grouping switchable between crate, module,
  generic origin and section" and diff/absolute modes; this is that control.
- `Tag` — the treemap legend and active filters need a chip; DESIGN describes the legend but
  not its form.
- `components/agents/` — the four backend-configuration components (§7). PRD U10 names a
  settings surface for the model layer but never describes it; the shape here follows the
  product's own rule that the backend is always visible and never silently changed.

**Deliberately absent:** Avatar, Toast, Breadcrumb, Accordion, Pagination, DatePicker. Nothing
in the sources calls for them, and Binmap has no accounts, no notifications feed and no
paginated lists.

### Agent and model backends

The product treats the model layer as swappable and optional, so configuration is a first-class
screen rather than a preferences afterthought. Two kinds of backend, kept apart because their
trust properties differ:

**1. External coding agents over ACP.** The Agent Client Protocol is Zed's open standard —
JSON-RPC 2.0 over stdin/stdout — and it is how Claude Agent, Codex CLI, Gemini CLI, Copilot CLI,
Kimi CLI, OpenCode, Goose and anything else in the ACP registry connect. Binmap is the
*client*: it exposes the project's files, the terminal and its own deterministic analysis tools,
and it owns the permission boundary. An ACP agent authenticates and bills itself, so
`AcpAgentCard` shows handshake state and negotiated capabilities and never asks for the agent's
key. Adapters are named exactly: `npx @zed-industries/claude-code-acp`, `codex-acp --stdio`,
`gemini --experimental-acp`.

**2. Keyed model providers.** Anthropic and OpenAI over their first-party APIs; z.ai (GLM),
Moonshot (Kimi) and DeepSeek over their OpenAI-compatible endpoints; a local llama.cpp server;
and a user-defined OpenAI-compatible provider for vLLM, Ollama, LM Studio, OpenRouter, Together,
Fireworks, Groq or an Azure/Bedrock gateway. Keys live in the OS keyring and are shown only as
fingerprints — `ApiKeyField` cannot read a stored key back out.

**Model lists are newest-first, always.** `ModelPicker` sorts every group by its `released`
date descending and badges the newest entry, so a provider's latest model is on top without the
caller sorting anything. Only supported models are listed; retired IDs (`kimi-k2.5`,
`moonshot-v1`, Claude 3.x, GPT-4.x) are omitted, and models that exist but are not callable on
the account — Claude Mythos, invitation-only through Project Glasswing — render disabled with
`invite only`. The offline catalog lives in `ui_kits/binmap-desktop/model-catalog.js` and was
checked against provider documentation on **6 September 2026**; at runtime Binmap refreshes it
from each provider's `/v1/models`. It covers Claude Fable 5.1 / Opus 5 / Sonnet 5 / Opus 4.8 /
Sonnet 4.6 / Haiku 4.5, GPT-6 Astra / 5.6 Sol · Terra · Luna / 5.5 / 5.4 / 5.3-Codex / 5-Codex,
GLM-5.3 · 5.3-Flash · 5.2 · 5-Turbo · 4.7, Kimi K3 · K3 Swarm Max · K2.7 Code · K2.6, and
DeepSeek V4 Pro · V4 Flash · V4 Flash Vision.

Because a model list dates fast, treat that file as data with a timestamp, not as part of the
visual system: refresh it, and the picker keeps its ordering guarantees.

---

## 3. Content fundamentals

The product documents are the voice sample, and it is a distinctive one. Copy in this system
matches it.

**Register: engineer to engineer, no marketing.** Short declaratives. Concrete nouns. The
documents open with "Binmap reads your source and your compiled binary at the same time"
— not "empowers developers to unlock insight." Match that.

**Say the mechanism, not the benefit.**

- Yes: "panic = abort removes 148 KB of unwinding tables."
- No: "Optimize your binary with one click."

**Second person for the user's things, third person for the tool's.** "Your binary", "your
crate", "your machine" · "Binmap measures", "the engine computes", "the verifier rejected
it". Never first person; the tool does not say "I". No "we".

**State limits as plainly as capabilities.** The source documents lead with
"Status: pre-alpha. Nothing works yet." and keep a non-goals list longer than the goals list.
UI copy inherits this: empty states say what is missing, errors name the gate that failed,
and a partial answer is labelled `Speculative` rather than dressed up.

**Numbers are exact and carry units.** `1,214,336 bytes`, not "~1.2 MB" — the abbreviated
form appears only where space forces it, and the exact number is one click away.
Thousands separators always. Deltas always signed: `−38.2%`, `+12.4%`. A result inside the
noise floor is written `+1.1% n.s.`, never rounded to zero and never coloured green.

**Casing.** Sentence case for everything readable: headings, buttons, labels, dialog titles.
UPPERCASE only for 11px eyebrows and table headers, with `.08em` tracking. Never title case.
Code identifiers keep their real casing verbatim — `opt-level=z`, `core::fmt`,
`DW_TAG_inlined_subroutine`, `TestsPass` — inside mono, never re-capitalized to look tidy.

**Command names are code.** `binmap tune`, `--no-ai`, `binmap.json` are always mono, always
exact. Empty states show the equivalent CLI command as a footnote, because the CLI ships
first and the audience lives in terminals.

**Teach in empty states.** DESIGN §7.3: "Empty states teach." A first-run Size Explorer
explains what a treemap of a binary means, in two or three sentences, because for many users
this is their first exposure to binary-level thinking.

**Provenance language is fixed vocabulary.** Three words, never synonyms:
**Measured** (a tool ran) · **Derived** (our own rule) · **Inferred** (a model said so).
Likewise the four confidence words: **Certain · High · Probable · Speculative**. And the four
tiers: **Observe · Propose · Tune · Autonomous**. Do not paraphrase these anywhere.

**Never the words** "AI-powered", "smart", "magic", "effortless", "seamless", "revolutionary".
The PRD makes this a risk mitigation, not a style preference: R3, "Never market as an AI
debugger." The naming criteria in Appendix C explicitly exclude "AI", "GPT", "Copilot" and
"smart".

**Emoji: never.** Not in the UI, not in the CLI, not in documentation. None of the source
material contains a single one. Status is carried by the provenance markers
(● ◈ ◆), the `✓`/`✗`/`!` glyphs in CLI output, and colour.

**Microcopy examples, verbatim from this system**

- Button: `Verify proposal` · `Apply to Cargo.toml` · `Show 47 omitted`
- Blocked action: `Tier 3 required to patch source`
- Gate row: `BenchmarkNotWorse { significance: 0.05 }` → `+1.1% n.s.` `inside noise floor (0.9%)`
- Model caveat: `This is a model claim. It cites 4 evidence items and is capped at Probable.`
- Warning: `dirty` on the project fingerprint, tooltip "Working tree is dirty — binary and
  source may not correspond."
- Truncation: `47 instructions omitted, call disassemble_range to see them` — never a silent cut.

---

## 4. Visual foundations

### 4.1 Colour

**Two brand colours and one long neutral ramp.** Slate `#3D4658` and orange `#FF6F15`, both
sampled from the mark. The slate hue (~222°) is extended into a twelve-step ramp that supplies
every surface, border and text colour; orange gets nine steps but is used at a fraction of
slate's area.

**Orange is rationed.** It marks exactly one thing at a time: the single primary action in a
view, the current selection, the focus ring, the active nav item, your-own-code in a treemap,
and — deliberately — **model-inferred provenance**. Assigning the brand accent to "a machine
said this" is the strongest available signal that the claim needs reading differently. Orange
is never a page background, never a success state, never decoration.

**Dark theme is the default** (PRD U11), light theme available via
`data-theme="light"` on any ancestor. App background `#0E1116`, panels `#14181F`,
raised `#1A1F27`. Both themes share one token vocabulary; components never branch on theme.

**Semantic hues are desaturated** so they sit beside slate without shouting: green `#46C08D`
(pass, measured), red `#E5565B` (fail), amber `#E5A93C` (warn, Tune tier), blue `#5B8FB9`
(info, Propose tier). Diff overlays follow DESIGN §7.2: growth warm, shrinkage cool.

**A categorical palette of eight** for treemap regions, flamegraph groups and sweep series:
your code, dependencies, `core::fmt`, panic machinery, unwinding tables, `Drop` glue, vtables,
static data. Fixed assignments — the same category is the same colour in every view.

**Imagery colour vibe:** the one supplied image is cool, desaturated and near-monochrome —
grey-blue line art on white, with the orange mark as the only saturated element. Photography,
where it exists at all, is high-key with a cool cast. No warm filters, no grain, no duotones.

### 4.2 Type

Three families, none of them supplied as binaries (see §6 for the substitution flag):

| Role | Family | Where |
|---|---|---|
| Display | **Poppins** 600/700 | Titles, empty-state headings, poster-style caps. Closest Google Fonts match to the geometric wordmark. |
| UI | **IBM Plex Sans** 400/500/600 | Every in-app string. 13px base, 12px in dense regions, 11px eyebrows. |
| Mono | **JetBrains Mono** 400/500 | Code, symbols, addresses, flags, and **every measured number**. |

**The mono rule is the type system's centre.** If a value was measured, or could be clicked
through to its evidence, it is mono with `font-variant-numeric: tabular-nums` so columns align.
Prose is never mono; measurements are never proportional.

Display type is used sparingly — the app is dense and data-first. Tracking: `-0.02em` on
display, `0` on UI, `.08em` uppercase on 11px eyebrows. No text below 11px anywhere.

### 4.3 Spacing and layout

4px grid; 2px exists only for icon-to-label kerning. Controls are 24 / 30 / 36px high.
Data rows are **28px** dense (symbol tables, sweeps, disassembly) or 34px for findings and
gate lists.

**The application frame is fixed** and every screen respects it: 40px title bar, 72px nav
rail, 340px Findings Inspector, 28px status bar, 12px gutters between panels. The Inspector is
**always present** — DESIGN calls this the single most important UI decision in the product,
because a claim and its evidence must occupy the same screen.

Panels fill their region and scroll internally; the window itself never scrolls. Canvas views
(treemap, flamegraph, scatter) fill their panel and are flush to its border — `padding={false}`.

### 4.4 Backgrounds

Flat colour. No gradients, no images, no textures, no patterns, no noise — none appear in the
brand material and a measurement instrument should not decorate its data.

The only gradients in the system are **protection scrims** (`--scrim-top`, `--scrim-bottom`):
a surface-to-transparent fade where a scrolling canvas passes under chrome. They exist to keep
text legible, never for effect.

### 4.5 Borders, radii, elevation

Hairlines do the structural work. `1px solid var(--border-subtle)` separates every region;
`--border-default` on controls; `--border-strong` on hover.

Radii, exact: **2px** chips and provenance markers · **3px** inputs and table selection ·
**5px** buttons and tags · **8px** panels and cards · **12px** dialogs and the command
palette · full only on switches and pills.

**Cards** — `FindingCard`, `MetricStat` — are 8px radius, raised surface, 1px subtle border,
**no shadow**. Selected cards swap the border to orange and the surface to `--surface-selected`.
Never a coloured left border on a rounded card.

**Elevation is surface value plus a hairline**, not blur. Shadows appear only on things that
genuinely float: dialogs, the command palette, tooltips (`--shadow-overlay`). A single
`--glow-accent` exists for an actively running analysis and nothing else.

Inner shadows are used once: `--shadow-inset-top`, a 4%-white hairline that lifts a raised
header off its panel. No inset wells, no embossing.

### 4.6 Interaction states

- **Hover:** lighten the surface one step (`--surface-hover`) and, on controls, strengthen the
  border. Ghost buttons additionally raise their text from secondary to primary. Never opacity,
  never scale.
- **Press:** `translateY(1px)` and one step darker (`--accent-press`). Nothing shrinks.
- **Selected:** `--surface-selected` plus a **2px orange inset bar** on the leading edge —
  table rows, nav items, palette items and code highlights all use the same signature.
- **Focus:** 2px `--focus-ring` (orange 400) outline at 1px offset, plus a 2px translucent wash
  on inputs. Keyboard-first product; focus is never suppressed.
- **Disabled:** 45% opacity, `not-allowed` cursor. A blocked-by-tier action stays visible and
  states why rather than disappearing.

### 4.7 Motion

Short, flat, no bounce: 80 / 120 / 160 / 240ms with `cubic-bezier(.2,0,.2,1)`.
Only three things move — colour transitions on hover, a 2–4px translate on press, and the
progress/streaming indicators. Findings **stream in** as they are discovered (400ms fade,
no slide-and-settle) because a sweep takes minutes and a blank screen for minutes is a broken
experience. Nothing bounces, nothing springs, nothing parallaxes. Long analyses are always
cancellable — motion never implies a modal wait.

### 4.8 Transparency and blur

Almost never. Three uses only: the dialog/palette scrim (`--surface-scrim`, ~68% with a 2px
blur), the quiet accent washes (`--accent-quiet` at 12–22% orange, for inferred-provenance
panels and toggled controls), and the diff/highlight tints on code lines. Data surfaces are
always fully opaque — a measurement must never be read through something else.

### 4.9 Iconography

See §5.

---

## 5. Iconography

**No icon set was supplied.** The only vector in the sources is the logo — which this system does
not use — and the poster's illustrations are bespoke line art with no reusable glyphs.

**Substitution, flagged:** the system standardises on **Lucide** (`lucide@0.446.0`, loaded
from unpkg), chosen for a 24×24 grid with a light uniform stroke that matches the poster's
thin grey-blue line drawings and the wordmark's even weight. Every icon in this system is a
Lucide name passed to the `Icon` component; there is no hand-drawn SVG anywhere.

**Rules**

- **Stroke, never filled.** Stroke weight `1.75` default, `2` for a selected nav item. Sizes:
  **14** dense table rows, **16** default, **18** nav rail, **20** empty-state marks. No other sizes.
- Icons take `currentColor` and inherit the text colour of their context. They are decorative
  by default (`aria-hidden`); `IconButton` requires a `label` that becomes both tooltip and
  accessible name.
- **Icons never carry meaning alone.** Every status icon is paired with a word or a number.
- The recurring vocabulary, fixed per concept: `hard-drive` size · `sliders-horizontal` tune ·
  `bug` crash · `flame` perf · `waypoints` agent · `shield-check` verification ·
  `git-compare` diff · `link` evidence · `terminal` CLI · `crosshair` location.

**Unicode glyphs are used deliberately, and are not icons.** The provenance markers are
characters, not SVG, because they must sit inline in running text, table cells and terminal
output identically: **● U+25CF Measured · ◈ U+25C8 Derived · ◆ U+25C6 Inferred.**
The CLI additionally uses `✓ ✗ ! ↳ █ ░`. Deltas use the true minus sign **− U+2212**, not a
hyphen, so columns align.

**Emoji are never used.**

**No logo, anywhere.** The system ships no logo files and no product surface displays one. The
title bar identifies the project typographically — a `crosshair` icon in accent, then the crate
name, commit and dirty flag in mono. The CLI identifies itself by its command name, `binmap`. If a
mark is adopted later it belongs in the title bar's leading slot at 18px and nowhere else.

---

## 6. Substitutions and gaps — please confirm

1. **No product mark exists.** The supplied logo was read for colour and character and then set
   aside, as requested. The product needs its own name and, eventually, its own mark — see the
   naming question in the chat.
2. **Fonts are Google Fonts stand-ins.** No binaries were supplied. `tokens/fonts.css`
   `@import`s Poppins, IBM Plex Sans and JetBrains Mono from the Google Fonts CSS API, so the
   design system reports **0 self-hosted `@font-face` rules**. Send the real families (or their
   files) and this becomes local `@font-face` in one edit.
3. **Icons are Lucide from CDN**, not a supplied set (§5).
4. **Poppins is an inference.** The wordmark's geometry reads as a Museo-Sans/Poppins-class
   geometric sans; the real family is unknown.
5. **The name was chosen in this project, not in the sources.** The PRD calls its own
   `lodestone`/`lode` a placeholder (Appendix C). **Binmap** was picked against the brief's
   criteria: plainly descriptive, two words joined, language-neutral, no "AI/smart", readable to
   any developer, and no collision with an existing Rust tool. CLI binary: `binmap`; artifact:
   `binmap.json`; config: `binmap.toml`. A registry search surfaced no `binmap` crate, but that
   is not proof — confirm with `cargo search binmap`, crates.io and a GitHub org check before
   publishing.
6. **No screenshots of the real app exist** — it is pre-alpha. Screens are built from the
   prose and ASCII diagrams in DESIGN §7, which specify layout, panes and provenance markers
   but not exact pixel values. Where a value was unspecified this system chose one and
   documented it; treat those as proposals.
7. **Phase 4 Timeline scrubber is not recreated** (one paragraph of description, no layout).
8. **Model catalog is point-in-time.** Verified 6 September 2026 from provider documentation;
   a few identifiers for very recent releases (Claude Fable 5.1, GPT-6 Astra) are inferred from
   the vendors' naming schemes rather than copied from a model-list response. Binmap should
   always prefer the live `/v1/models` result.
9. **General settings beyond backends are not recreated** (PRD U10 names a settings surface;
   only the agent and model-backend part is described well enough to build).
