# Lodestone — Harness Toolchain Reference

> Working reference for `clap`, `hyperfine`, and `bloaty` — the CLI surface and the two
> measurement tools Lodestone shells out to. Companion to `TOOLING-BINARY.md`.

---

## 1. The headless harness (`lode-eval`)

**There is no shipped CLI** (PRD N8). The product is one GUI binary. But the engine still needs
to be driven without a window — for the eval suite, for CI, and to keep the engine from
quietly growing a dependency on the UI. That job belongs to `lode-eval`: a development-only
binary that is never published to crates.io, never included in a release artifact, and never
documented for users.

`clap` is still the right tool for it, just with a much smaller surface.

### 1.1 What it needs

```rust
#[derive(Parser)]
#[command(name = "lode-eval", about = "Development harness. Not a product surface.")]
struct Cli {
    #[arg(long, default_value = ".")]
    project: PathBuf,

    #[arg(long, value_enum, default_value_t = Reasoner::None)]
    reasoner: Reasoner,      // None | Native(provider) | Acp(agent)

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run one analysis and emit lodestone.json
    Run { kind: AnalysisKind },
    /// Run the eval corpus and report metrics
    Suite { #[arg(long)] fixtures: PathBuf },
    /// Environment check (the engine half of the GUI's Environment panel)
    Doctor,
}
```

That is the whole thing. Resist growing it: every flag added here is a flag someone will
eventually ask you to support as a product feature, and N8 exists to prevent that conversation.

### 1.2 Output discipline still applies

The rules that made the CLI scriptable now protect the artifact contract instead:

1. **stdout is data, stderr is progress.** `lode-eval` emits only `lodestone.json` on stdout.
   The eval harness parses it; anything else on that stream is a bug.
2. **The JSON schema is versioned and stable.** Every artifact carries `schema_version`.
   Generate the schema from the Rust types with `schemars` and commit it — it is documentation
   and a regression test at once. The GUI, the eval harness, and session export/import all
   consume this same shape.
3. **Exit codes carry meaning**, because CI uses them:

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Internal error |
| 2 | Usage error |
| 3 | A verification gate failed |
| 5 | Required external tool missing |

### 1.3 Where the old CLI concerns went

Each command from the previous design has a home in the GUI. Recording the mapping prevents
capability from being lost silently in the transition.

| Former command | Now |
|---|---|
| `lode doctor` | Environment panel (PRD U10). Same checks, same exact-fix messages, rendered rather than printed. |
| `lode tune` | Profile Lab |
| `lode size` | Size Explorer |
| `lode analyze <core>` | Crash view, with a file picker. **See PRD open question 1** — this is the one case that plausibly warrants a thin launcher, since core dumps originate in terminals. |
| `lode profile` | Flamegraph view |
| `lode verify` | The Verify button on any proposal in the Findings Inspector |
| `lode ci` | **Dropped.** PRD R13 records the cost: this was the cheapest path into an organisation's repository. Revisit after Phase 2 as a separate crate. |
| `--json` | Session export (PRD U13) |
| `--no-ai` | "No reasoner" in the reasoner picker — a listed option, not a hidden flag |
| `--trust` | Trust tier control in the chrome |
| Shell completions, man page | No longer applicable |

### 1.4 The accessibility debt

A CLI is, incidentally, an accessibility feature: it works with screen readers, it scripts, and
it runs over SSH. Removing it puts that weight entirely on the GUI, and GPUI's screen-reader
story is weaker than the web's (PRD open question 7). Keyboard-first navigation and a command
palette are therefore **Must**, not **Should** (PRD U12) — they are the only remaining
accommodation for the substantial fraction of this audience that lives in a terminal.

---

## 2. `hyperfine` — timing

### 2.1 Invocation

```bash
hyperfine \
  --warmup 3 \
  --min-runs 20 \
  --shell=none \
  --setup 'cargo build --release' \
  --prepare 'sync; echo 3 > /proc/sys/vm/drop_caches' \
  --command-name 'baseline'  './target/base/app --input bench.json' \
  --command-name 'candidate' './target/cand/app --input bench.json' \
  --export-json result.json \
  --style none
```

| Flag | Why it matters |
|---|---|
| `--warmup N` | Discards the first N runs. Essential — first-run filesystem cache and page-fault costs dominate short benchmarks. |
| `--min-runs`, `--max-runs`, `--runs` | Run count. Defaults are tuned for interactive use, not for statistical confidence. Set explicitly. |
| `--shell=none` / `-N` | **Use this.** Avoids spawning a shell per run, removing both the overhead and its variance. Without it, hyperfine measures and subtracts shell startup, which is a correction you do not need if you never pay the cost. |
| `--setup CMD` | Runs once before a command's runs. Where the build goes. |
| `--prepare CMD` | Runs before *each* run. Where cache clearing or state reset goes. |
| `--cleanup CMD` | Once after. |
| `--command-name` | Names the entry in JSON. Without it you match on command strings, which is fragile. |
| `--export-json` | The only output you should parse. |
| `--style none` | Suppresses progress rendering when running programmatically. |
| `--ignore-failure` | Do **not** use by default. A candidate config that makes the program crash should fail the gate, not produce a fast time. |
| `--parameter-list` / `--parameter-scan` | Sweeps an input parameter. Useful for finding where two configurations cross over. |

### 2.2 The JSON

```json
{
  "results": [
    {
      "command": "./target/base/app --input bench.json",
      "mean": 0.4213,
      "stddev": 0.0087,
      "median": 0.4198,
      "user": 0.3901,
      "system": 0.0290,
      "min": 0.4102,
      "max": 0.4488,
      "times": [0.4102, 0.4155, 0.4198, "..."],
      "exit_codes": [0, 0, 0],
      "parameters": {}
    }
  ]
}
```

Two notes. **`stddev` is `null` when there is only one run** — deserialize it as
`Option<f64>` or you will panic on a degenerate case in the field. And **`times` is the field
that matters**; the summary statistics are for humans.

### 2.3 Statistics — where credibility is won or lost

**Do not compare `mean ± stddev` and call non-overlap significance.** Overlapping error bars
are not a test, and non-overlapping ones are a conservative and low-power test. Take the raw
`times` arrays and do this properly.

**Recommended: Welch's t-test on the two `times` arrays**, which does not assume equal
variance. Report the p-value and the effect size, not just a verdict.

```rust
struct Comparison {
    delta_pct: f64,          // (cand_mean - base_mean) / base_mean
    ci95: (f64, f64),        // bootstrap CI on the difference
    p_value: f64,
    verdict: Verdict,        // Improved | Regressed | NoSignificantChange | TooNoisy
}
```

**Better still for skewed data: bootstrap the difference in medians.** Benchmark timings are
right-skewed — a run can be slow because of an unlucky scheduler decision, but never faster
than the hardware allows. The mean is dragged by that tail. Resample both `times` arrays with
replacement 10,000 times, take the difference of medians each time, and report the 2.5th and
97.5th percentiles. It is about twenty lines, assumes nothing about the distribution, and
gives you a confidence interval you can render directly as an error bar.

**Effect size gate.** Statistical significance is not practical significance. With enough runs
a 0.3% difference becomes significant and is still noise for a user's purposes. Require both
`p < 0.05` **and** `|delta| > noise_floor`, where the floor is measured, not assumed.

### 2.4 Measuring the noise floor

Run the *same binary* against itself under the same harness, twenty times, and record the
distribution of the "difference." That is the floor. Store it per machine in the session, and
never report an improvement smaller than it.

This one measurement does more for the product's credibility than any amount of statistical
machinery. It is also a good `lode doctor` check: a machine with a 15% noise floor should be
told that its results will be weak before it wastes an hour.

### 2.5 Environment control matters more than statistics

Ranked by impact on a typical Linux developer machine:

1. **CPU frequency scaling and turbo.** Set the governor to `performance` or accept a wide
   floor. Detect the governor and warn.
2. **Core pinning.** `taskset -c N` removes migration noise. Consider offering
   `--pin-cpu`.
3. **Thermal throttling.** A long sweep heats the machine; later configurations look worse
   through no fault of their own. **Randomize or interleave the run order** rather than
   testing configurations in sequence. This is the single most under-appreciated source of
   bias in build-configuration sweeps, and interleaving costs nothing.
4. **ASLR.** Adds variance through cache and TLB layout effects. Not worth disabling
   (it changes the thing you are measuring), but worth knowing when explaining variance.
5. **Other load.** Detect high load average and warn.

### 2.6 Criterion, for in-repo benchmarks

When the project already has Criterion benches, use them rather than imposing `hyperfine`.
Criterion writes structured estimates into `target/criterion/<bench>/new/estimates.json`,
including confidence intervals it computed itself, and `cargo criterion --message-format=json`
gives a machine-readable stream.

Rule of thumb: **`hyperfine` for whole-program wall clock, Criterion for functions.** A size
optimization usually needs the former; a monomorphization change often needs the latter, since
the effect may be invisible at whole-program scale.

---

## 3. `bloaty` — size

### 3.1 Invocation

```bash
bloaty ./target/release/app \
  -d compileunits,symbols \
  -n 0 \
  -s file \
  --domain=file \
  --demangle=none \
  --csv
```

| Flag | Notes |
|---|---|
| `-d <sources>` | Data sources, comma-separated, giving a hierarchy left to right |
| `-n N` | Rows to show; `-n 0` means all. The default truncates and will silently hide most of your binary. |
| `-s vm\|file` | Sort key |
| `--domain=vm\|file\|both` | Which size to report |
| `--demangle=none\|short\|full` | **Use `none`** — see §3.3 |
| `--csv` / `--tsv` | Machine-readable output |
| `-w` | Wide human output |
| `--source-filter REGEX` | Restrict to matching entries |
| `--debug-file=PATH` | Point at separate debug info for a stripped binary |
| `-- <old_binary>` | Diff mode: everything before `--` is new, after is the baseline |

**Data sources:**

| Source | Meaning | Cost |
|---|---|---|
| `sections` | ELF sections | Free |
| `segments` | Program headers | Free |
| `symbols` | Symbol table | Cheap |
| `compileunits` | Attributes bytes to source files — **requires DWARF** | Moderate |
| `inlines` | Attributes to source *lines* — requires line info | Slow |
| `inputfiles` | Attributes to object files / rlibs | Cheap |
| `armembers` | Archive members | Cheap |
| `rawranges` | Debugging bloaty itself | — |

For Rust, `compileunits` maps roughly to codegen units and crates, which is a useful
cross-check on your own crate attribution.

### 3.2 VM versus file size

They differ, and conflating them produces numbers that do not add up:

- `.bss` has VM size and zero file size
- Alignment padding and `PT_LOAD` gaps exist in the file but not meaningfully in memory
- Debug sections have file size but are not loaded at all
- Compressed debug sections have a file size that does not reflect their content size

Pick per feature and label it in the UI. "Binary size" for a user shipping a container image
means file size; for an embedded user it usually means what occupies flash. `--domain=both`
shows the pair and is a good default for the cross-check.

### 3.3 The Rust demangling problem

**`bloaty` will not demangle Rust v0 symbols correctly.** Its demangler targets the Itanium
C++ ABI; v0 is a different scheme with a `_R` prefix.

Consequence: pass `--demangle=none` and demangle the raw symbols yourself with
`rustc-demangle` (`TOOLING-BINARY.md` §4). This is better anyway — you need the structure for
grouping, and a display string from a C++ demangler is useless for that.

It also means **`bloaty`'s own grouped output should not be shown to users directly.** Its role
is arithmetic validation, not presentation.

### 3.4 The cross-check test

This is the reason to keep `bloaty` as a dependency at all.

```rust
#[test]
fn attribution_reconciles_with_bloaty() {
    for binary in corpus_binaries() {
        let ours   = lode_size::attribute(&binary).unwrap();
        let theirs = bloaty::section_totals(&binary).unwrap();

        // Section totals must match exactly — this is just parsing.
        assert_eq!(ours.by_section, theirs.by_section);

        // Symbol attribution plus padding plus non-symbol overhead must equal
        // the section total. Tolerance covers linker-inserted entries.
        let accounted = ours.symbol_bytes + ours.padding_bytes + ours.overhead_bytes;
        let delta = (accounted as i64 - theirs.total_file as i64).abs();
        assert!(delta < theirs.total_file / 1000,
                "unattributed: {} bytes in {}", delta, binary.display());
    }
}
```

If this test fails, your symbol-size algorithm (`TOOLING-BINARY.md` §1.3) is dropping bytes.
That failure is otherwise invisible — the treemap looks fine, it is just wrong.

### 3.5 Diff mode

```bash
bloaty new_binary -- old_binary -d compileunits --csv
```

Outputs deltas rather than absolutes. Useful as a validation oracle for your own diff view,
but your diff needs to distinguish four cases `bloaty` conflates:

1. Symbol grew
2. Symbol shrank
3. Symbol appeared
4. Symbol disappeared

...plus a fifth `bloaty` cannot see at all: **a symbol was renamed because its generic
arguments changed**. That is a rename to a text-matching tool and an important signal to a
tool that understands v0 mangling. Handling it is a differentiator, not an edge case.

### 3.6 Alternatives and complements

| Tool | Role |
|---|---|
| `cargo-bloat` | Rust-native, understands the crate model. Good reference implementation to read; largely superseded by your own attribution. |
| `cargo-llvm-lines` | Counts LLVM IR lines per generic function. A **pre-build** proxy for monomorphization cost — ranks candidates without a full build, which makes it useful for a fast first pass. |
| `twiggy` | wasm-specific, with dominator-tree analysis for "what would I save by deleting this." Required for the WASM corpus crate. |
| `nm`, `size`, `readelf` | Ground truth for tests. `readelf --debug-dump` is invaluable when your `gimli` code disagrees with reality. |

---

## 4. Subprocess discipline

All three shelled-out tools share one wrapper pattern.

```rust
pub struct ExternalTool {
    name: &'static str,
    path: PathBuf,
    version: Version,
    min_version: Version,
}

impl ExternalTool {
    pub fn discover(name: &'static str, min: Version) -> Result<Self, ToolError>;
    pub async fn run(&self, args: &[&str], stdin: Option<&[u8]>) -> Result<ToolOutput>;
}

pub struct ToolOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub status: ExitStatus,
    pub digest: [u8; 32],   // feeds Provenance::Deterministic
    pub duration: Duration,
}
```

Rules:

1. **Version-check at discovery**, not at first use. `lode doctor` surfaces the result.
2. **Parse in exactly one module per tool.** Nowhere else may see raw output.
3. **Snapshot-test every parser** against captured real output committed as a fixture. This
   catches upstream format changes without requiring the tools in CI.
4. **Never parse human-readable output** when a machine format exists. `--csv`,
   `--export-json`, `--message-format=json`.
5. **Record the digest** of every output into the evidence store. This is what makes
   `Provenance::Deterministic` meaningful rather than decorative.
6. **Kill on cancel.** A sweep that leaves orphaned `cargo` processes after the user hits
   escape is a bug that will be reported as "the tool ate my CPU."
7. **Cap and stream output.** A pathological `bloaty -n 0` on a huge binary produces a lot of
   CSV; do not buffer unboundedly.

---

## 5. Fixtures to capture now

Before writing any parser, capture real output from each tool on each corpus binary and commit
it under `evals/fixtures/tool-output/`. Ten minutes of work that makes every parser testable
without the tools installed, and gives you a diff to look at when a tool upgrade changes
something.

```
evals/fixtures/tool-output/
├── bloaty/     {crate}.{sections,symbols,compileunits}.csv
├── hyperfine/  {crate}.{baseline,candidate}.json
├── readelf/    {crate}.debug-dump.txt        # DWARF ground truth
├── nm/         {crate}.symbols.txt
└── perf/       {crate}.script.txt
```
