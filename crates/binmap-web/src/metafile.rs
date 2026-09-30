//! esbuild metafile attribution (`TOOLING-WEB §3.1`, `§4.2`).
//!
//! The best tier available, and better than source maps in a way that matters:
//! it has module identity and an import graph, so it can answer *why* a module
//! is in the bundle rather than only that it is.
//!
//! The import graph is what makes `§4.2` possible. With code splitting the
//! total is the wrong headline — what a user waits for is the initial load, and
//! that is the entry chunk plus everything reachable from it through *static*
//! imports. A `dynamic-import` edge is precisely where the initial load stops,
//! so the distinction between the two kinds of edge is the whole calculation.
//!
//! One trap the format sets, and it is easy to walk into: a metafile lists
//! `.js.map` outputs alongside `.js` outputs, with their byte counts. Source
//! maps are not page weight — a browser fetches one only when devtools are
//! open — so counting them inflates every number, and by a lot, because a map
//! is often larger than the code it describes.

use crate::tier::Tier;
use binmap_core::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// esbuild's metafile, as much of it as we read.
#[derive(Debug, Clone, Deserialize)]
struct Metafile {
    #[serde(default)]
    inputs: BTreeMap<String, Input>,
    #[serde(default)]
    outputs: BTreeMap<String, Output>,
}

#[derive(Debug, Clone, Deserialize)]
struct Input {
    #[serde(default)]
    bytes: u64,
    #[serde(default)]
    imports: Vec<Import>,
}

#[derive(Debug, Clone, Deserialize)]
struct Import {
    path: String,
    #[serde(default)]
    kind: String,
}

#[derive(Debug, Clone, Deserialize)]
struct Output {
    #[serde(default)]
    bytes: u64,
    #[serde(default)]
    imports: Vec<Import>,
    #[serde(default)]
    inputs: BTreeMap<String, InputInOutput>,
    #[serde(rename = "entryPoint")]
    entry_point: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct InputInOutput {
    #[serde(rename = "bytesInOutput", default)]
    bytes_in_output: u64,
}

/// Why a chunk is fetched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Load {
    /// An entry point. The user waits for this.
    Entry,
    /// Reachable from an entry through static imports only. The user waits for
    /// this too, whether or not they know it.
    Initial,
    /// Behind a dynamic import. Fetched later, or never.
    Lazy,
}

impl Load {
    pub fn label(self) -> &'static str {
        match self {
            Load::Entry => "entry",
            Load::Initial => "initial",
            Load::Lazy => "lazy",
        }
    }

    /// Whether this is on the critical path.
    ///
    /// The predicate the headline number is built from (`§4.2`).
    pub fn is_on_the_critical_path(self) -> bool {
        matches!(self, Load::Entry | Load::Initial)
    }
}

/// One module's contribution to one chunk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleCost {
    pub module: String,
    /// Bytes this module contributes to this chunk *after* bundling and
    /// minification — not its size on disk, which is usually much larger.
    pub bytes: u64,
    /// The package it came from, where the path says so.
    ///
    /// `node_modules/lodash/x.js` is lodash's. The web equivalent of telling a
    /// dependency from your own code, and it comes from the path because
    /// nothing else records it.
    pub package: Option<String>,
}

/// One built chunk.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Chunk {
    pub path: String,
    pub bytes: u64,
    pub load: Load,
    pub entry_point: Option<String>,
    /// Largest first.
    pub modules: Vec<ModuleCost>,
}

/// One edge in the module graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edge {
    pub from: String,
    pub to: String,
    /// esbuild's own word: `import-statement`, `dynamic-import`, `require-call`
    /// and so on. Kept verbatim rather than mapped onto our vocabulary,
    /// because it is what the bundler said and a reader may know it already.
    pub kind: String,
}

/// Everything a metafile says about a build.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BundleAttribution {
    /// Largest first, critical path before lazy.
    pub chunks: Vec<Chunk>,
    pub tier: Tier,
    /// The module-level import graph, which is what lets this tier answer why
    /// a module is in the bundle at all (`§3`). Source maps cannot.
    pub graph: Vec<Edge>,
    /// Each module's size on disk before bundling, for the comparison that
    /// shows how much tree shaking and minification actually removed.
    pub source_bytes: BTreeMap<String, u64>,
}

impl BundleAttribution {
    /// The headline number (`§4.2`).
    ///
    /// Initial-load bytes, not the sum of everything. With code splitting the
    /// total is the wrong number to lead with: a route nobody visits costs
    /// nobody anything, and reporting it in the headline makes splitting look
    /// like it achieved less than it did.
    pub fn initial_load_bytes(&self) -> u64 {
        self.chunks
            .iter()
            .filter(|chunk| chunk.load.is_on_the_critical_path())
            .map(|chunk| chunk.bytes)
            .sum()
    }

    /// Everything, for the cases where that is the question.
    pub fn total_bytes(&self) -> u64 {
        self.chunks.iter().map(|chunk| chunk.bytes).sum()
    }

    pub fn lazy_bytes(&self) -> u64 {
        self.total_bytes() - self.initial_load_bytes()
    }

    /// Every module's cost across every chunk, largest first.
    ///
    /// A module split across chunks is counted in each, which is what it
    /// actually costs: duplication across chunks is real bytes, and summing it
    /// away would hide one of the things a reader most wants to find.
    pub fn modules(&self) -> Vec<ModuleCost> {
        let mut totals: BTreeMap<&str, (u64, Option<&str>)> = BTreeMap::new();
        for chunk in &self.chunks {
            for module in &chunk.modules {
                let entry = totals.entry(&module.module).or_insert((0, module.package.as_deref()));
                entry.0 += module.bytes;
            }
        }
        let mut modules: Vec<ModuleCost> = totals
            .into_iter()
            .map(|(module, (bytes, package))| ModuleCost {
                module: module.to_string(),
                bytes,
                package: package.map(str::to_string),
            })
            .collect();
        modules.sort_by_key(|module| std::cmp::Reverse(module.bytes));
        modules
    }

    /// What each package costs, largest first.
    ///
    /// The web equivalent of per-crate attribution. Modules outside
    /// `node_modules` are grouped as the user's own code.
    pub fn packages(&self) -> Vec<(String, u64)> {
        let mut totals: BTreeMap<String, u64> = BTreeMap::new();
        for module in self.modules() {
            let key = module.package.unwrap_or_else(|| "your code".to_string());
            *totals.entry(key).or_insert(0) += module.bytes;
        }
        let mut packages: Vec<(String, u64)> = totals.into_iter().collect();
        packages.sort_by_key(|(_, bytes)| std::cmp::Reverse(*bytes));
        packages
    }

    /// Why this module is in the bundle (`§3`).
    ///
    /// The shortest import chain from an entry point, as the bundler recorded
    /// it. This is the question a size tool is really asked — "what is
    /// pulling this in?" — and the one source maps cannot answer at all,
    /// which is why the tier that can is worth preferring.
    ///
    /// `None` when the module is not reachable from any entry, which means the
    /// metafile disagrees with itself and is worth knowing rather than
    /// papering over.
    pub fn why_is_this_here(&self, module: &str) -> Option<Vec<Edge>> {
        // Only chunks classified as a real entry. A lazily imported chunk also
        // carries an `entryPoint` under `--splitting`, and treating that as a
        // root made a dynamically imported module look like an entry point —
        // the same mistake as in the load classification, in a second place.
        let entries: BTreeSet<&str> = self
            .chunks
            .iter()
            .filter(|chunk| chunk.load == Load::Entry)
            .filter_map(|chunk| chunk.entry_point.as_deref())
            .collect();
        if entries.contains(module) {
            // It is an entry point. Nothing pulled it in; it is why everything
            // else is here.
            return Some(Vec::new());
        }

        // Breadth-first from every entry, so the chain found first is the
        // shortest — the most useful answer rather than an arbitrary one.
        let mut seen: BTreeSet<&str> = entries.iter().copied().collect();
        let mut queue: VecDeque<(&str, Vec<Edge>)> =
            entries.iter().map(|entry| (*entry, Vec::new())).collect();

        while let Some((current, path)) = queue.pop_front() {
            for edge in self.graph.iter().filter(|edge| edge.from == current) {
                let mut extended = path.clone();
                extended.push(edge.clone());
                if edge.to == module {
                    return Some(extended);
                }
                if seen.insert(edge.to.as_str()) {
                    queue.push_back((edge.to.as_str(), extended));
                }
            }
        }
        None
    }

    /// Why this module is here, as a sentence.
    pub fn explain(&self, module: &str) -> String {
        match self.why_is_this_here(module) {
            Some(chain) if chain.is_empty() => {
                format!("{module} is an entry point — it is why the rest is here.")
            }
            Some(chain) => {
                let mut line = String::new();
                for (index, edge) in chain.iter().enumerate() {
                    if index == 0 {
                        line.push_str(&edge.from);
                    }
                    line.push_str(&format!(" ──{}──> {}", edge.kind, edge.to));
                }
                line
            }
            None => format!(
                "{module} is in the bundle, but no import chain from an entry point reaches it. \
                 That is the metafile disagreeing with itself."
            ),
        }
    }

    /// How much smaller a module got on the way into the bundle.
    ///
    /// `None` when the metafile does not record its source size. A module that
    /// barely shrank is usually one tree shaking could not touch — a side
    /// effect, or a namespace import — which is a different problem from one
    /// that is simply large.
    pub fn shrinkage(&self, module: &str) -> Option<(u64, u64)> {
        let before = *self.source_bytes.get(module)?;
        let after: u64 = self
            .chunks
            .iter()
            .flat_map(|chunk| chunk.modules.iter())
            .filter(|cost| cost.module == module)
            .map(|cost| cost.bytes)
            .sum();
        Some((before, after))
    }

    /// The sentence shown beside the result.
    pub fn describe(&self) -> String {
        format!(
            "{} bytes on the initial load across {} chunks; {} bytes more behind dynamic \
             imports. {}",
            self.initial_load_bytes(),
            self.chunks.iter().filter(|c| c.load.is_on_the_critical_path()).count(),
            self.lazy_bytes(),
            self.tier.describe()
        )
    }
}

/// Read an esbuild metafile.
pub fn attribute(metafile: &[u8]) -> Result<BundleAttribution> {
    let parsed: Metafile = serde_json::from_slice(metafile).map_err(|error| {
        Error::Other(format!(
            "this is not an esbuild metafile: {error}. Produce one with `esbuild --metafile=…`."
        ))
    })?;

    if parsed.outputs.is_empty() {
        return Err(Error::Other(
            "this metafile lists no outputs, so there is nothing to attribute. An empty build \
             usually means the entry point resolved to nothing."
                .into(),
        ));
    }

    // Which chunks something dynamically imports. Under `--splitting` esbuild
    // gives a dynamic import target its own `entryPoint`, so that field alone
    // says nothing about whether the user waits for it — a real build proved
    // this, after a synthetic fixture written to match the wrong assumption
    // had passed.
    let lazily_imported = lazily_imported(&parsed);
    let critical = critical_path(&parsed, &lazily_imported);

    let mut chunks: Vec<Chunk> = parsed
        .outputs
        .iter()
        // A source map is not page weight: a browser fetches one only when
        // devtools are open. Counting them inflates every number, and by a
        // lot — a map is often larger than the code it describes.
        .filter(|(path, _)| !is_a_source_map(path))
        .map(|(path, output)| {
            let load = if !critical.contains(path.as_str()) {
                Load::Lazy
            } else if output.entry_point.is_some() && !lazily_imported.contains(path.as_str()) {
                Load::Entry
            } else {
                Load::Initial
            };

            let mut modules: Vec<ModuleCost> = output
                .inputs
                .iter()
                .filter(|(_, cost)| cost.bytes_in_output > 0)
                .map(|(module, cost)| ModuleCost {
                    module: module.clone(),
                    bytes: cost.bytes_in_output,
                    package: package_of(module),
                })
                .collect();
            modules.sort_by_key(|module| std::cmp::Reverse(module.bytes));

            Chunk {
                path: path.clone(),
                bytes: output.bytes,
                load,
                entry_point: output.entry_point.clone(),
                modules,
            }
        })
        .collect();

    // Critical path first, then largest — because the first question is what
    // the user waits for, and the second is what is big.
    chunks.sort_by_key(|chunk| {
        (!chunk.load.is_on_the_critical_path(), std::cmp::Reverse(chunk.bytes))
    });

    let graph = parsed
        .inputs
        .iter()
        .flat_map(|(module, input)| {
            input.imports.iter().map(move |import| Edge {
                from: module.clone(),
                to: import.path.clone(),
                kind: if import.kind.is_empty() {
                    "import".to_string()
                } else {
                    import.kind.clone()
                },
            })
        })
        .collect();

    let source_bytes =
        parsed.inputs.iter().map(|(module, input)| (module.clone(), input.bytes)).collect();

    Ok(BundleAttribution { chunks, tier: Tier::EsbuildMetafile, graph, source_bytes })
}

/// Every output that something dynamically imports.
///
/// The load-bearing distinction. esbuild marks a code-split chunk with its own
/// `entryPoint`, so the only thing that separates "an entry point the build was
/// asked for" from "a chunk that exists because someone wrote `await import`"
/// is whether anything reaches it through a dynamic edge.
fn lazily_imported(parsed: &Metafile) -> BTreeSet<&str> {
    parsed
        .outputs
        .values()
        .flat_map(|output| output.imports.iter())
        .filter(|import| import.kind == "dynamic-import")
        .map(|import| import.path.as_str())
        .collect()
}

/// Which outputs the user waits for.
///
/// Breadth-first from every *real* entry chunk, following only static import
/// edges. A `dynamic-import` is exactly where the initial load stops — that is
/// the point of writing one — so it is neither a root nor traversed.
fn critical_path<'a>(parsed: &'a Metafile, lazily_imported: &BTreeSet<&str>) -> BTreeSet<&'a str> {
    let mut reachable = BTreeSet::new();
    let mut queue: VecDeque<&str> = parsed
        .outputs
        .iter()
        .filter(|(path, output)| {
            output.entry_point.is_some() && !lazily_imported.contains(path.as_str())
        })
        .map(|(path, _)| path.as_str())
        .collect();

    for entry in &queue {
        reachable.insert(*entry);
    }

    while let Some(path) = queue.pop_front() {
        let Some(output) = parsed.outputs.get(path) else { continue };
        for import in &output.imports {
            if import.kind == "dynamic-import" {
                continue;
            }
            // `insert` returning false means we have been here, which is also
            // how a cycle in the chunk graph terminates.
            if reachable.insert(import.path.as_str()) {
                queue.push_back(import.path.as_str());
            }
        }
    }
    reachable
}

/// Whether an output is a source map rather than something a user downloads.
fn is_a_source_map(path: &str) -> bool {
    path.ends_with(".map")
}

/// The package a module path belongs to, where it says so.
///
/// Handles scoped packages, which is the case a naive split gets wrong:
/// `node_modules/@scope/name/file.js` is `@scope/name`, not `@scope`.
pub fn package_of(module: &str) -> Option<String> {
    let after = module.rsplit_once("node_modules/").map(|(_, rest)| rest)?;
    let mut parts = after.split('/');
    let first = parts.next()?;
    if first.starts_with('@') {
        let second = parts.next()?;
        return Some(format!("{first}/{second}"));
    }
    (!first.is_empty()).then(|| first.to_string())
}
