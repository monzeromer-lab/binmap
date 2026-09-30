//! Deterministic web findings (`TOOLING-WEB §6`).
//!
//! Mostly restraint, as with the native collapse strategies: each rule fires
//! only when the measurement supports it, and never claims a saving is
//! certain.

use binmap_web::findings::{Kind, WORTH_REPORTING, analyse};
use binmap_web::metafile::attribute;
use serde_json::{Value, json};

/// A metafile from modules and their bundled sizes, plus import edges.
fn build(modules: &[(&str, u64)], edges: &[(&str, &str)]) -> Value {
    let mut inputs = serde_json::Map::new();
    for (module, _) in modules {
        let imports: Vec<Value> = edges
            .iter()
            .filter(|(from, _)| from == module)
            .map(|(_, to)| json!({"path": to, "kind": "import-statement"}))
            .collect();
        inputs.insert(module.to_string(), json!({"bytes": 1000, "imports": imports}));
    }

    let mut in_output = serde_json::Map::new();
    for (module, bytes) in modules {
        in_output.insert(module.to_string(), json!({"bytesInOutput": bytes}));
    }

    json!({
        "inputs": inputs,
        "outputs": {
            "out/main.js": {
                "bytes": modules.iter().map(|(_, b)| b).sum::<u64>(),
                "entryPoint": "src/main.ts",
                "inputs": in_output,
                "imports": []
            }
        }
    })
}

fn findings(modules: &[(&str, u64)], edges: &[(&str, &str)]) -> Vec<binmap_web::findings::Finding> {
    let metafile = build(modules, edges).to_string();
    analyse(&attribute(metafile.as_bytes()).expect("a valid metafile"))
}

// --- duplicate dependencies -------------------------------------------------

#[test]
fn two_copies_of_one_package_are_found() {
    // The clearest win available, because nobody chooses it: it is what a
    // dependency tree does when two packages disagree about a version.
    let found = findings(
        &[
            ("node_modules/lodash/index.js", 8_000),
            ("node_modules/a/node_modules/lodash/index.js", 7_000),
        ],
        &[],
    );

    let duplicate = found
        .iter()
        .find(|finding| finding.kind == Kind::DuplicateDependency)
        .expect("two copies of lodash");
    assert!(duplicate.title.contains("lodash"), "{}", duplicate.title);
    assert_eq!(
        duplicate.saves_at_most, 7_000,
        "the larger copy has to stay, so only the smaller is saved"
    );
    assert!(
        duplicate.detail.contains("npm ls lodash"),
        "it says how to check: {}",
        duplicate.detail
    );
}

#[test]
fn two_files_of_the_same_copy_are_not_a_duplicate() {
    // The mistake a naive grouping makes: `lodash/a.js` and `lodash/b.js` are
    // one copy of lodash, not two.
    let found =
        findings(&[("node_modules/lodash/a.js", 8_000), ("node_modules/lodash/b.js", 7_000)], &[]);
    assert!(
        !found.iter().any(|finding| finding.kind == Kind::DuplicateDependency),
        "one copy in two files is not a duplicate: {found:?}"
    );
}

#[test]
fn a_scoped_package_duplicated_keeps_its_scope() {
    let found = findings(
        &[
            ("node_modules/@scope/pkg/index.js", 4_000),
            ("node_modules/x/node_modules/@scope/pkg/index.js", 3_000),
        ],
        &[],
    );
    let duplicate = found.iter().find(|f| f.kind == Kind::DuplicateDependency).expect("duplicated");
    assert!(duplicate.title.contains("@scope/pkg"), "{}", duplicate.title);
}

// --- node shims -------------------------------------------------------------

#[test]
fn node_shims_in_a_browser_bundle_are_reported() {
    // Nobody asks for these. They arrive because a dependency assumed a
    // server.
    let found = findings(
        &[("node_modules/buffer/index.js", 12_000), ("node_modules/process/browser.js", 2_000)],
        &[],
    );

    let shim = found.iter().find(|f| f.kind == Kind::NodePolyfill).expect("shims");
    assert_eq!(shim.saves_at_most, 14_000);
    assert!(shim.detail.contains("assumed it was running on a server"), "{}", shim.detail);
}

#[test]
fn an_ordinary_package_is_not_mistaken_for_a_node_shim() {
    let found = findings(&[("node_modules/react/index.js", 40_000)], &[]);
    assert!(!found.iter().any(|f| f.kind == Kind::NodePolyfill));
}

// --- polyfills --------------------------------------------------------------

#[test]
fn polyfills_are_reported_with_the_number_the_decision_needs() {
    // "Dropping an old target saves N bytes" is a business decision the tool
    // can put a number on.
    let found = findings(
        &[
            ("node_modules/core-js/modules/es.array.flat.js", 3_000),
            ("node_modules/core-js/modules/es.promise.js", 9_000),
            ("node_modules/regenerator-runtime/runtime.js", 6_000),
        ],
        &[],
    );

    let polyfill = found.iter().find(|f| f.kind == Kind::Polyfill).expect("polyfills");
    assert_eq!(polyfill.saves_at_most, 18_000);
    assert!(polyfill.detail.contains("Raising the target"), "{}", polyfill.detail);
}

#[test]
fn dropping_an_old_browser_is_named_as_a_decision_about_people() {
    // The trade a size tool must not hide.
    assert!(Kind::Polyfill.cost().contains("who can use the product"), "{}", Kind::Polyfill.cost());
}

// --- whole-library imports --------------------------------------------------

#[test]
fn a_package_included_whole_for_one_import_is_found() {
    let mut modules: Vec<(String, u64)> =
        (0..10).map(|i| (format!("node_modules/lodash/f{i}.js"), 900)).collect();
    modules.push(("src/main.ts".to_string(), 100));
    let modules: Vec<(&str, u64)> = modules.iter().map(|(m, b)| (m.as_str(), *b)).collect();

    let found = findings(&modules, &[("src/main.ts", "node_modules/lodash/f0.js")]);
    let whole =
        found.iter().find(|f| f.kind == Kind::WholeLibraryImport).expect("one import, ten modules");
    assert!(whole.title.contains("lodash"), "{}", whole.title);
    assert!(whole.detail.contains("subpath import"), "{}", whole.detail);
}

#[test]
fn a_package_reached_from_many_places_is_being_used_as_intended() {
    // React imported from twenty components is not a whole-library import, it
    // is a framework. Reporting it would train the reader to ignore the list.
    let mut modules: Vec<(String, u64)> =
        (0..10).map(|i| (format!("node_modules/react/f{i}.js"), 900)).collect();
    let mut edges: Vec<(String, String)> = Vec::new();
    for i in 0..8 {
        modules.push((format!("src/c{i}.tsx"), 100));
        edges.push((format!("src/c{i}.tsx"), "node_modules/react/f0.js".to_string()));
    }

    let modules: Vec<(&str, u64)> = modules.iter().map(|(m, b)| (m.as_str(), *b)).collect();
    let edges: Vec<(&str, &str)> = edges.iter().map(|(f, t)| (f.as_str(), t.as_str())).collect();

    let found = findings(&modules, &edges);
    assert!(
        !found.iter().any(|f| f.kind == Kind::WholeLibraryImport),
        "a widely used framework is not a whole-library import: {found:?}"
    );
}

#[test]
fn a_small_single_file_package_is_left_alone() {
    // A package that is genuinely small is being used as intended, whatever
    // shape it ships in.
    let found = findings(
        &[("node_modules/tiny/index.js", 3_000), ("src/main.ts", 100)],
        &[("src/main.ts", "node_modules/tiny/index.js")],
    );
    assert!(!found.iter().any(|f| f.kind == Kind::WholeLibraryImport), "{found:?}");
}

#[test]
fn a_monolithic_package_is_flagged_even_though_it_is_one_file() {
    // lodash ships as a single 73 KB file, so a rule counting *modules* missed
    // the canonical whole-library import entirely. It is the shape of the
    // import that is wrong, not how many files the package is split into.
    let found = findings(
        &[("node_modules/lodash/lodash.js", 73_000), ("src/main.ts", 100)],
        &[("src/main.ts", "node_modules/lodash/lodash.js")],
    );
    let whole = found
        .iter()
        .find(|f| f.kind == Kind::WholeLibraryImport)
        .expect("a 73 KB single-file package pulled in for one import");
    assert!(whole.title.contains("included whole"), "{}", whole.title);
    assert!(whole.detail.contains("subpath import"), "{}", whole.detail);
}

#[test]
fn a_dependencys_own_internal_barrels_are_not_reported() {
    // `core-js/internals/get-async-iterator.js` re-exports six modules, and
    // nobody imports it directly or can do anything about it. Reporting those
    // flooded the list with entries nobody could act on.
    let names: Vec<String> =
        (0..8).map(|i| format!("node_modules/dep/internals/m{i}.js")).collect();
    let mut modules: Vec<(&str, u64)> = vec![("node_modules/dep/internals/barrel.js", 40)];
    modules.extend(names.iter().map(|name| (name.as_str(), 900)));
    let edges: Vec<(&str, &str)> =
        names.iter().map(|name| ("node_modules/dep/internals/barrel.js", name.as_str())).collect();

    let found = findings(&modules, &edges);
    assert!(
        !found.iter().any(|f| f.kind == Kind::BarrelImport),
        "a dependency's internal barrel is its own business: {found:?}"
    );
}

#[test]
fn a_package_already_explained_by_a_better_rule_is_not_reported_twice() {
    // core-js *is* a whole library included whole — that is what a polyfill
    // bundle is. Saying so twice double-counts the bytes and buries the
    // finding that tells the reader what to do about them.
    let names: Vec<String> =
        (0..10).map(|i| format!("node_modules/core-js/modules/m{i}.js")).collect();
    let modules: Vec<(&str, u64)> = names.iter().map(|name| (name.as_str(), 3_000)).collect();

    let found = findings(&modules, &[]);
    assert_eq!(
        found.iter().filter(|f| f.kind == Kind::Polyfill).count(),
        1,
        "the polyfill rule is the one that applies"
    );
    assert!(
        !found.iter().any(|f| f.kind == Kind::WholeLibraryImport),
        "and it is not also reported generically: {found:?}"
    );
}

// --- barrels ----------------------------------------------------------------

#[test]
fn a_re_export_barrel_is_found_by_its_shape() {
    // Almost no bytes of its own, many imports — which is exactly what a file
    // of `export * from "./x"` compiles to.
    // Owned first, borrowed after: leaking the names would be the very thing
    // this codebase refuses to do in production.
    let names: Vec<String> = (0..8).map(|i| format!("src/m{i}.ts")).collect();
    let mut modules: Vec<(&str, u64)> = vec![("src/index.ts", 40)];
    modules.extend(names.iter().map(|name| (name.as_str(), 900)));
    let edges: Vec<(&str, &str)> =
        names.iter().map(|name| ("src/index.ts", name.as_str())).collect();

    let found = findings(&modules, &edges);
    let barrel = found.iter().find(|f| f.kind == Kind::BarrelImport).expect("a barrel");
    assert!(barrel.title.contains("src/index.ts"), "{}", barrel.title);
    assert!(barrel.saves_at_most >= 7_000);
}

#[test]
fn a_module_that_imports_a_lot_and_is_large_itself_is_not_a_barrel() {
    // A barrel is defined by contributing nothing of its own. A big module
    // with many imports is just a big module.
    let names: Vec<String> = (0..8).map(|i| format!("src/n{i}.ts")).collect();
    let mut modules: Vec<(&str, u64)> = vec![("src/app.ts", 9_000)];
    modules.extend(names.iter().map(|name| (name.as_str(), 900)));
    let edges: Vec<(&str, &str)> = names.iter().map(|name| ("src/app.ts", name.as_str())).collect();

    let found = findings(&modules, &edges);
    assert!(
        !found.iter().any(|f| f.kind == Kind::BarrelImport),
        "a large module is not a barrel: {found:?}"
    );
}

// --- restraint --------------------------------------------------------------

#[test]
fn nothing_below_the_threshold_is_reported() {
    // A list that always has something in it is a list nobody reads.
    let found = findings(
        &[("node_modules/lodash/a.js", 100), ("node_modules/x/node_modules/lodash/a.js", 90)],
        &[],
    );
    assert!(found.is_empty(), "under {WORTH_REPORTING} bytes is noise: {found:?}");
}

#[test]
fn a_clean_bundle_produces_no_findings() {
    // The common case, and it must be quiet.
    let found = findings(
        &[("src/main.ts", 5_000), ("src/util.ts", 3_000)],
        &[("src/main.ts", "src/util.ts")],
    );
    assert!(found.is_empty(), "a bundle of your own code has nothing to report: {found:?}");
}

#[test]
fn findings_are_ordered_by_what_is_at_stake() {
    // A list ordered by anything else asks the reader to do the ranking.
    let found = findings(
        &[
            ("node_modules/buffer/index.js", 2_000),
            ("node_modules/core-js/modules/a.js", 40_000),
            ("node_modules/lodash/i.js", 9_000),
            ("node_modules/z/node_modules/lodash/i.js", 8_000),
        ],
        &[],
    );

    let sizes: Vec<u64> = found.iter().map(|f| f.saves_at_most).collect();
    let mut sorted = sizes.clone();
    sorted.sort_by(|a, b| b.cmp(a));
    assert_eq!(sizes, sorted, "largest first");
}

#[test]
fn every_rule_states_what_acting_on_it_costs() {
    // All of them trade something, and a finding showing only the saving is
    // one that will be regretted.
    for kind in [
        Kind::DuplicateDependency,
        Kind::WholeLibraryImport,
        Kind::BarrelImport,
        Kind::NodePolyfill,
        Kind::Polyfill,
    ] {
        assert!(!kind.cost().is_empty(), "{kind:?} claims to cost nothing");
        assert!(kind.rule().starts_with("web-"), "{}", kind.rule());
    }
}

#[test]
fn rule_names_are_unique_so_provenance_identifies_the_rule() {
    let rules: Vec<&str> = [
        Kind::DuplicateDependency,
        Kind::WholeLibraryImport,
        Kind::BarrelImport,
        Kind::NodePolyfill,
        Kind::Polyfill,
    ]
    .iter()
    .map(|kind| kind.rule())
    .collect();

    let unique: std::collections::BTreeSet<&&str> = rules.iter().collect();
    assert_eq!(unique.len(), rules.len(), "two rules share a name: {rules:?}");
}

#[test]
fn the_real_corpus_bundle_reports_nothing_because_it_has_nothing_wrong() {
    // corpus/web bundles no dependencies at all. A rule firing here would be
    // a false positive on the cleanest possible input.
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/web/dist/meta.json");
    let Ok(raw) = std::fs::read(path) else { return };
    let attribution = attribute(&raw).expect("a real metafile");

    assert!(
        analyse(&attribution).is_empty(),
        "no dependencies means nothing to report: {:?}",
        analyse(&attribution)
    );
}
