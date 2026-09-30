# Working on Binmap

## Do not stop

**Never stop to ask whether to continue.** Never end a turn with a summary and
a question. Never write "tell me if you want X" and then stop — just do X.

When a phase finishes, start the next one immediately in the same turn. Keep
going until the whole project is built and tested. The only reasons to end a
turn are: the work is genuinely finished, or something is blocked in a way no
decision of mine can resolve.

This has been said three times. It is the single most important instruction in
this file.

Progress reports are fine *while working* — they are not a reason to hand
control back. Report and keep going in the same turn.

## What "done" means

Every phase in `docs/Binmap implementation plan.md`, implemented and tested,
with its acceptance criterion measured rather than asserted. The phases are
Phase 0, 1, 1.5, 2, 3, 4. Each one's criterion is in the plan.

A phase is not done because the types exist. It is done when:

- `cargo test --workspace` passes,
- `cargo clippy --workspace --all-targets` is silent,
- `cargo deny check bans licenses advisories` passes,
- `cargo fmt --all --check` is clean,
- and the feature is reachable from `binmap-eval` without the GUI.

That last one is not a formality. A control only the interface can exercise is
a control nobody can test.

## How to work

**Test against real artifacts, not fixtures written to pass.** Every corpus
project here exists because a fixture lied. Real esbuild output corrected the
chunk classification, the source-map span arithmetic, and three of the finding
rules. Real cargo output corrected the baseline. When a synthetic test passes
and a real one fails, the synthetic test was written to match the assumption
being tested.

**Use the Edit tool for Rust source, not python string replacement.** rustfmt
reformats what was just written, so an exact-match replace silently does
nothing. This has cost time repeatedly.

**Never write to a real project's `Cargo.toml` or `dist/`.** Sweeps build into
`target/binmap` and `node_modules/.binmap`. Testing the apply path uses
throwaway copies in the scratchpad only — this was Monzer's explicit decision.

**Run one sweep per project at a time.** Two concurrent sweeps delete each
other's build directories and produce believable wrong numbers. There is a
lock (`crates/binmap-build/src/lock.rs`); heed its refusal rather than working
around it.

## The standards the codebase holds itself to

These are visible in every commit message and comment, and new code is
expected to match:

- A claim carries its evidence. `Finding::new` refuses otherwise.
- Provenance is never paraphrased: ● Measured, ◈ Derived (with the rule
  named), ◆ Inferred. A conclusion drawn by a rule of ours is Derived, not
  Measured, however certain the inputs were.
- Say what a thing costs, not only what it saves. Every strategy and every
  finding names its trade-off.
- A failure exits non-zero and says why. Reporting success on a failed run is
  the bug this codebase has hit most often.
- Refuse rather than guess. An unbuilt project, a stripped binary, a missing
  source map — each says so instead of reporting an empty result as an answer.
