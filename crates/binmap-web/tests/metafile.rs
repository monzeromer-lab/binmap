//! esbuild metafile attribution, against a real metafile (`§3.1`, `§4.2`).

use binmap_web::metafile::{Load, attribute, package_of};
use binmap_web::tier::Tier;
use serde_json::json;
use std::path::PathBuf;

fn real_metafile() -> Option<Vec<u8>> {
    std::fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus/web/dist/meta.json"))
        .ok()
}

#[test]
fn a_real_split_build_separates_initial_load_from_lazy() {
    // The whole point of §4.2. corpus/web imports `analysis.ts` dynamically,
    // so its chunk must not be counted against what the user waits for.
    let Some(metafile) = real_metafile() else {
        eprintln!("corpus/web is not built; run `npm run build:split` there");
        return;
    };
    let attribution = attribute(&metafile).expect("esbuild emits a readable metafile");

    assert_eq!(attribution.tier, Tier::EsbuildMetafile);
    assert!(attribution.initial_load_bytes() > 0);
    assert!(
        attribution.lazy_bytes() > 0,
        "the split build has a dynamically imported chunk: {:?}",
        attribution.chunks
    );
    assert!(
        attribution.initial_load_bytes() < attribution.total_bytes(),
        "the headline must be smaller than everything, or splitting achieved nothing"
    );

    let lazy: Vec<&str> = attribution
        .chunks
        .iter()
        .filter(|chunk| chunk.load == Load::Lazy)
        .map(|chunk| chunk.path.as_str())
        .collect();
    assert!(
        lazy.iter().any(|path| path.contains("analysis")),
        "the dynamically imported module's chunk is lazy, got {lazy:?}"
    );
}

#[test]
fn source_maps_are_not_counted_as_page_weight() {
    // The trap the format sets. A browser fetches a map only when devtools are
    // open, and a map is often larger than the code it describes, so counting
    // them inflates every number by a lot.
    let Some(metafile) = real_metafile() else { return };
    let attribution = attribute(&metafile).unwrap();

    assert!(
        !attribution.chunks.iter().any(|chunk| chunk.path.ends_with(".map")),
        "a .map output was counted as a chunk: {:?}",
        attribution.chunks.iter().map(|c| &c.path).collect::<Vec<_>>()
    );
}

#[test]
fn the_entry_chunk_names_its_entry_point() {
    let Some(metafile) = real_metafile() else { return };
    let attribution = attribute(&metafile).unwrap();

    let entry = attribution
        .chunks
        .iter()
        .find(|chunk| chunk.load == Load::Entry)
        .expect("a build has an entry chunk");
    assert_eq!(entry.entry_point.as_deref(), Some("src/index.ts"));
}

#[test]
fn modules_are_attributed_at_their_size_after_bundling_not_on_disk() {
    // `bytesInOutput` is what the module costs after minification. Reporting
    // its size on disk would overstate every number, usually several-fold.
    let Some(metafile) = real_metafile() else { return };
    let attribution = attribute(&metafile).unwrap();

    let modules = attribution.modules();
    assert!(!modules.is_empty());
    for module in &modules {
        assert!(module.bytes > 0, "{} claims no bytes", module.module);
    }

    // Largest first.
    let bytes: Vec<u64> = modules.iter().map(|m| m.bytes).collect();
    let mut sorted = bytes.clone();
    sorted.sort_by(|a, b| b.cmp(a));
    assert_eq!(bytes, sorted);

    // And every module's total is within its chunks' total.
    assert!(modules.iter().map(|m| m.bytes).sum::<u64>() <= attribution.total_bytes());
}

#[test]
fn our_own_code_is_told_apart_from_a_dependency() {
    let Some(metafile) = real_metafile() else { return };
    let attribution = attribute(&metafile).unwrap();

    let packages = attribution.packages();
    assert!(!packages.is_empty());
    // corpus/web bundles nothing from node_modules, so it is all ours.
    assert!(packages.iter().any(|(name, _)| name == "your code"), "got {packages:?}");
}

#[test]
fn the_critical_path_stops_at_a_dynamic_import() {
    // Synthetic, because a real build has only one lazy edge and the
    // transitive case is where this goes wrong: a chunk reachable *through* a
    // lazy chunk is also lazy.
    let metafile = json!({
        "inputs": {},
        "outputs": {
            "out/entry.js": {
                "bytes": 100,
                "entryPoint": "src/main.ts",
                "inputs": {},
                "imports": [
                    {"path": "out/shared.js", "kind": "import-statement"},
                    {"path": "out/lazy.js", "kind": "dynamic-import"}
                ]
            },
            "out/shared.js": {"bytes": 50, "inputs": {}, "imports": []},
            "out/lazy.js": {
                "bytes": 200,
                "inputs": {},
                "imports": [{"path": "out/deep.js", "kind": "import-statement"}]
            },
            "out/deep.js": {"bytes": 400, "inputs": {}, "imports": []}
        }
    })
    .to_string();

    let attribution = attribute(metafile.as_bytes()).unwrap();
    let load = |path: &str| {
        attribution.chunks.iter().find(|c| c.path == path).map(|c| c.load).expect(path)
    };

    assert_eq!(load("out/entry.js"), Load::Entry);
    assert_eq!(load("out/shared.js"), Load::Initial, "a static import is waited for");
    assert_eq!(load("out/lazy.js"), Load::Lazy);
    assert_eq!(
        load("out/deep.js"),
        Load::Lazy,
        "a chunk reachable only through a lazy chunk is also lazy"
    );

    assert_eq!(attribution.initial_load_bytes(), 150, "entry plus shared");
    assert_eq!(attribution.lazy_bytes(), 600);
    assert_eq!(attribution.total_bytes(), 750);
}

#[test]
fn a_cycle_in_the_chunk_graph_terminates() {
    // Two chunks importing each other is legal and would hang a naive walk.
    let metafile = json!({
        "inputs": {},
        "outputs": {
            "out/a.js": {
                "bytes": 10,
                "entryPoint": "src/a.ts",
                "inputs": {},
                "imports": [{"path": "out/b.js", "kind": "import-statement"}]
            },
            "out/b.js": {
                "bytes": 20,
                "inputs": {},
                "imports": [{"path": "out/a.js", "kind": "import-statement"}]
            }
        }
    })
    .to_string();

    let attribution = attribute(metafile.as_bytes()).expect("a cycle is not an error");
    assert_eq!(attribution.initial_load_bytes(), 30);
}

#[test]
fn a_module_duplicated_across_chunks_is_counted_in_each() {
    // Duplication across chunks is real bytes the user downloads twice, and
    // summing it away would hide one of the things a reader most wants to
    // find.
    let metafile = json!({
        "inputs": {},
        "outputs": {
            "out/a.js": {
                "bytes": 100,
                "entryPoint": "src/a.ts",
                "inputs": {"src/shared.ts": {"bytesInOutput": 40}},
                "imports": []
            },
            "out/b.js": {
                "bytes": 100,
                "entryPoint": "src/b.ts",
                "inputs": {"src/shared.ts": {"bytesInOutput": 40}},
                "imports": []
            }
        }
    })
    .to_string();

    let attribution = attribute(metafile.as_bytes()).unwrap();
    let shared = attribution
        .modules()
        .into_iter()
        .find(|module| module.module == "src/shared.ts")
        .expect("the shared module");
    assert_eq!(shared.bytes, 80, "it really is in the bundle twice");
}

#[test]
fn a_build_with_no_outputs_says_so_rather_than_reporting_zero() {
    // "0 bytes" reads as a wonderfully optimised bundle.
    let error = attribute(json!({"inputs": {}, "outputs": {}}).to_string().as_bytes())
        .expect_err("an empty build is an error");
    assert!(error.to_string().contains("no outputs"), "{error}");
    assert!(error.to_string().contains("entry point"), "it names a likely cause: {error}");
}

#[test]
fn something_that_is_not_a_metafile_says_so() {
    let error = attribute(b"<html>not json</html>").expect_err("not a metafile");
    assert!(error.to_string().contains("not an esbuild metafile"), "{error}");
    assert!(error.to_string().contains("--metafile"), "it says how to make one: {error}");
}

// --- package identity from a path ------------------------------------------

#[test]
fn a_scoped_package_keeps_its_scope() {
    // The case a naive split gets wrong: `@scope/name`, not `@scope`.
    assert_eq!(
        package_of("node_modules/@tanstack/query-core/build/index.js").as_deref(),
        Some("@tanstack/query-core")
    );
}

#[test]
fn an_ordinary_package_is_its_first_path_segment() {
    assert_eq!(package_of("node_modules/lodash/lodash.js").as_deref(), Some("lodash"));
}

#[test]
fn a_nested_dependency_is_attributed_to_the_innermost_package() {
    // npm hoists, but nested node_modules still happen, and the bytes belong
    // to the package they came from rather than the one that pulled it in.
    assert_eq!(package_of("node_modules/a/node_modules/b/index.js").as_deref(), Some("b"));
}

#[test]
fn our_own_source_belongs_to_no_package() {
    for ours in ["src/index.ts", "app/routes/home.tsx", "index.js"] {
        assert_eq!(package_of(ours), None, "{ours} is not a dependency");
    }
}

#[test]
fn a_code_split_chunk_with_its_own_entry_point_is_still_lazy() {
    // The real-data finding that a synthetic fixture missed: under
    // `--splitting` esbuild gives a dynamic import target its own
    // `entryPoint`, so that field alone says nothing about whether the user
    // waits for the chunk. What decides it is whether anything reaches it
    // through a dynamic edge.
    let metafile = json!({
        "inputs": {},
        "outputs": {
            "out/main.js": {
                "bytes": 100,
                "entryPoint": "src/main.ts",
                "inputs": {},
                "imports": [{"path": "out/route.js", "kind": "dynamic-import"}]
            },
            // Marked as an entry point by esbuild, and nonetheless lazy.
            "out/route.js": {
                "bytes": 900,
                "entryPoint": "src/route.ts",
                "inputs": {},
                "imports": []
            }
        }
    })
    .to_string();

    let attribution = attribute(metafile.as_bytes()).unwrap();
    let route = attribution.chunks.iter().find(|c| c.path == "out/route.js").expect("route");
    assert_eq!(
        route.load,
        Load::Lazy,
        "an `entryPoint` on a dynamically imported chunk must not put it on the critical path"
    );
    assert_eq!(attribution.initial_load_bytes(), 100, "the user waits for main.js alone");
    assert_eq!(attribution.lazy_bytes(), 900);
}

#[test]
fn several_real_entry_points_are_all_on_the_critical_path() {
    // A multi-page build has several genuine entries, and every one of them is
    // something a user waits for on some page.
    let metafile = json!({
        "inputs": {},
        "outputs": {
            "out/home.js": {"bytes": 100, "entryPoint": "src/home.ts", "inputs": {}, "imports": []},
            "out/about.js": {"bytes": 200, "entryPoint": "src/about.ts", "inputs": {}, "imports": []}
        }
    })
    .to_string();

    let attribution = attribute(metafile.as_bytes()).unwrap();
    assert_eq!(attribution.initial_load_bytes(), 300);
    assert_eq!(attribution.lazy_bytes(), 0);
    assert!(attribution.chunks.iter().all(|chunk| chunk.load == Load::Entry));
}

// --- why a module is in the bundle (`§3`) ----------------------------------

#[test]
fn a_real_build_can_say_what_pulls_each_module_in() {
    // The question a size tool is really asked, and the one source maps cannot
    // answer at all. It is why this tier is worth preferring.
    let Some(metafile) = real_metafile() else { return };
    let attribution = attribute(&metafile).unwrap();

    // format.ts is imported by index.ts, which is the entry.
    let chain = attribution
        .why_is_this_here("src/format.ts")
        .expect("format.ts is reachable from the entry");
    assert!(!chain.is_empty());
    assert_eq!(chain.last().unwrap().to, "src/format.ts");
    assert_eq!(chain[0].from, "src/index.ts", "the chain starts at the entry point");

    let explained = attribution.explain("src/format.ts");
    assert!(explained.contains("src/index.ts"), "{explained}");
    assert!(explained.contains("src/format.ts"), "{explained}");
}

#[test]
fn an_entry_point_is_why_everything_else_is_here() {
    let Some(metafile) = real_metafile() else { return };
    let attribution = attribute(&metafile).unwrap();

    let chain = attribution.why_is_this_here("src/index.ts").expect("the entry point");
    assert!(chain.is_empty(), "nothing pulled the entry in");
    assert!(attribution.explain("src/index.ts").contains("entry point"));
}

#[test]
fn a_dynamic_import_is_named_as_the_kind_of_edge_it_is() {
    // Knowing *how* a module is reached is most of the value: a dynamic edge
    // is a deliberate split, a static one is a cost you did not choose.
    let Some(metafile) = real_metafile() else { return };
    let attribution = attribute(&metafile).unwrap();

    let chain = attribution.why_is_this_here("src/analysis.ts").expect("reachable");
    assert!(
        chain.iter().any(|edge| edge.kind == "dynamic-import"),
        "analysis.ts is reached through `await import`, got {chain:?}"
    );
}

#[test]
fn the_shortest_chain_is_the_one_reported() {
    // A module reachable two ways should be explained by the short route, not
    // an arbitrary one.
    let metafile = json!({
        "inputs": {
            "src/entry.ts": {
                "bytes": 10,
                "imports": [
                    {"path": "src/target.ts", "kind": "import-statement"},
                    {"path": "src/middle.ts", "kind": "import-statement"}
                ]
            },
            "src/middle.ts": {
                "bytes": 10,
                "imports": [{"path": "src/target.ts", "kind": "import-statement"}]
            },
            "src/target.ts": {"bytes": 10, "imports": []}
        },
        "outputs": {
            "out/entry.js": {
                "bytes": 30, "entryPoint": "src/entry.ts", "inputs": {}, "imports": []
            }
        }
    })
    .to_string();

    let attribution = attribute(metafile.as_bytes()).unwrap();
    let chain = attribution.why_is_this_here("src/target.ts").expect("reachable");
    assert_eq!(chain.len(), 1, "the direct import is shorter: {chain:?}");
}

#[test]
fn an_import_cycle_does_not_hang_the_explanation() {
    let metafile = json!({
        "inputs": {
            "src/a.ts": {"bytes": 10, "imports": [{"path": "src/b.ts", "kind": "import-statement"}]},
            "src/b.ts": {"bytes": 10, "imports": [{"path": "src/a.ts", "kind": "import-statement"}]}
        },
        "outputs": {
            "out/a.js": {"bytes": 20, "entryPoint": "src/a.ts", "inputs": {}, "imports": []}
        }
    })
    .to_string();

    let attribution = attribute(metafile.as_bytes()).unwrap();
    assert!(attribution.why_is_this_here("src/b.ts").is_some());
    // And an unreachable module says so rather than looping.
    assert!(attribution.why_is_this_here("src/nowhere.ts").is_none());
}

#[test]
fn a_module_nothing_imports_is_reported_as_a_contradiction_not_hidden() {
    let metafile = json!({
        "inputs": {"src/orphan.ts": {"bytes": 10, "imports": []}},
        "outputs": {
            "out/main.js": {"bytes": 5, "entryPoint": "src/main.ts", "inputs": {}, "imports": []}
        }
    })
    .to_string();

    let attribution = attribute(metafile.as_bytes()).unwrap();
    let explained = attribution.explain("src/orphan.ts");
    assert!(explained.contains("disagreeing with itself"), "{explained}");
}

#[test]
fn how_much_a_module_shrank_on_the_way_in_is_available() {
    // A module that barely shrank is usually one tree shaking could not touch,
    // which is a different problem from one that is simply large.
    let Some(metafile) = real_metafile() else { return };
    let attribution = attribute(&metafile).unwrap();

    let (before, after) = attribution.shrinkage("src/format.ts").expect("recorded");
    assert!(before > 0, "the metafile records the source size");
    assert!(after > 0, "and the bundled size");
    assert!(after < before, "minification should have removed something: {after} of {before}");

    assert!(attribution.shrinkage("src/never-existed.ts").is_none());
}
