//! The web configuration sweep (`TOOLING-WEB §5`).
//!
//! The bundler is injected, so everything here runs without node installed.
//! What is being tested is the sweep's arithmetic and its judgement about what
//! counts as a candidate — the parts that decide whether the answer is right.

use binmap_web::configuration::{Target, WebConfiguration, WebMatrix};
use binmap_web::size::CompressionSettings;
use binmap_web::sweep::WebSweepPlan;
use std::path::Path;

/// A fake bundler that writes `contents` for every configuration.
fn writing(
    contents: &'static str,
) -> impl FnMut(&WebConfiguration, &[String], &Path) -> binmap_core::Result<()> {
    move |_, _, directory| {
        std::fs::write(directory.join("index.js"), contents).unwrap();
        Ok(())
    }
}

fn plan<'a>(root: &'a Path, matrix: WebMatrix) -> WebSweepPlan<'a> {
    WebSweepPlan { root, entry: "src/index.ts", matrix, settings: CompressionSettings::default() }
}

#[test]
fn every_configuration_in_the_matrix_is_built_and_measured() {
    let scratch = tempfile::tempdir().unwrap();
    let matrix = WebMatrix::targets_only();
    let expected = matrix.cardinality();

    let sweep = plan(scratch.path(), matrix).run(writing("console.log(1)")).expect("sweeps");

    assert_eq!(sweep.measured.len(), expected);
    assert!(sweep.failed.is_empty(), "{:?}", sweep.failed);
    assert!(sweep.baseline.is_some(), "the baseline is measured, not assumed");
}

#[test]
fn a_configuration_that_fails_to_build_is_kept_with_its_reason() {
    // `es2015` with syntax esbuild cannot down-level is a real answer about
    // that target. Dropping it silently produced "0 configurations built" and
    // nothing else, which is a sweep nobody can debug.
    let scratch = tempfile::tempdir().unwrap();

    let sweep = plan(scratch.path(), WebMatrix::targets_only())
        .run(|configuration, _, directory| {
            if configuration.target == Target::Es2015 {
                return Err(binmap_core::Error::Other("cannot down-level that syntax".into()));
            }
            std::fs::write(directory.join("index.js"), "x").unwrap();
            Ok(())
        })
        .expect("a failing configuration does not abort the sweep");

    assert_eq!(sweep.failed.len(), 1);
    assert_eq!(sweep.failed[0].0.target, Target::Es2015);
    assert!(sweep.failed[0].1.contains("down-level"), "{}", sweep.failed[0].1);
    assert!(sweep.summary().contains("did not build"), "{}", sweep.summary());
    assert!(!sweep.measured.is_empty(), "the rest still measured");
}

#[test]
fn a_build_that_emits_nothing_is_a_failure_rather_than_a_bundle_of_zero_bytes() {
    // "0 bytes" reads as a wonderfully optimised bundle and would win the
    // ranking outright.
    let scratch = tempfile::tempdir().unwrap();
    let sweep =
        plan(scratch.path(), WebMatrix::targets_only()).run(|_, _, _| Ok(())).expect("sweeps");

    assert!(sweep.measured.is_empty());
    assert_eq!(sweep.failed.len(), WebMatrix::targets_only().cardinality());
    assert!(sweep.failed[0].1.contains("built nothing"), "{}", sweep.failed[0].1);
}

#[test]
fn source_maps_and_the_metafile_are_not_counted_as_page_weight() {
    // Counting them would make every configuration look several times larger,
    // and a map is frequently larger than the code it describes.
    let scratch = tempfile::tempdir().unwrap();
    let sweep = plan(scratch.path(), WebMatrix::targets_only())
        .run(|_, _, directory| {
            std::fs::write(directory.join("index.js"), "abc").unwrap();
            std::fs::write(directory.join("index.js.map"), "x".repeat(10_000)).unwrap();
            std::fs::write(directory.join("meta.json"), "y".repeat(10_000)).unwrap();
            Ok(())
        })
        .expect("sweeps");

    let measured = &sweep.measured[0];
    assert_eq!(measured.size.raw, 3, "only index.js counts");
    assert_eq!(measured.assets, 1);
}

#[test]
fn a_control_never_wins_the_ranking() {
    // The matrix holds combinations that are measurements rather than
    // candidates. An unminified development build ranked alongside real
    // options would put a huge bundle at the bottom of a list of shippable
    // ones and make the list untrustworthy.
    let scratch = tempfile::tempdir().unwrap();
    let matrix = WebMatrix {
        targets: vec![Target::EsNext],
        minify: vec![true, false],
        tree_shaking: vec![true],
        splitting: vec![false],
        production: vec![true],
    };

    let sweep = plan(scratch.path(), matrix)
        .run(|configuration, _, directory| {
            // The unminified build is much smaller here, purely to prove the
            // ranking excludes it on principle rather than by luck.
            let contents = if configuration.minify { "x".repeat(500) } else { "y".to_string() };
            std::fs::write(directory.join("index.js"), contents).unwrap();
            Ok(())
        })
        .expect("sweeps");

    let smallest = sweep.smallest().expect("something shippable");
    assert!(smallest.configuration.minify, "a control must not win: {:?}", smallest.configuration);
    assert!(smallest.configuration.is_shippable());
}

#[test]
fn the_disagreement_between_raw_and_transfer_size_is_reported() {
    // §4.1's counterintuitive result, and the finding nobody else produces.
    // Reproduced deterministically: highly repetitive content compresses
    // almost for free, so a larger raw file can be the smaller transfer.
    let scratch = tempfile::tempdir().unwrap();
    let matrix = WebMatrix {
        targets: vec![Target::EsNext, Target::Es2020],
        minify: vec![true],
        tree_shaking: vec![true],
        splitting: vec![false],
        production: vec![true],
    };

    let sweep = plan(scratch.path(), matrix)
        .run(|configuration, _, directory| {
            let contents = if configuration.target == Target::EsNext {
                // The baseline: short, and incompressible.
                (0..400u32).map(|i| char::from(b'a' + (i * 7 % 26) as u8)).collect::<String>()
            } else {
                // Larger on disk, and compresses to almost nothing.
                "aaaaaaaaaa".repeat(200)
            };
            std::fs::write(directory.join("index.js"), contents).unwrap();
            Ok(())
        })
        .expect("sweeps");

    let disagreements = sweep.disagreements();
    assert!(!disagreements.is_empty(), "the repetitive build is larger raw and smaller compressed");
    assert!(
        disagreements.iter().any(|(_, ranking)| ranking.describe().contains("actually pays")),
        "{disagreements:?}"
    );
}

#[test]
fn the_user_s_own_output_directory_is_never_written_to() {
    // A sweep that wrote into `dist/` would destroy the build someone already
    // had, which is the web version of the rule that Cargo.toml is never
    // edited.
    let scratch = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(scratch.path().join("dist")).unwrap();
    std::fs::write(scratch.path().join("dist/existing.js"), "do not touch").unwrap();

    plan(scratch.path(), WebMatrix::targets_only()).run(writing("console.log(1)")).expect("sweeps");

    assert_eq!(
        std::fs::read_to_string(scratch.path().join("dist/existing.js")).unwrap(),
        "do not touch"
    );
    assert!(
        scratch.path().join("node_modules/.binmap").is_dir(),
        "our builds go somewhere of our own"
    );
}

#[test]
fn a_stale_directory_from_an_earlier_run_is_not_measured_as_this_one() {
    let scratch = tempfile::tempdir().unwrap();
    let matrix = WebMatrix::targets_only();

    plan(scratch.path(), matrix.clone())
        .run(|_, _, directory| {
            std::fs::write(directory.join("index.js"), "x".repeat(5_000)).unwrap();
            Ok(())
        })
        .expect("first");
    let second = plan(scratch.path(), matrix).run(writing("tiny")).expect("second");

    for measured in &second.measured {
        assert_eq!(measured.size.raw, 4, "a stale build was measured: {measured:?}");
    }
}

#[test]
fn a_reduction_is_measured_against_the_baseline_rather_than_the_largest() {
    // Comparing against the worst configuration would flatter every result.
    let scratch = tempfile::tempdir().unwrap();
    let matrix = WebMatrix {
        targets: vec![Target::EsNext, Target::Es2015],
        minify: vec![true],
        tree_shaking: vec![true],
        splitting: vec![false],
        production: vec![true],
    };

    let sweep = plan(scratch.path(), matrix)
        .run(|configuration, _, directory| {
            // esnext is the default, so it is the baseline.
            let size = if configuration.target == Target::EsNext { 1_000 } else { 4_000 };
            std::fs::write(directory.join("index.js"), "q".repeat(size)).unwrap();
            Ok(())
        })
        .expect("sweeps");

    let reduction = sweep.best_reduction().expect("a baseline and a smallest");
    assert!(
        reduction.abs() < 0.001,
        "the baseline is already the smallest, so the reduction is zero, got {reduction}"
    );
}

// --- the matrix itself ------------------------------------------------------

#[test]
fn the_matrix_cardinality_matches_what_it_produces() {
    // A count that disagrees with reality makes every progress bar wrong.
    for matrix in [WebMatrix::default(), WebMatrix::targets_only()] {
        assert_eq!(matrix.configurations().len(), matrix.cardinality(), "{matrix:?}");
    }
}

#[test]
fn the_default_matrix_holds_no_controls() {
    // Sweeping unminified development builds doubles the cost to learn
    // something everyone already knows.
    for configuration in WebMatrix::default().configurations() {
        assert!(configuration.is_shippable(), "{configuration:?} is a control, not a candidate");
    }
}

#[test]
fn the_baseline_asserts_nothing_the_bundler_would_not_do_anyway() {
    // The native side shipped a baseline that asserted cargo's defaults
    // wrongly and made every number nine times too flattering. The web
    // baseline is a normal production build and nothing else.
    let baseline = WebConfiguration::default();
    assert!(baseline.minify && baseline.tree_shaking && baseline.production);
    assert!(!baseline.splitting, "splitting is a choice, not a default");
    assert_eq!(baseline.target, Target::EsNext, "transpiling is a choice too");
}

#[test]
fn every_configuration_has_a_distinct_name() {
    // Names are directory names and session keys, so a collision would make
    // one configuration measure another.
    let names: std::collections::BTreeSet<String> =
        WebMatrix::default().configurations().iter().map(|c| c.name()).collect();
    assert_eq!(names.len(), WebMatrix::default().cardinality());
}

#[test]
fn a_target_states_the_audience_it_costs_rather_than_only_the_bytes_it_saves() {
    // The smallest bundle is the one that excludes the most people, and that
    // is a product decision rather than a build setting.
    for target in Target::ALL {
        assert!(!target.audience().is_empty(), "{target:?} states no trade");
        assert!(!target.flag().is_empty());
    }
    assert!(Target::EsNext > Target::Es2015, "newer targets emit less");
}

#[test]
fn the_flags_never_edit_a_config_file() {
    // §5: the working tree stays untouched, so everything is a CLI flag.
    let flags = WebConfiguration::default().flags();
    assert!(flags.iter().any(|flag| flag.starts_with("--target=")));
    assert!(flags.iter().any(|flag| flag.contains("NODE_ENV")));
    assert!(flags.iter().all(|flag| flag.starts_with("--")), "everything is a flag: {flags:?}");
}

#[test]
fn tree_shaking_is_only_mentioned_when_it_is_turned_off() {
    // esbuild does it by default, so passing the positive is noise that can
    // drift from the default.
    let on = WebConfiguration { tree_shaking: true, ..WebConfiguration::default() };
    let off = WebConfiguration { tree_shaking: false, ..WebConfiguration::default() };
    assert!(!on.flags().iter().any(|f| f.contains("tree-shaking")));
    assert!(off.flags().iter().any(|f| f == "--tree-shaking=false"));
}
