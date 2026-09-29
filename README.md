# Binmap

A source-aware artifact analysis tool for Rust. One GPUI desktop application,
Linux x86-64.

Build-profile advice is folklore until somebody measures it on the code in
front of them. Binmap builds your crate across the profile matrix — opt-level,
lto, codegen-units, panic, strip, overflow-checks — measures size, runtime and
build time for each, verifies every one against your own tests, and derives the
Pareto frontier over what it measured.

**Status: Phase 0.** Configuration sweeps and the application shell. Size
attribution, crash analysis, performance attribution and replay debugging are
later phases — see [the implementation plan](docs/Binmap%20implementation%20plan.md).
The model layer is a null backend: the tool registry and evidence store exist,
and nothing calls a model.

## What it will not do

**It will not tell you something it did not measure.** Every claim carries its
evidence — the command, its arguments, the verbatim output and a digest — and a
finding cannot be constructed without at least one. The three provenances are
distinct and never paraphrased:

| | Means | Ceiling |
|---|---|---|
| ● Measured | A tool ran | Certain |
| ◈ Derived | A named rule of ours concluded it from measured inputs | High |
| ◆ Inferred | A model concluded it, under the finding gate | Probable |

**It will not report noise as a win.** The machine's noise floor is measured
once by timing one unchanged binary repeatedly. A difference inside it is
reported inconclusive — never coloured green, and never allowed to decide which
configuration you are shown.

**It will not touch your `Cargo.toml` or your `target/` directory** during a
sweep. Configurations reach cargo as environment variables, and every build goes
into Binmap's own directory. Writing a configuration into your manifest is a
separate act behind the Tune tier, and it refuses a configuration whose tests
failed however good its size looked.

**It will not hide a candidate that failed.** A rejected configuration stays in
the table, greyed, with the gate that rejected it named. A near miss is
informative.

## Running it

```bash
cargo run --release -p binmap -- path/to/your/crate
```

The first run walks three steps — what is missing and the exact fix, what was
discovered, and who is going to think about it — and every step is skippable.
Reopening a project restores what the last session measured.

Keyboard: `⌘K` command palette, `⌘⇧T` sweep, `⌘⇧E` export, `⌘⇧L` theme,
`Escape` to dismiss or cancel. Every action the interface offers is in the
palette; a control that exists only as a click is a bug.

## Configuring a sweep

Optional. Without it you get a sensible default matrix of 96 configurations.

```toml
# binmap.toml, in your project root
[sweep]
opt-level = ["3", "s", "z"]
lto = [false, "thin", "fat"]
overflow-checks = [false, true]
parallelism = 4

# Runtime is only an objective if you declare how to measure it.
# {artifact} is substituted with the binary each configuration produced.
[benchmark]
program = "{artifact}"
arguments = ["--bench"]
samples = 10
```

An axis value Binmap does not recognise is refused by name rather than dropped:
sweeping a matrix you did not ask for and reporting results you would read as
covering it is worse than refusing to start.

## The gates

Every configuration is verified before it is offered, **under its own profile**
— not against a fixed build.

| Gate | Passes when |
|---|---|
| `Builds` | The build succeeds |
| `TestsPass` | Your test suite passes |
| `NoNewWarnings` | It adds no warnings the baseline did not have |
| `SizeNotWorse` | The size change is measured and recorded — in either direction |
| `BenchmarkNotWorse` | No significant regression against the measured noise floor |
| `MiriClean` | Sanitizers are clean, when unsafe is touched |

`SizeNotWorse` asks whether the cost is *known*, not whether it is zero: a
configuration chosen for runtime may cost bytes, out loud.

`MiriClean` carries a caveat on the pass itself where a project makes
substantial foreign-function calls. A clean run says much less there than it
sounds, and a tool implying otherwise is misleading you about safety.

## Requirements

Linux x86-64 and a Rust toolchain. Everything else is optional and the
Environment panel says what each missing thing costs, with the command that
fixes it:

- `hyperfine` — runtime measurement. Without it runtime is not an objective.
- `bloaty` — a cross-check on size attribution.
- nightly with `rust-src` — the `build-std` axis.
- `miri` — the sanitizer gate.

## Development

```bash
cargo test --workspace      # 225 tests, no model calls
cargo clippy --workspace --all-targets
cargo deny check bans       # the architectural boundary, enforced
```

`binmap-eval` is a development-only harness — never packaged, never a supported
surface. Every acceptance criterion is measured through it rather than through
the interface, because an engine feature that cannot be exercised without a
window is a layering violation.

```bash
cargo run --release -p binmap-eval -- doctor corpus/tiny
cargo run --release -p binmap-eval -- targets corpus/tiny
cargo run --release -p binmap-eval -- sweep corpus/generics --jobs 4
cargo run --release -p binmap-eval -- acceptance corpus/tiny
```

`corpus/` holds six reference projects, each there to make one claim testable.
[corpus/README.md](corpus/README.md) says which.

### Layout

| Crate | Responsibility |
|---|---|
| `binmap-core` | Types, the Finding and Evidence model, backend traits, the `Engine` facade |
| `binmap-verify` | The gate harness |
| `binmap-measure` | Size, sections, hyperfine, significance, the Pareto frontier |
| `binmap-build` | Cargo integration, the sweep, the environment probe, the engine |
| `binmap-session` | `binmap.json`, redaction, import |
| `binmap-gui` | The GPUI application — the only product surface |
| `binmap-eval` | The headless harness. Development only |

`binmap-gui` may depend on `binmap-core` and `binmap-session` and nothing else.
The interface may not compute, and `cargo deny check bans` is what holds that
rather than convention.

## Licence

MIT or Apache-2.0, at your option.
