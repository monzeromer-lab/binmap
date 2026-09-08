# Binmap — Web Toolchain Reference (JS/TS)

> The tools behind the JS/TS backend. Companion to `TOOLING-BINARY.md` (native) and
> `TOOLING-DOTNET.md` (C#). Read `DESIGN-POLYGLOT.md` §4.2 first for the rationale.

---

## 1. The mapping

Everything here is the web equivalent of something in the native reference. Reading them
side by side is the fastest way to see what transfers.

| Native | Web | Notes |
|---|---|---|
| `object` — ELF parsing | Bundler metafile / `stats.json` | Structured JSON, not a binary format |
| `gimli` — DWARF | **Source maps** | Same job: generated output ↔ authored source |
| `addr2line` — address to line | `sourcemap` crate lookup | Line/column rather than address |
| `rustc-demangle` | Module path parsing | Package boundaries from `node_modules` paths |
| `iced-x86` — disassembly | *(nothing)* | No machine-code layer. Capability absent. |
| `bloaty` — size | Metafile + gzip/brotli | Three numbers, not one |
| `hyperfine` — timing | `vitest bench` / `tinybench` | Same statistical discipline |
| `perf` — sampling | V8 CPU profile | JSON, source-mapped back to TS |
| Core dump | Minified stack trace | Far shallower, still useful |

**No disassembly, no unwinding, no core dumps, no `rr`.** The entire hard half of the native
backend is absent, which is why this backend is cheap.

---

## 2. Source maps

The load-bearing piece. Source Map v3 is a JSON document that maps positions in generated
output back to positions in original sources.

### 2.1 Shape

```json
{
  "version": 3,
  "file": "bundle.js",
  "sourceRoot": "",
  "sources": ["src/app.ts", "src/util.ts"],
  "sourcesContent": ["export function app() {...", "..."],
  "names": ["app", "compute"],
  "mappings": "AAAA,SAASA,IAAT;AACE,..."
}
```

`mappings` is base64 VLQ, semicolon-separated by generated line, comma-separated by segment.
Each segment is 1, 4, or 5 fields: generated column; source index; original line; original
column; name index. Fields are **relative to the previous segment**, which is why you decode
sequentially rather than randomly.

**Use the `sourcemap` crate.** Do not write a VLQ decoder; the format has enough edge cases
(index maps, relative resolution, one-based versus zero-based line conventions across
producers) that hand-rolling it is a false economy.

### 2.2 What to actually do with it

Two operations carry the whole backend:

**Attribution** — walk every mapping segment, compute the byte span each covers in the
generated file, and attribute those bytes to the original source. That is `source-map-explorer`'s
technique and it is the fallback when no bundler metafile exists.

**Lookup** — given a generated line and column, find the original position. This powers stack
de-minification and profile attribution.

```rust
use sourcemap::SourceMap;

let sm = SourceMap::from_reader(file)?;
if let Some(tok) = sm.lookup_token(line, col) {      // 0-based
    tok.get_source();           // Option<&str> — "src/app.ts"
    tok.get_src_line();
    tok.get_src_col();
    tok.get_name();             // original identifier, when present
}
sm.get_source_contents(idx);    // Option<&str> — embedded original source
```

### 2.3 The traps

1. **`sourcesContent` is often absent** in production builds, since it roughly doubles map
   size. You then need the original files on disk, resolved against `sourceRoot` and the map's
   own location. Handle both, and say which you used — a finding based on source you inferred
   rather than read is weaker.

2. **Index maps.** Multi-pass builds (a transpiler feeding a bundler) can emit a map with a
   `sections` array of nested maps rather than flat `mappings`. Some consumers ignore this and
   silently produce garbage.

3. **Column accuracy degrades after minification.** A minified line can be 40,000 characters
   holding fifty original statements. Mappings are usually correct but *coarse* — several
   statements can share a segment. Never present a column as exact without checking that the
   segment actually starts there.

4. **`sourceMappingURL` comes in three forms**: an external file reference, an inline
   base64 data URI, or absent entirely (hidden source maps, deliberately not shipped). Support
   all three, and when a map is missing say so rather than degrading to unmapped output
   quietly.

5. **Off-by-one between producers.** The spec is 0-based for both line and column, but stack
   traces are 1-based for lines and vary for columns. Normalize at the boundary, in one place,
   and test it. This is the source-map equivalent of the DWARF register-numbering trap.

6. **Chained transformations lose fidelity.** TS → JS → bundled → minified, each with its own
   map, composed. Fidelity degrades at each step. Where a bundler emits a single composed map,
   prefer it over composing yourself.

---

## 3. Bundler metadata

When available, this is much better than source-map attribution: it gives module identity,
import relationships, and — critically — *why* a module is in the bundle.

### 3.1 esbuild

```bash
esbuild src/index.ts --bundle --minify --metafile=meta.json --outfile=out.js
```

```json
{
  "inputs": {
    "node_modules/lodash/lodash.js": { "bytes": 544000, "imports": [...] },
    "src/app.ts": { "bytes": 1200, "imports": [{"path": "...", "kind": "import-statement"}] }
  },
  "outputs": {
    "out.js": {
      "bytes": 128400,
      "entryPoint": "src/index.ts",
      "inputs": { "src/app.ts": { "bytesInOutput": 890 } }
    }
  }
}
```

`bytesInOutput` is the number you want — post-treeshaking, post-minification contribution.
Clean, structured, no parsing tricks. This is the best-supported path and the one to build
against first.

### 3.2 webpack

`webpack --json > stats.json`, or the stats API. Larger and messier, but it carries something
esbuild does not:

- **`reasons`** — for each module, *which modules imported it and how*. This directly answers
  "why is this in my bundle," which is the highest-value question in the whole backend.
- **`usedExports` / `providedExports`** — tree-shaking outcome per export.
- **`chunks`** and **`assets`** — the splitting result.

Stats files for a large app can be tens of megabytes. Stream-parse; do not load naively.

### 3.3 Vite / Rollup

Vite uses Rollup for production builds. There is no metafile equivalent by default; options are
a plugin that emits the bundle graph, or falling back to source-map attribution. Rspack and
Turbopack emit webpack-compatible stats to varying degrees — detect and validate rather than
assume.

### 3.4 Detection strategy

```
1. esbuild metafile present or producible  → best
2. webpack stats producible                → best for "why", biggest parse cost
3. Rollup/Vite bundle graph via plugin     → good
4. Source maps only                        → always works, no module identity
5. Nothing                                 → raw asset sizes only; say so plainly
```

Always report which tier was used. An attribution from tier 4 is genuinely less precise than
tier 1 and the UI should not present them identically.

---

## 4. Size — three numbers, not one

Raw bytes are nearly irrelevant to a web user. Transfer size is what costs latency.

```rust
let raw    = bytes.len();
let gzip   = flate2::write::GzEncoder::new(sink, Compression::new(6)).finish()?;
let brotli = brotli::CompressorWriter::new(sink, 4096, 11, 22);
```

**Compression settings must match what actually ships**, and they usually don't match the
defaults:

| Context | Typical setting |
|---|---|
| gzip, most CDNs | level 6 |
| brotli, precompressed static assets | quality 11 |
| brotli, dynamic/on-the-fly | quality 4-5 |

Record the assumption in the session and expose it as a setting. A size claim under brotli 11
that a user's CDN serves at quality 4 is a wrong number delivered confidently.

### 4.1 The counterintuitive result worth building for

**Compressed size is not proportional to raw size.** A change can remove raw bytes and *increase*
gzipped bytes by breaking compression locality — for instance, deduplicating a repeated string
literal that was compressing almost for free, or reordering modules so that similar code no
longer sits within the compressor's window.

No existing tool measures this during a configuration sweep. Showing raw, gzip, and brotli side
by side across a sweep, and flagging where their ranking disagrees, is a finding nobody else
produces.

### 4.2 Per-chunk, not just total

With code splitting, total size matters less than the size of the *initial* chunk. Attribute per
chunk, mark which are on the critical path from the entry point, and default the headline number
to initial-load bytes rather than the sum of everything.

---

## 5. The config sweep

The Phase 0 machinery ports directly. The axes:

| Axis | Values | Effect |
|---|---|---|
| `target` | es2015 … esnext | Down-level transpilation adds substantial bytes |
| Minifier | esbuild, terser, swc | Different size/time trade-offs |
| Minify options | mangle, compress passes, property mangling | Property mangling is high-risk, high-reward |
| Tree shaking | on/off, `sideEffects` in package.json | A wrong `sideEffects: false` is a correctness bug the verifier catches |
| Code splitting | manual chunks, granularity | Shifts bytes between initial and lazy |
| Source maps | none / external / inline / hidden | Inline maps ship in the bundle |
| `define` / `NODE_ENV` | production | Enables dead-code elimination in dependencies |
| browserslist | target matrix | **Drives polyfill and transpile cost, often the single largest lever** |
| Compression | gzip level, brotli quality | Measurement setting, not a build setting |

`browserslist` is the web analogue of `opt-level`: one setting with an enormous, poorly
understood effect that nobody measures on their own code.

**Sweep mechanics** mirror the Cargo approach — never mutate the user's config files. Bundlers
accept CLI flags and programmatic config, so drive them through a generated temporary config or
CLI arguments and leave the working tree untouched.

---

## 6. Findings the deterministic layer can produce

These are the web equivalents of monomorphization bloat, and each is detectable without a model.

**Duplicate dependencies.** Group metafile inputs by package name parsed from the `node_modules`
path, detect multiple versions present in one bundle. Cross-check with `npm ls <pkg>`,
`pnpm why`, or the lockfile. The remedy is a `resolutions`/`overrides` entry, which is a
proposable, verifiable patch.

**Barrel-file bloat.** A module whose body is almost entirely re-exports, imported for one
symbol, pulling in a large subtree. Detect via high re-export ratio plus large transitive
`bytesInOutput`. The remedy is a deep import.

**Tree-shaking failures.** From webpack `usedExports`, or from a module included whole despite
one named import. Frequently caused by a missing or incorrect `sideEffects` field in a
dependency's `package.json` — worth reporting even when the fix belongs upstream.

**Polyfill cost.** `core-js` modules present in the output, attributed back to the
`browserslist` entries that required them. "Dropping IE11 saves 34 KB gzipped" is a concrete,
measurable business decision the tool can make for the user.

**Whole-library imports.** A large package with a single small named import, where a
subpath import or a lighter alternative exists.

**Unexpected Node polyfills** in a browser bundle — `buffer`, `process`, `crypto` shims pulled
in by a dependency that assumed Node.

---

## 7. Performance

### 7.1 CPU profiles

```bash
node --cpu-prof --cpu-prof-dir=./prof dist/index.js
```

Produces a `.cpuprofile`: V8's JSON format with a `nodes` array (the call tree, each node
carrying `callFrame` with `url`, `lineNumber`, `columnNumber`), a `samples` array of node ids,
and `timeDeltas`. For browser code, the Chrome DevTools Protocol `Profiler` domain gives the
same structure.

Reduce to a weighted call tree, map each `callFrame` through the source map, and the existing
flamegraph view works unchanged.

The chief trap: frames from `node_modules` and V8 internals dominate by count. Group by package
and offer a "your code only" filter by default, or the flamegraph shows a wall of framework
internals.

### 7.2 Benchmarks

`vitest bench` and `tinybench` produce structured results and fill Criterion's role. `hyperfine`
still applies for whole-process timing such as build duration or CLI startup.

The statistical discipline from `TOOLING-HARNESS.md` §2.3 applies unchanged, and matters more
here: JavaScript timing is noisier than native because of JIT warm-up and GC. Warm-up runs are
not optional.

### 7.3 Load-time metrics — keep optional

Parse and compile time scale roughly with bytes, so raw size is already a decent proxy.
Lighthouse or `web-vitals` can serve as an extra verifier gate, but the setup cost is high
(headless browser, stable environment) and the noise is worse than any other measurement in the
product. Treat this as an opt-in gate, never a default.

---

## 8. Failure analysis

The shallowest capability, and still worth having. A production stack trace against minified
code is unreadable; the source map makes it readable.

Input: a stack trace, pasted or loaded. For each frame, parse `file:line:column`, look up
through the map, present original file, line, and surrounding source.

Traps: browser stack formats differ (V8, SpiderMonkey, JavaScriptCore all differ); column
numbers may be 0- or 1-based depending on origin; async stack traces have gaps where the event
loop intervened; and the map must match the exact deployed build, so verify by content hash and
refuse on mismatch rather than producing plausible nonsense. That refusal is the same discipline
as `DESIGN.md` F2.2's stale-binary check.

---

## 9. Project discovery and the verifier

**Package managers.** Detect from the lockfile: `package-lock.json`, `pnpm-lock.yaml`,
`yarn.lock`, `bun.lockb`. This determines the install and run commands, and pnpm's symlinked
`node_modules` layout changes how package paths parse.

**Monorepos.** pnpm workspaces, npm workspaces, Turborepo, Nx. Each package with a build script
is a candidate `Target` in the multi-target model (`DESIGN-POLYGLOT.md` §3). Discovery here
directly populates the target sidebar, so it is worth doing properly rather than asking the user
to add targets by hand.

**Verifier gates** for a web target:

1. Build succeeds
2. Type check passes (`tsc --noEmit`) — cheap and catches a class of bad patches
3. Tests pass (vitest / jest, detected)
4. Bundle size within budget, measured compressed
5. Optional: Lighthouse or a declared benchmark

Gate 2 has no native equivalent and is worth having. A model-proposed change that breaks types
should never reach a human.

---

## 10. The Node bridge

Bundlers are Node programs; Binmap is a Rust program. `binmap-bridge-node` is a small Node
helper spawned on demand.

**What crosses the bridge:** running a bundler with a generated config and returning its
metafile or stats; resolving the module graph; and querying package manager state.

**What does not:** source map parsing (native Rust), gzip and brotli measurement (native Rust),
CPU profile parsing (JSON, native Rust). Keep the bridge as small as possible — every operation
that crosses it is slower, harder to test, and adds a failure mode.

**Lifecycle:** spawn lazily on first use of a web target, keep alive for the session, kill on
close. JSON-RPC over stdio, one request per operation.

**Environment panel entries:** Node present and version; package manager present; bundler
detected and version. Grouped under the web target so a Rust-only user never sees them.

---

## 11. Fragility

| Dependency | Fragility | Why | Mitigation |
|---|---|---|---|
| webpack `stats.json` shape | High | Large, version-dependent, semi-documented | Snapshot fixtures per major version; treat unknown fields as ignorable |
| Rollup/Vite graph access | High | No stable metafile equivalent | Source-map fallback always available |
| Bundler CLI flags | Medium | Change across majors | Version-detect; pin fixtures |
| Node bridge | Medium | User's Node version varies | Declare a minimum; check at spawn |
| `sourcemap` crate | Low | Stable format, mature crate | Normal |
| gzip/brotli crates | Low | Stable | Normal |
| V8 `.cpuprofile` | Low | Stable for years | Snapshot test |

The pattern matches the native side: the highest-fragility dependencies are the *optional*
enrichment paths, and the always-available fallback — source maps — is the stable one.
