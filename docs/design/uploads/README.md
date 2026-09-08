# Lodestone

**A source-aware binary analysis tool for Rust.** Lodestone reads your source and your
compiled binary at the same time, and uses that dual view to explain why your program is
wrong, why it's slow, and why it's large — then proposes changes and proves they work.

> Status: pre-alpha. Nothing works yet. See [`docs/PRD.md`](docs/PRD.md) and
> [`docs/DESIGN.md`](docs/DESIGN.md).

---

## Scope

Lodestone answers three questions about a Rust program, using the same underlying engine:

- **Why is this so big?** Attributes every byte in your binary to a crate, module, or generic
  instantiation — then finds the build configuration and source changes that shrink it.
- **Why is this slow?** Attributes CPU samples back to source constructs, through inlining,
  and explains what the compiler actually did.
- **Why did this crash?** Takes a core dump from an optimized build and reconstructs the
  faulting line, the local values, and the root cause — including variables the debugger
  reports as "optimized out."

**Linux x86-64, Rust only, for now.**

## Non-goals

Lodestone is not an IDE, not a linter, not a replacement for GDB or LLDB, and not a hosted
service. It does not run your code anywhere but your machine.

---

## How it works

Lodestone is a **deterministic analysis engine with an optional language-model layer.**

The engine measures, symbolizes, and verifies. It computes every fact. The model layer reads
those facts and produces explanations, rankings, and proposals — and it may only reference
evidence the engine produced.

Two consequences:

1. **It works with the model turned off.** `--no-ai` is a first-class mode, and the entire
   test suite runs in it. You get a config autotuner, a size attributor, and a crash
   symbolizer with no AI involved at all.
2. **Every claim shows its evidence.** Findings carry a provenance chain distinguishing what
   was measured from what was inferred, and the UI never shows you one without the other.

Nothing is uploaded anywhere. Local model backends are the default; cloud backends require
your own API key and can be disabled per-project.

---

## Trust tiers

Lodestone's autonomy is a single visible setting:

| Tier | Behaviour |
|---|---|
| **Observe** | Measures and explains. Read-only. |
| **Propose** *(default)* | Generates diffs. Never applies them. |
| **Tune** | Modifies build configuration only, after tests pass and benchmarks confirm. |
| **Autonomous** | Applies source patches and opens a PR. Requires explicit opt-in. |

Nothing is applied without passing the verifier: build succeeds, tests pass, benchmarks show
a statistically significant win, sanitizers clean if `unsafe` was touched.

---

## Install

```
# Not yet available.
```

Requires: Linux x86-64, a Rust toolchain, and `bloaty` + `hyperfine` on `PATH`.
Optional: `perf` (profiling), `rr` (replay debugging).

## Usage

```bash
lode tune                    # find the best build profile for this crate
lode size                    # where did the bytes go?
lode size --diff <baseline>  # what grew since the last build?
lode analyze core.1234       # root-cause a crash
lode profile -- ./bench      # attribute CPU time to source
lode ci                      # enforce size/perf budgets, exit non-zero on regression

lode --no-ai <any of the above>   # deterministic only
lode gui                          # open the desktop app
```

All commands accept `--json` and emit a `lodestone.json` artifact.

---

## Contributing

Contributions welcome, especially on the deterministic layer — the analysis engines and the
`lodestone.json` artifact format are the easiest places to start, and require no AI
involvement.

Issues labelled `good-first-issue` are kept stocked. Before proposing a feature, please read
the non-goals in [`docs/PRD.md`](docs/PRD.md) §5. Scope discipline is what keeps this project
alive.

## License

Dual-licensed under Apache-2.0 or MIT, at your option.
