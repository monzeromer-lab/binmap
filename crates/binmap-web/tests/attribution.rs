//! Source-map attribution, against output from a real bundler.
//!
//! `corpus/web` is built by esbuild with `--minify --sourcemap --splitting`, so
//! these run against the shape that actually ships: one long minified line,
//! several sources, and a chunk reachable only through a dynamic import. The
//! synthetic cases below cover the traps `§2.3` names, which a real bundle
//! happens not to contain.

use binmap_web::sourcemap::attribute;
use binmap_web::tier::Tier;
use serde_json::json;
use std::path::PathBuf;

fn corpus() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus/web/dist")
}

/// The corpus is built by `npm run build:split`. Skip rather than fail when it
/// has not been: a contributor without node should not see a red suite for a
/// backend they are not touching.
fn built_asset(name: &str) -> Option<(Vec<u8>, Vec<u8>)> {
    let asset = std::fs::read(corpus().join(name)).ok()?;
    let map = std::fs::read(corpus().join(format!("{name}.map"))).ok()?;
    Some((asset, map))
}

#[test]
fn a_real_minified_bundle_is_attributed_to_its_typescript_sources() {
    let Some((asset, map)) = built_asset("index.js") else {
        eprintln!("corpus/web is not built; run `npm run build:split` there");
        return;
    };

    let attribution = attribute(&asset, &map).expect("esbuild emits a readable map");
    assert_eq!(attribution.tier, Tier::SourceMaps);
    assert_eq!(attribution.total_bytes, asset.len() as u64);

    // Every source the map names is a file we wrote.
    assert!(!attribution.sources.is_empty(), "something should be attributed");
    for source in &attribution.sources {
        assert!(
            source.source.ends_with(".ts"),
            "attributed to something that is not a source: {}",
            source.source
        );
        assert!(source.segments > 0, "{} claims bytes with no segments", source.source);
    }

    // Largest first, which the whole view depends on.
    let bytes: Vec<u64> = attribution.sources.iter().map(|source| source.bytes).collect();
    let mut sorted = bytes.clone();
    sorted.sort_by(|a, b| b.cmp(a));
    assert_eq!(bytes, sorted, "sources must be ordered largest first");
}

#[test]
fn nothing_is_attributed_twice_and_nothing_exceeds_the_asset() {
    // The arithmetic that makes the whole view trustworthy. Overlapping spans
    // would inflate every number without ever looking wrong.
    let Some((asset, map)) = built_asset("index.js") else { return };
    let attribution = attribute(&asset, &map).unwrap();

    let attributed: u64 = attribution.sources.iter().map(|source| source.bytes).sum();
    assert_eq!(
        attributed + attribution.unattributed_bytes,
        attribution.total_bytes,
        "every byte is either attributed exactly once or counted as unattributed"
    );
    assert!(attributed <= attribution.total_bytes);
}

#[test]
fn the_bundlers_own_glue_is_reported_rather_than_blamed_on_a_source() {
    // Module wrappers and the runtime are generated, not authored. Spreading
    // them over whichever source sits nearby would be invisible and wrong.
    let Some((asset, map)) = built_asset("index.js") else { return };
    let attribution = attribute(&asset, &map).unwrap();

    assert!(attribution.coverage() > 0.0 && attribution.coverage() <= 1.0);
    assert!(
        attribution.describe().contains("bundler glue"),
        "the reader is told what the gap is: {}",
        attribution.describe()
    );
}

#[test]
fn esbuild_embeds_its_sources_and_we_say_so() {
    // §2.3: a finding based on source read from disk rather than from the map
    // is weaker, because the file on disk may not be the file that was built.
    let Some((asset, map)) = built_asset("index.js") else { return };
    let attribution = attribute(&asset, &map).unwrap();

    assert!(attribution.has_embedded_sources, "this build was made with sourcesContent");
    assert!(attribution.describe().contains("embedded in the map"));
}

#[test]
fn the_lazy_chunk_is_attributed_independently_of_the_entry() {
    // Code splitting is the whole reason initial-load bytes differ from total
    // bytes, so each chunk has to be attributable on its own.
    let Some(entry) = built_asset("index.js") else { return };
    let lazy = std::fs::read_dir(corpus())
        .expect("dist exists")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .find(|name| name.starts_with("analysis-") && name.ends_with(".js"));
    let Some(lazy) = lazy.and_then(|name| built_asset(&name)) else {
        panic!("the split build should produce an analysis chunk");
    };

    let entry = attribute(&entry.0, &entry.1).unwrap();
    let lazy = attribute(&lazy.0, &lazy.1).unwrap();

    assert!(
        entry.sources.iter().any(|s| s.source.contains("index.ts")),
        "the entry chunk holds the entry point"
    );
    assert!(
        lazy.sources.iter().any(|s| s.source.contains("analysis.ts")),
        "the lazy chunk holds the dynamically imported module"
    );
    assert!(
        !entry.sources.iter().any(|s| s.source.contains("analysis.ts")),
        "the lazily imported module must not be counted against initial load"
    );
}

// --- the traps a real bundle happens not to contain (§2.3) ------------------

/// A minimal but valid map: one generated line, two segments, two sources.
fn a_small_map() -> Vec<u8> {
    // "AAAA,SAACA" — segment at column 0 → source 0, then a later column →
    // source 1. Built by hand so the spans are known exactly.
    json!({
        "version": 3,
        "file": "out.js",
        "sources": ["a.ts", "b.ts"],
        "names": [],
        // "AAAA"  → generated col 0, source 0, line 0, col 0
        // "KCAA"   → col +5, source +1, line +0, col +0
        // The first version of this wrote "KACA", which leaves the source
        // index at 0 — so b.ts never appeared and the test failed on its own
        // data rather than on the code.
        "mappings": "AAAA,KCAA"
    })
    .to_string()
    .into_bytes()
}

#[test]
fn a_segment_runs_to_the_next_one_not_to_the_end_of_its_line() {
    // Getting this wrong silently loses the tail of every line — and in a
    // minified bundle one line can be the entire file.
    let generated = b"aaaaabbbbb".to_vec();
    let attribution = attribute(&generated, &a_small_map()).expect("a valid map");

    assert_eq!(attribution.total_bytes, 10);
    let a = attribution.sources.iter().find(|s| s.source == "a.ts").expect("a.ts");
    let b = attribution.sources.iter().find(|s| s.source == "b.ts").expect("b.ts");
    assert_eq!(a.bytes, 5, "the first segment runs to the second, not to the line end");
    assert_eq!(b.bytes, 5, "the last segment runs to the end of the asset");
    assert_eq!(attribution.unattributed_bytes, 0);
}

#[test]
fn bytes_before_the_first_segment_belong_to_nobody() {
    // A bundler prelude sits before anything authored. Attributing it to the
    // first source would be a quiet lie that scales with the runtime's size.
    let map = json!({
        "version": 3,
        "file": "out.js",
        "sources": ["a.ts"],
        "names": [],
        // one segment, at column 4
        "mappings": "IAAA"
    })
    .to_string()
    .into_bytes();

    let attribution = attribute(b"glueXXXX", &map).expect("a valid map");
    assert_eq!(attribution.unattributed_bytes, 4, "the prelude is not anyone's");
    assert_eq!(attribution.sources[0].bytes, 4);
}

#[test]
fn a_map_that_is_not_a_map_says_so_rather_than_attributing_nothing() {
    // §2.3 lists index maps as a case consumers silently mishandle. An empty
    // attribution would look like a bundle with no sources.
    let error = attribute(b"whatever", b"{\"this\": \"is not a source map\"}")
        .expect_err("a non-map is an error");
    assert!(error.to_string().contains("not a source map we can read"), "{error}");
    assert!(error.to_string().contains("Index maps"), "it names a known cause: {error}");
}

#[test]
fn a_map_with_no_mappings_attributes_everything_to_nobody() {
    // Valid, and means exactly one thing: nothing is traced. Reporting full
    // coverage of zero sources would be the dangerous reading.
    let map = json!({"version": 3, "file": "out.js", "sources": [], "names": [], "mappings": ""})
        .to_string()
        .into_bytes();

    let attribution = attribute(b"some output here", &map).expect("an empty map is still a map");
    assert!(attribution.sources.is_empty());
    assert_eq!(attribution.unattributed_bytes, 16);
    assert_eq!(attribution.coverage(), 0.0);
}

#[test]
fn an_empty_asset_does_not_divide_by_zero() {
    let map = json!({"version": 3, "file": "out.js", "sources": [], "names": [], "mappings": ""})
        .to_string()
        .into_bytes();
    let attribution = attribute(b"", &map).expect("valid");
    assert_eq!(attribution.coverage(), 0.0);
    assert_eq!(attribution.total_bytes, 0);
}

// --- the tier, which is what stops these numbers over-claiming --------------

#[test]
fn source_map_attribution_never_claims_to_know_why_a_module_is_present() {
    // The question a size tool is really asked, and the one this tier cannot
    // answer at all.
    assert!(!Tier::SourceMaps.explains_inclusion());
    assert!(!Tier::SourceMaps.has_module_identity());
    assert!(Tier::SourceMaps.attributes_anything());
    assert!(Tier::EsbuildMetafile.explains_inclusion());
}

#[test]
fn the_tiers_are_ordered_so_the_best_available_can_be_taken() {
    assert!(Tier::EsbuildMetafile > Tier::WebpackStats);
    assert!(Tier::WebpackStats > Tier::BundleGraph);
    assert!(Tier::BundleGraph > Tier::SourceMaps);
    assert!(Tier::SourceMaps > Tier::AssetSizesOnly);
    assert!(!Tier::AssetSizesOnly.attributes_anything());
}

#[test]
fn every_tier_states_what_it_can_and_cannot_say() {
    for tier in [
        Tier::AssetSizesOnly,
        Tier::SourceMaps,
        Tier::BundleGraph,
        Tier::WebpackStats,
        Tier::EsbuildMetafile,
    ] {
        assert!(!tier.describe().is_empty(), "{tier:?} describes itself as nothing");
        assert!(!tier.label().is_empty());
    }
    // And the weakest tier must never carry a Certain finding.
    use binmap_core::finding::Confidence;
    assert_eq!(Tier::AssetSizesOnly.ceiling(), Confidence::Probable);
    assert_eq!(Tier::SourceMaps.ceiling(), Confidence::High);
    assert_eq!(Tier::EsbuildMetafile.ceiling(), Confidence::Certain);
}

#[test]
fn a_fallback_says_what_it_could_not_use() {
    // §2.3: when a map is missing, say so rather than degrading quietly.
    let detection = binmap_web::tier::Detection::new(Tier::SourceMaps)
        .rejecting(Tier::EsbuildMetafile, "no metafile was found beside the bundle");

    let described = detection.describe();
    assert!(described.contains("source maps"), "{described}");
    assert!(described.contains("no metafile"), "{described}");
}

#[test]
fn an_unmapped_trailing_line_belongs_to_nobody() {
    // The `//# sourceMappingURL=` comment sits on its own line that no segment
    // covers. Letting the last segment run to the end of the file attributed
    // it — 9% of the corpus bundle — to whichever source happened to be last,
    // silently and to a real file.
    let generated = b"code\n//# sourceMappingURL=x.map\n".to_vec();
    let map = json!({
        "version": 3,
        "file": "out.js",
        "sources": ["a.ts"],
        "names": [],
        "mappings": "AAAA"
    })
    .to_string()
    .into_bytes();

    let attribution = attribute(&generated, &map).expect("valid");
    assert_eq!(attribution.sources[0].bytes, 5, "the mapped line, including its newline");
    assert_eq!(
        attribution.unattributed_bytes,
        generated.len() as u64 - 5,
        "everything after the mapped line is nobody's"
    );
    assert!(attribution.coverage() < 1.0);
}

#[test]
fn a_real_bundles_coverage_excludes_its_source_mapping_comment() {
    let Some((asset, map)) = built_asset("index.js") else { return };
    let attribution = attribute(&asset, &map).unwrap();

    assert!(
        attribution.unattributed_bytes > 0,
        "the sourceMappingURL comment is not authored code, so it is not attributed"
    );
    assert!(
        attribution.coverage() < 1.0,
        "100% coverage on a bundle with a trailing comment means the comment was absorbed"
    );
    // But most of it is still traced: this is a fallback, not a failure.
    assert!(attribution.coverage() > 0.8, "coverage was {:.3}", attribution.coverage());
}
