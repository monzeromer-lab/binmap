# Binmap

**A source-aware artifact analysis tool for Rust, TypeScript, and C#.** Binmap reads your
source and the thing your build produced at the same time, and uses that dual view to explain
why your program is wrong, why it's slow, and why it's large — then proposes changes and proves
they work.

> Status: pre-alpha. Nothing works yet. See [`docs/PRD.md`](docs/PRD.md),
> [`docs/DESIGN.md`](docs/DESIGN.md), and [`docs/DESIGN-AI.md`](docs/DESIGN-AI.md).

---

## Scope

Binmap answers three questions about a Rust program, using one engine:

- **Why is this so big?** Attributes every byte to a crate, module, or generic instantiation —
  then finds the build configuration and source changes that shrink it.
- **Why is this slow?** Attributes CPU samples back to source constructs, through inlining, and
  explains what the compiler actually did.
- **Why did this crash?** Takes a core dump from an optimized build and reconstructs the
  faulting line, the local values, and the root cause — including variables the debugger
  reports as "optimized out."

Three languages, one engine: **Rust** (native binaries + DWARF), **JS/TS** (bundles + source
maps), and **C#** (NativeAOT and CoreCLR). Nothing else is on the roadmap — see
[`docs/DESIGN-POLYGLOT.md`](docs/DESIGN-POLYGLOT.md) §4.4.

It is a **desktop application**; there is no CLI.

## Non-goals

Not an IDE, not a linter, not a replacement for GDB or LLDB, not a hosted service, and not a
command-line tool. It does not run your code anywhere but your machine.

---

## How it works

Binmap is a **deterministic analysis engine with an optional language-model layer.**

The engine measures, symbolizes, and verifies. It computes every fact. The model layer reads
those facts and produces explanations, rankings, and proposals — and it may only reference
evidence the engine produced.

Two consequences:

1. **It works with no model at all.** "No reasoner" is a listed option in the picker, not a
   hidden flag, and the entire test suite runs that way. You get a config autotuner, a size
   attributor, and a crash symbolizer with no AI involved.
2. **Every claim shows its evidence.** Findings carry a provenance chain distinguishing what
   was measured from what was inferred, and the UI never shows one without the other.

## Bring your own reasoner

Binmap does not sell you a model. It connects to what you already have.

**External agents over [ACP](https://agentclientprotocol.com)** — Claude Code and Codex CLI at
launch, with the rest of the ACP ecosystem to follow. The agent runs as its own process with
its own authentication and billing, and Binmap hands it the analysis toolset over MCP.

**Direct API access** with your own key — Claude, OpenAI, DeepSeek, Kimi, Z.ai, and any
OpenAI-compatible endpoint including local runners like Ollama and llama.cpp.

**Or nothing at all.**

Local runners are the default. Nothing is uploaded anywhere unless you pick a cloud reasoner,
and a project can set `allow_cloud_models = false` to take that choice off the table entirely.

---

## Trust tiers

Binmap's autonomy is a single visible setting:

| Tier | Behaviour |
|---|---|
| **Observe** | Measures and explains. Read-only. |
| **Propose** *(default)* | Generates diffs. Never applies them. |
| **Tune** | Modifies build configuration only, after tests pass and benchmarks confirm. |
| **Autonomous** | Applies source patches and opens a PR. Requires explicit opt-in. |

The tier also governs external agents: it filters which tools they can see at all, and it
drives the answer to their permission requests.

Nothing is applied without passing the verifier — build succeeds, tests pass, benchmarks show a
statistically significant win, sanitizers clean if `unsafe` was touched. **Permission gets an
agent to the verifier; it does not get it past.**

---

## Install

```
# Not yet available.
```

Requires: Linux x86-64 (for now), plus the toolchain for whichever languages you analyze — a
Rust toolchain and `bloaty`/`hyperfine`, Node for JS/TS targets, a .NET SDK for C# targets.
Optional: `perf` (profiling), `rr` (replay debugging), an ACP agent, a local model runner.

The Environment panel checks all of this on first run and tells you the exact fix for anything
missing.

---

## Contributing

Contributions are welcome, especially on the deterministic layer — the analysis engines and the
`binmap.json` artifact format are the easiest places to start and require no AI involvement.

The engine is a library, driven headlessly by `binmap-eval` for tests and evaluation. You can
work on analysis without touching the UI, and on the UI without touching DWARF.

Issues labelled `good-first-issue` are kept stocked. Before proposing a feature, please read
the non-goals in [`docs/PRD.md`](docs/PRD.md) §5. Scope discipline is what keeps this project
alive.

## Documentation

| Document | Contents |
|---|---|
| [`PRD.md`](docs/PRD.md) | Problem, users, goals, phased requirements, risks |
| [`DESIGN.md`](docs/DESIGN.md) | Architecture, data model, verification, testing |
| [`DESIGN-GUI.md`](docs/DESIGN-GUI.md) | GPUI application design |
| [`DESIGN-AI.md`](docs/DESIGN-AI.md) | ACP and direct-API agent design, trust boundary |
| [`TOOLING.md`](docs/TOOLING.md) | Toolchain survey and fragility assessment |
| [`TOOLING-BINARY.md`](docs/TOOLING-BINARY.md) | `object`, `gimli`, `addr2line`, demangling, `iced-x86` |
| [`TOOLING-HARNESS.md`](docs/TOOLING-HARNESS.md) | `hyperfine`, `bloaty`, the headless harness |
| [`TOOLING-WEB.md`](docs/TOOLING-WEB.md) | Source maps, bundler metafiles, gzip/brotli, V8 profiles |
| [`TOOLING-DOTNET.md`](docs/TOOLING-DOTNET.md) | ILC `.mstat`/`.dgml`, trim warnings, ClrMD, MSBuild |
| [`DESIGN-POLYGLOT.md`](docs/DESIGN-POLYGLOT.md) | Capability model, per-language backends, UI impact |

## License

Dual-licensed under Apache-2.0 or MIT, at your option.
