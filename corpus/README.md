# The reference corpus

The five projects `docs/Binmap implementation plan.md` §8 asks for, plus the
trivial baseline everything else is compared against. Each is here to make a
specific claim testable; none is here to be impressive.

They are excluded from the workspace, because they exist to be *built by
Binmap*, not to be built alongside it. `cargo test --workspace` does not
compile them; the integration tests that need one point at it explicitly.

| Project | §8's entry | What it is here to prove |
|---|---|---|
| `tiny` | — | The baseline. A binary carrying weight for the ordinary reasons: formatting machinery, panic strings, unwinding tables, symbols nobody reads. Phase 0's acceptance criterion is measured against this. |
| `generics` | a generics-heavy library | Two things. `Pipeline<T>` is instantiated at several unrelated type arguments, so monomorphization grouping has real instantiations to collapse. And its suite is configuration-dependent on purpose — `checksum` asserts wrapping, so it passes under `overflow-checks = false` and fails under `true`. A sweep must reject exactly the overflow-checked configurations, which is what proves the gates run under the configuration they are judging rather than against a fixed build. |
| `workspace` | a multi-crate workspace | `F0.1` says "detect the cargo project **or workspace**; enumerate targets". A single-package project exercises none of that. Three members of different shapes — a leaf library, a library that depends on it, and the binary that depends on both — so the target list has a real dependency order and the sweep has a choice of what to point at. |
| `embedded` | an embedded crate | `no_std`. The size levers that matter to people who count bytes are not the ones that matter to a desktop binary: there is no formatting machinery, no allocator and no panic strings to remove. A tool tuned only against crates full of `println!` reports large easy wins and is useless here. |
| `deep` | an application with a deep dependency tree | Proportion. Twenty-five crates, and most of the bytes belong to code the author did not write. That is the common case, and the one attribution has to get right — a tool that says "your code is 4 KB" without saying where the other 1.5 MB went has answered the wrong question. |
| `wasm` | a WASM target | Size is the whole story for something served over a network, and the levers differ. It is also the entry that proved the section reader was not reading WASM at all: `object`'s `wasm` feature was off, so a `.wasm` module reported no sections — which is not degrading honestly, it is not looking. |

## Notes worth keeping

**`panic = "abort"` cannot be used to make a suite fail.** It was the obvious
axis for `generics` and it does not work: cargo ignores the panic setting for
test targets, because the harness has to unwind to catch a `#[should_panic]`.
`overflow-checks` is the better demonstration anyway — silent wrapping in
release and a panic in debug is a realistic latent defect rather than a
contrived one.

**A corpus project may carry its own `binmap.toml`.** `generics` does, to
narrow the matrix to the eight configurations that make its point. Without it
the default matrix would sweep ninety-six and take far longer to say the same
thing.
