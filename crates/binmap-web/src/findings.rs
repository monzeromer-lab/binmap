//! Findings the deterministic layer can produce (`TOOLING-WEB §6`).
//!
//! "The web equivalents of monomorphization bloat, and each is detectable
//! without a model." That last clause is the design point rather than an
//! optimisation: `§2.4` says the model may not lead an analysis, and every rule
//! here is a named pattern over the metafile, so each carries `Derived`
//! provenance with the rule that produced it — never `Measured`, because the
//! *bytes* are measured and the *conclusion* is ours.
//!
//! Each rule states what it costs to act on, because every one of them trades
//! something. A deep import defeats a package's own API design; an `overrides`
//! entry pins a version its dependents did not choose. A finding showing only
//! the saving is one that will be regretted, which is the same rule the native
//! backend's collapse strategies follow.

use crate::metafile::{BundleAttribution, ModuleCost};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Below this, a finding is noise in a list.
///
/// Smaller than the native threshold because web bytes are transfer bytes: a
/// kilobyte over the network matters more than a kilobyte on disk.
pub const WORTH_REPORTING: u64 = 512;

/// What kind of problem this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    /// Two versions of one package in a single bundle.
    DuplicateDependency,
    /// A package included whole for one small import.
    WholeLibraryImport,
    /// A re-export barrel pulling in a subtree.
    BarrelImport,
    /// Node's standard library, shimmed into a browser bundle.
    NodePolyfill,
    /// `core-js` and friends, present because of an old browser target.
    Polyfill,
}

impl Kind {
    /// The rule name recorded as provenance.
    ///
    /// Named because `Derived` without a rule name is indistinguishable from a
    /// guess.
    pub fn rule(self) -> &'static str {
        match self {
            Kind::DuplicateDependency => "web-duplicate-dependency",
            Kind::WholeLibraryImport => "web-whole-library-import",
            Kind::BarrelImport => "web-barrel-import",
            Kind::NodePolyfill => "web-node-polyfill",
            Kind::Polyfill => "web-polyfill-cost",
        }
    }

    /// What acting on it costs. Never empty.
    pub fn cost(self) -> &'static str {
        match self {
            Kind::DuplicateDependency => {
                "An `overrides` or `resolutions` entry pins a version that some dependent did not \
                 choose. Check that the versions are actually compatible before forcing one — a \
                 major-version gap usually means they are not."
            }
            Kind::WholeLibraryImport => {
                "A subpath import defeats the package's own API design and can break on a minor \
                 release, because subpaths are only a contract where `exports` declares them."
            }
            Kind::BarrelImport => {
                "Deep imports are more brittle than the barrel they replace: the barrel is the \
                 package's public surface and its internals are not."
            }
            Kind::NodePolyfill => {
                "Removing the shim means the dependency stops working in the browser unless the \
                 code path is genuinely unused. Confirm it is before dropping it."
            }
            Kind::Polyfill => {
                "Dropping an old browser target is a decision about who can use the product, not \
                 a build setting. The bytes are real and so are the users."
            }
        }
    }
}

/// One deterministic finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    pub kind: Kind,
    pub title: String,
    pub detail: String,
    /// An upper bound on what acting on it saves, and labelled as one
    /// everywhere it is shown.
    pub saves_at_most: u64,
    /// The modules this is about, so the reader can go and look.
    pub modules: Vec<String>,
}

/// Every rule, applied.
///
/// Ordered by what is at stake, largest first, because a list ordered by
/// anything else asks the reader to do the ranking.
pub fn analyse(attribution: &BundleAttribution) -> Vec<Finding> {
    let modules = attribution.modules();
    let mut findings = Vec::new();

    findings.extend(duplicate_dependencies(&modules));
    findings.extend(node_polyfills(&modules));
    findings.extend(polyfills(&modules));
    // Packages already explained by a more specific rule are not reported
    // again as a generic whole-library import. core-js *is* a whole library
    // included whole — that is what a polyfill bundle is — and saying so twice
    // double-counts the same bytes and buries the finding that actually tells
    // the reader what to do about them.
    let already: std::collections::BTreeSet<String> = findings
        .iter()
        .flat_map(|finding| finding.modules.iter())
        .filter_map(|module| crate::metafile::package_of(module))
        .collect();

    findings.extend(whole_library_imports(attribution, &modules, &already));
    findings.extend(barrel_imports(attribution, &modules));

    findings.retain(|finding| finding.saves_at_most >= WORTH_REPORTING);
    findings.sort_by_key(|finding| std::cmp::Reverse(finding.saves_at_most));
    findings
}

/// Two versions of one package in one bundle (`§6`).
///
/// The clearest win available, because nobody chooses it: it is what a
/// dependency tree does when two packages disagree about a version.
fn duplicate_dependencies(modules: &[ModuleCost]) -> Vec<Finding> {
    // Group by package, then by the path prefix that distinguishes copies. Two
    // modules of `lodash` under different `node_modules` roots are two copies;
    // two modules under the same root are just two files.
    let mut roots: BTreeMap<&str, BTreeMap<String, u64>> = BTreeMap::new();
    for module in modules {
        let Some(package) = module.package.as_deref() else { continue };
        let root = copy_root(&module.module, package);
        *roots.entry(package).or_default().entry(root).or_insert(0) += module.bytes;
    }

    roots
        .into_iter()
        .filter(|(_, copies)| copies.len() > 1)
        .map(|(package, copies)| {
            let total: u64 = copies.values().sum();
            let largest = copies.values().copied().max().unwrap_or(0);
            let paths: Vec<String> = copies.keys().cloned().collect();

            Finding {
                kind: Kind::DuplicateDependency,
                title: format!("{package} is in the bundle {} times", copies.len()),
                detail: format!(
                    "{} copies totalling {total} bytes. Deduplicating to one would save at most \
                     {} — the largest copy has to stay. Found at: {}. Check which versions these \
                     are with `npm ls {package}` before forcing one.",
                    copies.len(),
                    total - largest,
                    paths.join(", ")
                ),
                saves_at_most: total - largest,
                modules: paths,
            }
        })
        .collect()
}

/// Which `node_modules` root a module's copy lives under.
///
/// `a/node_modules/b/x.js` and `node_modules/b/x.js` are different copies of
/// `b`; `node_modules/b/x.js` and `node_modules/b/y.js` are not.
fn copy_root(module: &str, package: &str) -> String {
    match module.rfind(&format!("node_modules/{package}")) {
        Some(at) => module[..at + format!("node_modules/{package}").len()].to_string(),
        None => module.to_string(),
    }
}

/// Node's standard library, shimmed into a browser bundle (`§6`).
///
/// Nobody asks for these. They arrive because a dependency assumed it was
/// running on a server, and they are frequently large.
fn node_polyfills(modules: &[ModuleCost]) -> Vec<Finding> {
    const SHIMS: [&str; 10] = [
        "buffer",
        "process",
        "crypto-browserify",
        "stream-browserify",
        "path-browserify",
        "os-browserify",
        "util",
        "assert",
        "readable-stream",
        "browserify-sign",
    ];

    let found: Vec<&ModuleCost> = modules
        .iter()
        .filter(|module| module.package.as_deref().is_some_and(|package| SHIMS.contains(&package)))
        .collect();

    if found.is_empty() {
        return Vec::new();
    }
    let total: u64 = found.iter().map(|module| module.bytes).sum();
    let packages: Vec<String> = found
        .iter()
        .filter_map(|module| module.package.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();

    vec![Finding {
        kind: Kind::NodePolyfill,
        title: format!("{total} bytes of Node shims in a browser bundle"),
        detail: format!(
            "{} is here because something in the tree assumed it was running on a server. \
             Nobody chose this, which is what makes it worth looking at: find which dependency \
             pulls it in before shimming it, because the answer is often a package with a \
             browser-native alternative.",
            packages.join(", ")
        ),
        saves_at_most: total,
        modules: found.iter().map(|module| module.module.clone()).collect(),
    }]
}

/// `core-js` and friends, present because of an old browser target (`§6`).
///
/// "Dropping IE11 saves 34 KB gzipped" is a business decision the tool can put
/// a number on, which is worth more than most build advice.
fn polyfills(modules: &[ModuleCost]) -> Vec<Finding> {
    let found: Vec<&ModuleCost> = modules
        .iter()
        .filter(|module| {
            module.package.as_deref().is_some_and(|package| {
                package.starts_with("core-js") || package == "regenerator-runtime"
            })
        })
        .collect();

    if found.is_empty() {
        return Vec::new();
    }
    let total: u64 = found.iter().map(|module| module.bytes).sum();

    vec![Finding {
        kind: Kind::Polyfill,
        title: format!("{total} bytes of polyfills for older browsers"),
        detail: format!(
            "{} polyfill modules are in the bundle because the build targets browsers that lack \
             the features they replace. Raising the target removes them, and the sweep can \
             measure exactly how much each target level costs.",
            found.len()
        ),
        saves_at_most: total,
        modules: found.iter().map(|module| module.module.clone()).collect(),
    }]
}

/// A large package pulled in whole for one import (`§6`).
///
/// Detected from the graph: one import edge into a package that contributes
/// many modules. The classic is `import { debounce } from "lodash"` costing the
/// whole library.
fn whole_library_imports(
    attribution: &BundleAttribution,
    modules: &[ModuleCost],
    already_explained: &std::collections::BTreeSet<String>,
) -> Vec<Finding> {
    let mut by_package: BTreeMap<&str, (u64, usize)> = BTreeMap::new();
    for module in modules {
        let Some(package) = module.package.as_deref() else { continue };
        let entry = by_package.entry(package).or_insert((0, 0));
        entry.0 += module.bytes;
        entry.1 += 1;
    }

    by_package
        .into_iter()
        .filter(|(package, _)| !already_explained.contains(*package))
        .filter(|(package, (bytes, files))| {
            // Reached from few places, and either many modules or one big one.
            //
            // The second half matters: lodash ships as a single 73 KB file, so
            // a rule counting *modules* misses the canonical whole-library
            // import entirely. It is the shape of the import that is wrong,
            // not the number of files the package happens to be split into.
            let few_callers = entry_points_into(attribution, package) <= 2;
            let bulky = *files >= 5 || *bytes >= 20_000;
            few_callers && bulky && *bytes >= WORTH_REPORTING
        })
        .map(|(package, (bytes, files))| {
            let callers = entry_points_into(attribution, package);
            Finding {
                kind: Kind::WholeLibraryImport,
                title: if files == 1 {
                    format!("{package} is one {bytes}-byte module, included whole")
                } else {
                    format!("{package} contributes {files} modules for {bytes} bytes")
                },
                detail: format!(
                    "Only {callers} import{} in this bundle reaches {package}, and {} included. \
                     That is the shape of a whole-library import — `import {{ one }} from \
                     \"{package}\"` where a subpath import, or a lighter alternative, would cost \
                     a fraction.",
                    if callers == 1 { "" } else { "s" },
                    if files == 1 {
                        "the whole file is".to_string()
                    } else {
                        format!("{files} of its modules are")
                    }
                ),
                saves_at_most: bytes,
                modules: vec![package.to_string()],
            }
        })
        .collect()
}

/// How many distinct modules outside a package import into it.
fn entry_points_into(attribution: &BundleAttribution, package: &str) -> usize {
    attribution
        .graph
        .iter()
        .filter(|edge| {
            crate::metafile::package_of(&edge.to).as_deref() == Some(package)
                && crate::metafile::package_of(&edge.from).as_deref() != Some(package)
        })
        .map(|edge| edge.from.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .len()
}

/// A re-export barrel pulling in a subtree (`§6`).
///
/// Detected as a module that imports many things and contributes almost
/// nothing itself: that is what a file of `export * from "./x"` compiles to.
fn barrel_imports(attribution: &BundleAttribution, modules: &[ModuleCost]) -> Vec<Finding> {
    let sizes: BTreeMap<&str, u64> =
        modules.iter().map(|module| (module.module.as_str(), module.bytes)).collect();

    let mut outgoing: BTreeMap<&str, usize> = BTreeMap::new();
    for edge in &attribution.graph {
        *outgoing.entry(edge.from.as_str()).or_insert(0) += 1;
    }

    outgoing
        .into_iter()
        .filter(|(module, imports)| {
            // Only in the reader's own code. A dependency's internal barrels
            // are its own business: `core-js/internals/get-async-iterator.js`
            // re-exports six modules, and nobody imports it directly or can do
            // anything about it. Reporting those flooded the list with
            // twenty entries nobody could act on and buried the polyfill
            // finding that actually said what to do.
            let ours = crate::metafile::package_of(module).is_none();
            // Many imports, and it contributes almost nothing of its own —
            // which is exactly a file that only re-exports.
            ours && *imports >= 5 && sizes.get(module).copied().unwrap_or(0) < 256
        })
        .filter_map(|(module, imports)| {
            // What the subtree it pulls in actually costs.
            let pulled: u64 = attribution
                .graph
                .iter()
                .filter(|edge| edge.from == module)
                .filter_map(|edge| sizes.get(edge.to.as_str()).copied())
                .sum();

            (pulled >= WORTH_REPORTING).then(|| Finding {
                kind: Kind::BarrelImport,
                title: format!("{module} re-exports {imports} modules costing {pulled} bytes"),
                detail: format!(
                    "This module contributes almost no bytes of its own but pulls in {imports} \
                     others, which is the shape of a re-export barrel. Importing directly from \
                     the file you need lets the bundler drop the rest — though only if nothing \
                     in that subtree has side effects.",
                ),
                saves_at_most: pulled,
                modules: vec![module.to_string()],
            })
        })
        .collect()
}
