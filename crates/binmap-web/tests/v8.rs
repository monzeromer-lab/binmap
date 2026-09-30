//! V8 CPU profiles (`F3.2` for web targets).
//!
//! The profile in `corpus/web/profiles` is real — `node --cpu-prof` running
//! the built bundle — so the shapes being handled are the ones node actually
//! emits rather than the ones the documentation describes.

use binmap_web::v8::{map_through, read};
use serde_json::json;
use std::path::PathBuf;

fn real_profile() -> Option<Vec<u8>> {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus/web/profiles");
    let entry = std::fs::read_dir(directory)
        .ok()?
        .filter_map(|entry| entry.ok())
        .find(|entry| entry.file_name().to_string_lossy().ends_with(".cpuprofile"))?;
    std::fs::read(entry.path()).ok()
}

#[test]
fn a_real_node_profile_is_read() {
    let Some(profile) = real_profile() else {
        eprintln!("no .cpuprofile in corpus/web/profiles; run `node --cpu-prof` there");
        return;
    };
    let read = read(&profile).expect("node emits a readable profile");

    assert!(read.total_samples > 0, "a real profile has samples");
    assert!(!read.hot.is_empty());
    assert!(read.describe().contains("samples"), "{}", read.describe());
}

#[test]
fn v8s_own_accounting_is_counted_separately_from_functions() {
    // `(program)` at the top of a profile of someone's code is true and
    // useless — it is V8 telling you it was busy.
    let Some(profile) = real_profile() else { return };
    let read = read(&profile).unwrap();

    for hot in &read.hot {
        assert!(!hot.function.starts_with("(program)"), "{}", hot.function);
        assert!(!hot.function.starts_with("(root)"), "{}", hot.function);
        assert!(!hot.function.starts_with("(garbage"), "{}", hot.function);
    }
    // And it is reported rather than hidden, or the percentages would not add
    // up and nothing would say why.
    assert!(read.describe().contains("V8's own accounting"));
}

#[test]
fn your_own_code_can_be_told_from_nodes() {
    // "Your-code-only on by default", because a profile of a node program is
    // mostly node.
    let Some(profile) = real_profile() else { return };
    let read = read(&profile).unwrap();

    let yours = read.yours();
    for hot in &yours {
        let location = hot.original.as_deref().or(hot.generated.as_deref()).unwrap_or("");
        assert!(!location.starts_with("node:"), "{location}");
        assert!(!location.contains("node_modules/"), "{location}");
    }
    assert!(yours.len() < read.hot.len(), "a node profile has node frames in it");
}

// --- the traps, on shapes a real profile contains --------------------------

#[test]
fn lines_are_converted_from_v8s_zero_base_to_the_one_a_developer_reads() {
    // Off by one in the direction that looks entirely plausible.
    let profile = json!({
        "nodes": [
            {"id": 1, "callFrame": {"functionName": "(root)", "url": "", "lineNumber": -1,
                                    "columnNumber": -1}, "hitCount": 0, "children": [2]},
            {"id": 2, "callFrame": {"functionName": "work", "url": "file:///app.js",
                                    "lineNumber": 0, "columnNumber": 4}, "hitCount": 10}
        ],
        "samples": [2, 2, 2]
    })
    .to_string();

    let read = read(profile.as_bytes()).unwrap();
    let work = read.hot.iter().find(|h| h.function == "work").expect("work");
    assert_eq!(work.line, Some(1), "V8's line 0 is a developer's line 1");
}

#[test]
fn a_synthetic_frames_negative_line_becomes_no_line_rather_than_line_zero() {
    let profile = json!({
        "nodes": [{"id": 1, "callFrame": {"functionName": "f", "url": "file:///a.js",
                                          "lineNumber": -1, "columnNumber": -1},
                   "hitCount": 1}],
        "samples": [1]
    })
    .to_string();
    let read = read(profile.as_bytes()).unwrap();
    assert_eq!(read.hot[0].line, None);
}

#[test]
fn total_time_sums_the_subtree_and_self_time_does_not() {
    // A tree where most nodes have no hits is normal; ranking by hit count
    // without summing children gives a list of leaves and no structure.
    let profile = json!({
        "nodes": [
            {"id": 1, "callFrame": {"functionName": "outer", "url": "file:///a.js",
                                    "lineNumber": 0, "columnNumber": 0},
             "hitCount": 0, "children": [2]},
            {"id": 2, "callFrame": {"functionName": "inner", "url": "file:///a.js",
                                    "lineNumber": 5, "columnNumber": 0},
             "hitCount": 0, "children": []}
        ],
        "samples": [2, 2, 2, 2]
    })
    .to_string();

    let read = read(profile.as_bytes()).unwrap();
    let outer = read.hot.iter().find(|h| h.function == "outer").expect("outer");
    let inner = read.hot.iter().find(|h| h.function == "inner").expect("inner");

    assert_eq!(outer.self_samples, 0, "nothing stopped in outer");
    assert_eq!(outer.total_samples, 4, "but everything went through it");
    assert_eq!(inner.self_samples, 4);
}

#[test]
fn one_function_appearing_at_several_nodes_is_ranked_once() {
    // A function appears once per call path, and ranking nodes ranks call
    // paths rather than functions.
    let profile = json!({
        "nodes": [
            {"id": 1, "callFrame": {"functionName": "a", "url": "file:///x.js",
                                    "lineNumber": 0, "columnNumber": 0},
             "hitCount": 0, "children": [2, 3]},
            {"id": 2, "callFrame": {"functionName": "shared", "url": "file:///x.js",
                                    "lineNumber": 9, "columnNumber": 0}, "hitCount": 0},
            {"id": 3, "callFrame": {"functionName": "shared", "url": "file:///x.js",
                                    "lineNumber": 9, "columnNumber": 0}, "hitCount": 0}
        ],
        "samples": [2, 2, 3]
    })
    .to_string();

    let read = read(profile.as_bytes()).unwrap();
    let shared: Vec<_> = read.hot.iter().filter(|h| h.function == "shared").collect();
    assert_eq!(shared.len(), 1, "one entry, not two: {:?}", read.hot);
    assert_eq!(shared[0].self_samples, 3, "both call paths summed");
}

#[test]
fn a_cycle_in_the_tree_does_not_hang() {
    // A malformed profile can point a child at an ancestor.
    let profile = json!({
        "nodes": [
            {"id": 1, "callFrame": {"functionName": "a", "url": "file:///x.js",
                                    "lineNumber": 0, "columnNumber": 0},
             "hitCount": 1, "children": [2]},
            {"id": 2, "callFrame": {"functionName": "b", "url": "file:///x.js",
                                    "lineNumber": 1, "columnNumber": 0},
             "hitCount": 1, "children": [1]}
        ],
        "samples": [1, 2]
    })
    .to_string();
    assert!(read(profile.as_bytes()).is_ok());
}

#[test]
fn hit_counts_are_used_when_a_profile_carries_no_sample_list() {
    // Both forms exist, and `samples` is the authority when present because it
    // is the actual sequence.
    let profile = json!({
        "nodes": [{"id": 1, "callFrame": {"functionName": "f", "url": "file:///a.js",
                                          "lineNumber": 0, "columnNumber": 0},
                   "hitCount": 7}]
    })
    .to_string();
    let read = read(profile.as_bytes()).unwrap();
    assert_eq!(read.total_samples, 7);
    assert_eq!(read.hot[0].self_samples, 7);
}

#[test]
fn something_that_is_not_a_profile_says_how_to_make_one() {
    let error = read(b"<html>not a profile</html>").expect_err("not a profile");
    assert!(error.to_string().contains("not a V8 CPU profile"), "{error}");
    assert!(error.to_string().contains("--cpu-prof"), "{error}");
}

#[test]
fn an_empty_profile_says_so_rather_than_reporting_nothing_wrong() {
    let error =
        read(json!({"nodes": [], "samples": []}).to_string().as_bytes()).expect_err("no nodes");
    assert!(error.to_string().contains("no nodes"), "{error}");
}

// --- mapping back to TypeScript --------------------------------------------

#[test]
fn frames_are_mapped_back_through_a_source_map() {
    // The point of the whole exercise: a profile of a minified bundle names
    // `o` and `u`, and a reader needs `format.ts`.
    let profile = json!({
        "nodes": [{"id": 1, "callFrame": {"functionName": "o", "url": "file:///dist/index.js",
                                          "lineNumber": 0, "columnNumber": 12},
                   "hitCount": 5}],
        "samples": [1]
    })
    .to_string();

    let mut read = read(profile.as_bytes()).unwrap();
    map_through(&mut read, |url, line, _column| {
        assert_eq!(url, "file:///dist/index.js");
        assert_eq!(line, 0, "the lookup speaks V8's zero-based numbering");
        Some(("src/format.ts".to_string(), 3))
    });

    assert_eq!(read.hot[0].original.as_deref(), Some("src/format.ts"));
    assert_eq!(read.hot[0].line, Some(3));
    assert!(read.hot[0].is_yours(), "a mapped source file is the reader's code");
}

#[test]
fn a_frame_the_map_does_not_cover_keeps_its_generated_location() {
    // node's own internals have no source map, and losing their location
    // would make them unidentifiable rather than merely unmapped.
    let profile = json!({
        "nodes": [{"id": 1, "callFrame": {"functionName": "readFile", "url": "node:fs",
                                          "lineNumber": 40, "columnNumber": 0},
                   "hitCount": 5}],
        "samples": [1]
    })
    .to_string();

    let mut read = read(profile.as_bytes()).unwrap();
    map_through(&mut read, |_, _, _| None);

    assert_eq!(read.hot[0].original, None);
    assert_eq!(read.hot[0].generated.as_deref(), Some("node:fs"));
    assert_eq!(read.hot[0].line, Some(41), "and its line is untouched");
    assert!(!read.hot[0].is_yours(), "node's internals are not your code");
}
