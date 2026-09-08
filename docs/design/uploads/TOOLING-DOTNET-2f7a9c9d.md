# Binmap — .NET Toolchain Reference (C#)

> The tools behind the C# backend. Companion to `TOOLING-BINARY.md` (native) and
> `TOOLING-WEB.md` (JS/TS). Read `DESIGN-POLYGLOT.md` §4.3 first for the rationale.
>
> **Verify the empirical claims marked ⚠ before building on them.** The .NET AOT tooling
> surface moves faster than the DWARF or source-map ecosystems, and several properties below
> are worth confirming against the SDK version you target.

---

## 1. Two backends, one language

The capability gap between them is larger than the gap between Rust and TypeScript, so the UI
must distinguish them explicitly (`DESIGN-POLYGLOT.md` §5.1 gives them separate badges).

| | **NativeAOT** | **CoreCLR (JIT)** |
|---|---|---|
| Artifact | Native ELF | Managed assemblies + host |
| Source mapping | DWARF ⚠ | Portable PDB |
| Size attribution | **ILC `.mstat`** (see §3) | Assembly / type level |
| Why-is-this-here | **ILC dependency graph** | Trim warnings |
| Instantiation bloat | Yes — value-type generics | Partially |
| Config sweep | Rich | **Richer** |
| Failure artifact | Core dump | Managed dump (ClrMD) |
| Profiling | `perf` | EventPipe / `perf` + perfmap |
| Effort | Rides the native backend | Separate, shallower engine |

---

## 2. The single most important finding

**Do not reverse-engineer NativeAOT's symbol mangling.**

That was the plan implied by `DESIGN-POLYGLOT.md` §4.3, and it is the wrong approach. The ILC
compiler can emit its own structured accounting of what it put in the binary and why:

| Property ⚠ | Output | Contains |
|---|---|---|
| `IlcGenerateMapFile` | `.map` | Symbol → address/size map |
| `IlcGenerateMstatFile` | `.mstat` | **Per-type and per-method size breakdown** |
| `IlcGenerateDgmlFile` | `.dgml` | **Dependency graph — why each item was kept** |

```xml
<PropertyGroup>
  <PublishAot>true</PublishAot>
  <IlcGenerateMstatFile>true</IlcGenerateMstatFile>
  <IlcGenerateDgmlFile>true</IlcGenerateDgmlFile>
</PropertyGroup>
```

The `.mstat` file is a managed assembly containing size records — the same data the existing
`sizoscope` tool consumes for NativeAOT size analysis. It gives you type and method attribution
with full managed names, no demangling required.

The `.dgml` is better still. It is the ILC dependency graph: for any item in the binary, the
chain of reasons it survived trimming. **That is a first-class answer to "why is this in my
binary," which the native Rust backend cannot produce at all** — DWARF tells you what is there,
not why the linker kept it.

**Consequences for the design:**

- The `.mstat` reader replaces symbol-table attribution for NativeAOT. Reading a managed
  assembly's metadata from Rust is awkward, so this likely goes through the .NET bridge (§7).
- The `.dgml` reader is a **differentiated feature**, not a nice-to-have. Ship it in the first
  NativeAOT release.
- ⚠ Verify the exact property names and `.mstat` schema against your target SDK. The format has
  changed across .NET versions and is not covered by a compatibility guarantee.
- DWARF from ILC still matters for crash analysis, but no longer for size. That lowers the risk
  of the unverified DWARF-quality assumption considerably — if it turns out weak, you lose
  Phase 2 for C#, not Phase 1.

---

## 3. The config sweep — the richest of the three languages

This is where the C# backend earns its place. The axes are numerous, consequential, and chosen
today by folklore.

### 3.1 Deployment shape

| Property | Values | Effect |
|---|---|---|
| `PublishAot` | true/false | The big one |
| `SelfContained` | true/false | Bundles the runtime; large size delta |
| `PublishTrimmed` | true/false | Dead code removal |
| `TrimMode` | `partial` / `full` | Aggressiveness |
| `PublishReadyToRun` | true/false | Precompiled native code; larger file, faster startup |
| `PublishSingleFile` | true/false | Packaging, some size cost |
| `EnableCompressionInSingleFile` | true/false | Trades startup for size |

### 3.2 Feature switches

Each removes a subsystem in exchange for functionality. These are exactly the folklore knobs
nobody measures on their own application:

`InvariantGlobalization` · `UseSystemResourceKeys` · `EventSourceSupport` ·
`MetadataUpdaterSupport` · `StackTraceSupport` ⚠ · `DebuggerSupport` ·
`HttpActivityPropagationSupport` · `UseNativeHttpHandler` ·
`NullabilityInfoContextSupport` · `EnableUnsafeBinaryFormatterSerialization`

`InvariantGlobalization` alone can remove several megabytes of ICU data. `StackTraceSupport`
trades diagnosability for size, which is precisely the kind of trade a tool should quantify
rather than a developer guess at.

### 3.3 ILC optimization

| Property ⚠ | Values |
|---|---|
| `IlcOptimizationPreference` | `Speed` / `Size` / `Blended` |
| `IlcFoldVirtualMethodBodies` | true/false |
| `IlcDisableReflection` | true/false |
| `StripSymbols` | true/false |

### 3.4 JIT-side runtime knobs

`TieredCompilation`, `TieredPGO`, `ReadyToRun` interaction. These affect startup and steady-state
throughput rather than size, and belong to the CoreCLR backend's sweep.

### 3.5 The flagship question

**"What would going AOT actually cost and save us?"**

Framed as a sweep, that is a single run producing a Pareto frontier over deployment shapes:
framework-dependent, self-contained, trimmed, ReadyToRun, and NativeAOT, each measured for
artifact size, startup time, steady-state throughput, and build time — with the test suite
passing at every point, and every failing configuration reported with its failing gate.

For a company running C# services, that is a decision currently made by argument. Turning it
into a measurement may justify the tool internally on its own.

### 3.6 Sweep mechanics

MSBuild properties can be set on the command line without editing project files, which mirrors
the `CARGO_PROFILE_*` approach:

```bash
dotnet publish -c Release \
  -p:PublishAot=true \
  -p:IlcOptimizationPreference=Size \
  -p:InvariantGlobalization=true \
  -p:StackTraceSupport=false
```

Never mutate the user's `.csproj` or `Directory.Build.props` during a sweep.

**Build time is a real constraint here.** A NativeAOT publish is slow — minutes, not seconds.
The sweep matrix must be pruned more aggressively than for Rust, results cached hard, and the
UI must set expectations before a user starts a 40-configuration sweep that will run for hours.
Consider a two-stage sweep: cheap axes first, then expensive ones around the winner.

---

## 4. Project discovery

MSBuild is the source of truth and should not be parsed by hand. Modern SDKs can emit evaluated
properties and items as JSON: ⚠

```bash
dotnet msbuild App.csproj \
  -getProperty:TargetFramework \
  -getProperty:OutputPath \
  -getProperty:PublishAot \
  -getItem:ProjectReference
```

That single mechanism gives structured project discovery without reimplementing MSBuild's
evaluation model — including `Directory.Build.props` inheritance, conditions, and imports, which
is exactly the part that makes hand-parsing hopeless.

Solution files (`.sln`, and newer `.slnx`) enumerate projects; each publishable project becomes
a `Target` in the multi-target model, feeding the target sidebar directly.

---

## 5. Trim and AOT warnings as a findings source

This has no analogue in the Rust or web backends and is genuinely valuable.

When trimming or compiling AOT, the toolchain emits warnings identifying code that **cannot be
statically analyzed** — reflection over types that might be trimmed, dynamic code generation
that AOT cannot support:

| Code | Meaning |
|---|---|
| `IL2026` | Uses a member annotated `RequiresUnreferencedCode` |
| `IL2xxx` | Trim analysis warnings generally |
| `IL3050` | Uses a member annotated `RequiresDynamicCode` — blocks AOT |
| `IL3xxx` | AOT compatibility warnings generally |

By default these are aggregated per assembly. Turn that off to get per-site detail: ⚠

```xml
<TrimmerSingleWarn>false</TrimmerSingleWarn>
```

**Each warning is a `Finding` with a source location, a deterministic origin, and a known class
of remedy.** "You cannot trim this because of reflection at `Foo.cs:42`, here is the annotation
or the source-generator alternative" is a well-grounded, actionable finding that requires no
model to produce and benefits from one to explain.

This makes trim analysis the C# backend's answer to monomorphization findings: the
differentiated, deterministic finding type that justifies the backend.

---

## 6. CoreCLR specifics

### 6.1 Size

No native artifact, so attribution works at a different granularity:

- **Deployment level** — `deps.json` lists every assembly shipped; file sizes are directly
  measurable. Often the most actionable view, since the answer is usually "this dependency is
  8 MB and you use one method from it."
- **Assembly level** — ECMA-335 metadata gives per-type and per-method IL sizes. Reading this
  from Rust means either a bridge using `System.Reflection.Metadata` (easy) or an ECMA-335
  reader in Rust (real work, poor ecosystem support). **Use the bridge.**
- **Runtime footprint** is a different question again and needs a dump (§6.3).

### 6.2 Portable PDB

The source mapping layer. A documented, comparatively simple format carrying sequence points
that map IL offsets to source lines — the Portable PDB analogue of DWARF's line table.

Two paths: a Rust reader (feasible; the format is far simpler than DWARF) or the bridge via
`System.Reflection.Metadata`. Start with the bridge, and consider a Rust reader later only if
lookup latency in a hot path justifies it.

Note that PDBs may be embedded in the assembly, sat beside it, or fetched from a symbol server —
handle at least the first two, and verify the PDB matches the assembly by its ID before
trusting it, for the same reason DWARF builds get a staleness check.

### 6.3 Managed dumps

```bash
dotnet-dump collect -p <pid>
dotnet-dump analyze core_dump
```

Analysis goes through **ClrMD** (`Microsoft.Diagnostics.Runtime`), which is a .NET library.
There is no Rust equivalent and writing one is out of the question — this is the strongest
argument for the bridge existing at all.

ClrMD gives managed threads and their stacks, the managed heap with per-type instance counts and
sizes, exception objects with inner exceptions, and object references for retention analysis.

A managed dump is a different kind of artifact from a core dump: better in some ways (fully
typed objects, real heap statistics, no DWARF reconstruction) and worse in others (no
instruction-level view, no register recovery). The Crash panel needs a distinct layout for it,
not a degraded version of the native one — this is exactly the shallow-target trap in
`DESIGN-POLYGLOT.md` §5.6.

### 6.4 Profiling

```bash
dotnet-trace collect -p <pid> --profile cpu-sampling
dotnet-trace convert trace.nettrace --format speedscope
```

The `.nettrace` format is EventPipe's native output; converting to speedscope JSON gives a
weighted call tree the existing flamegraph view consumes directly. That conversion step is the
pragmatic path — do not parse `.nettrace` yourself.

For `perf` interoperability with JIT-compiled frames, the runtime can emit a perf map: ⚠

```bash
DOTNET_PerfMapEnabled=1 dotnet run
```

Without it, JIT frames appear as unresolved addresses. With it, `perf` resolves managed methods
and the native attribution path works.

### 6.5 Benchmarking

**BenchmarkDotNet** is the ecosystem standard and fills Criterion's role. It emits structured
JSON results with statistics it computed itself, and it handles JIT warm-up, which matters more
in .NET than in Rust.

For whole-process timing, `hyperfine` applies unchanged. Startup time is a first-class metric
for the AOT question and is exactly a whole-process measurement.

### 6.6 Tests

```bash
dotnet test --logger "trx;LogFileName=results.trx"
```

TRX is XML and straightforward to parse. This is the verifier's test gate for both backends.

---

## 7. The .NET bridge

`binmap-bridge-dotnet` is a small .NET helper Binmap spawns on demand, for the same reason the
Node bridge exists: some of this cannot be done from Rust.

**What must cross the bridge:**

| Operation | Why |
|---|---|
| Managed dump analysis | ClrMD is .NET-only, with no alternative |
| `.mstat` reading | It is a managed assembly |
| ECMA-335 / IL metadata | `System.Reflection.Metadata` is far ahead of anything in Rust |
| Portable PDB (initially) | Same library; a Rust reader is a later optimization |

**What must not:**

`.dgml` (XML — parse in Rust), MSBuild property queries (JSON on stdout), speedscope profiles
(JSON), ELF and DWARF for NativeAOT (`TOOLING-BINARY.md` applies unchanged), trim warnings
(build output).

**Lifecycle and requirements:** JSON-RPC over stdio, spawned lazily when a .NET target is opened,
killed with the session. It requires a .NET runtime on the user's machine — an Environment panel
entry, grouped under .NET targets so a Rust-only user never sees it.

---

## 8. Verifier gates

| Gate | NativeAOT | CoreCLR |
|---|---|---|
| Build / publish succeeds | ✓ | ✓ |
| Tests pass (`dotnet test`) | ✓ | ✓ |
| No new trim/AOT warnings | ✓ | ✓ when trimming |
| Artifact size | ✓ | ✓ deployment size |
| Startup time | ✓ | ✓ — the AOT headline metric |
| Benchmark (BenchmarkDotNet) | ✓ | ✓ |

**"No new trim warnings" is a genuinely useful gate** with no equivalent in the other backends.
A patch that makes code less trimmable is a regression even when size happens not to move on
this build, because it will move on the next one.

---

## 9. Fragility

| Dependency | Fragility | Why | Mitigation |
|---|---|---|---|
| ILC `.mstat` schema | **High** | Undocumented, changes across SDK versions | Snapshot fixtures per SDK; degrade to symbol-table attribution |
| ILC property names | **High** | Not covered by compatibility guarantees | Probe support at Environment-panel time; feature-detect, never assume |
| NativeAOT DWARF quality | **High** ⚠ | Assumed, not verified | Spike before Phase 2.5. Size no longer depends on it (§2). |
| `.dgml` format | Medium | XML, stable-ish, undocumented | Parse defensively; ignore unknown elements |
| ClrMD across runtime versions | Medium | Must match target runtime | Bridge targets a range; report clearly on mismatch |
| MSBuild `-getProperty` | Medium ⚠ | Newer SDK feature | Detect; fall back to a binlog or targets file |
| `dotnet-trace` / speedscope | Low | Stable conversion path | Snapshot test |
| BenchmarkDotNet JSON | Low | Stable | Snapshot test |
| TRX | Low | Stable XML | Snapshot test |

**The pattern to notice:** the C# backend's highest-value features rest on its highest-fragility
inputs. `.mstat` and `.dgml` are the differentiators and neither carries a stability guarantee.

Build both behind a capability probe that degrades cleanly to plain symbol attribution, and
treat an SDK upgrade breaking them as an expected maintenance event rather than an emergency.
That is the same discipline as pinning `bloaty` output fixtures, applied to a less stable
dependency.
