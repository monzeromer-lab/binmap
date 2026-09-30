//! Web project discovery (`TOOLING-WEB §9`).
//!
//! The distinction being tested throughout: a project that *has* a bundler is
//! not the same as a project that *has been built*, and conflating them
//! produces a target that looks ready and then measures nothing.

use binmap_core::Capability;
use binmap_web::project::{Bundler, discover};
use binmap_web::tier::Tier;
use std::path::{Path, PathBuf};

fn corpus() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus/web")
}

fn write(root: &Path, relative: &str, contents: &str) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, contents).unwrap();
}

#[test]
fn the_real_corpus_project_is_discovered_with_its_bundler_and_build() {
    let project = discover(&corpus()).expect("corpus/web has a package.json");

    assert_eq!(project.name, "binmap-corpus-web");
    assert_eq!(project.bundler, Some(Bundler::Esbuild));

    if project.output_directory.is_none() {
        eprintln!("corpus/web is not built; run `npm run build:split` there");
        return;
    }
    assert!(!project.assets.is_empty(), "a built project has assets");
    assert!(project.metafile.is_some(), "the corpus build emits a metafile");
    assert_eq!(project.detection.chosen, Tier::EsbuildMetafile, "the best tier is taken");
}

#[test]
fn a_built_project_offers_attribution_and_a_web_target() {
    let project = discover(&corpus()).unwrap();
    if project.output_directory.is_none() {
        return;
    }

    let target = project.target();
    assert_eq!(target.family, binmap_core::traits::TargetFamily::TypeScript);
    assert!(target.capabilities.has(Capability::SizeAttribution));
    assert!(target.capabilities.has(Capability::CompressedSize));
    assert!(target.capabilities.has(Capability::SourceMapping));
}

#[test]
fn a_web_target_never_offers_what_the_web_does_not_have() {
    // §1: the entire hard half of the native backend is absent. The capability
    // set is where a user is told that, rather than by clicking something that
    // fails.
    let project = discover(&corpus()).unwrap();
    let capabilities = project.target().capabilities;

    for absent in [
        Capability::Disassembly,
        Capability::CrashAnalysis,
        Capability::ReplayDebugging,
        Capability::Monomorphization,
    ] {
        assert!(!capabilities.has(absent), "{absent:?} does not exist on the web");
    }
}

#[test]
fn a_project_that_has_never_been_built_says_so_rather_than_offering_attribution() {
    // The mistake this guards: declaring a bundler in package.json is a fact
    // about intent, not about the filesystem.
    let scratch = tempfile::tempdir().unwrap();
    write(
        scratch.path(),
        "package.json",
        r#"{"name":"unbuilt","devDependencies":{"esbuild":"^0.25.0"}}"#,
    );

    let project = discover(scratch.path()).expect("a package.json is enough to be a project");
    assert_eq!(project.bundler, Some(Bundler::Esbuild));
    assert!(project.output_directory.is_none());
    assert_eq!(project.detection.chosen, Tier::AssetSizesOnly);

    assert!(
        !project.target().capabilities.has(Capability::SizeAttribution),
        "an unbuilt project must not claim it can attribute anything"
    );
    assert!(
        project.detection.describe().contains("has not been built"),
        "and it must say why: {}",
        project.detection.describe()
    );
}

#[test]
fn a_build_with_maps_but_no_metafile_falls_back_and_names_the_fix() {
    // §2.3 and §9: when the better tier is missing, say so rather than
    // degrading quietly, and state the exact remedy.
    let scratch = tempfile::tempdir().unwrap();
    write(scratch.path(), "package.json", r#"{"name":"mapped","devDependencies":{"esbuild":"1"}}"#);
    write(scratch.path(), "dist/app.js", "console.log(1)");
    write(scratch.path(), "dist/app.js.map", "{}");

    let project = discover(scratch.path()).unwrap();
    assert_eq!(project.detection.chosen, Tier::SourceMaps);

    let described = project.detection.describe();
    assert!(described.contains("--metafile"), "the exact fix is named: {described}");
}

#[test]
fn a_build_with_neither_maps_nor_metadata_offers_file_sizes_and_admits_it() {
    let scratch = tempfile::tempdir().unwrap();
    write(scratch.path(), "package.json", r#"{"name":"bare"}"#);
    write(scratch.path(), "dist/app.js", "console.log(1)");

    let project = discover(scratch.path()).unwrap();
    assert_eq!(project.detection.chosen, Tier::AssetSizesOnly);
    assert!(!project.target().capabilities.has(Capability::SizeAttribution));
    // Transfer size still works: it needs nothing but the bytes.
    assert!(project.target().capabilities.has(Capability::CompressedSize));
}

#[test]
fn a_source_map_is_not_itself_listed_as_an_asset() {
    // A user does not download one, so counting it as an asset would put it in
    // the size total.
    let scratch = tempfile::tempdir().unwrap();
    write(scratch.path(), "package.json", r#"{"name":"m"}"#);
    write(scratch.path(), "dist/app.js", "x");
    write(scratch.path(), "dist/app.js.map", "{}");

    let project = discover(scratch.path()).unwrap();
    assert_eq!(project.assets.len(), 1);
    assert!(project.assets[0].path.ends_with("app.js"));
    assert!(project.assets[0].map.is_some(), "but the map is attached to it");
}

#[test]
fn a_bundler_is_detected_from_a_script_as_well_as_a_dependency() {
    // A project can call a bundler through a script without declaring it.
    let scratch = tempfile::tempdir().unwrap();
    write(
        scratch.path(),
        "package.json",
        r#"{"name":"scripted","scripts":{"build":"esbuild src/x.ts --bundle"}}"#,
    );
    assert_eq!(discover(scratch.path()).unwrap().bundler, Some(Bundler::Esbuild));
}

#[test]
fn a_word_inside_a_script_is_not_mistaken_for_the_bundler() {
    // "my-vite-helper" is not vite. Substring matching would say it was.
    let scratch = tempfile::tempdir().unwrap();
    write(
        scratch.path(),
        "package.json",
        r#"{"name":"careful","scripts":{"build":"node ./scripts/vite-like-thing.js"}}"#,
    );
    assert_eq!(discover(scratch.path()).unwrap().bundler, None);
}

#[test]
fn a_project_with_several_bundlers_gets_the_one_with_the_best_metadata() {
    // A project can depend on vite and still build with esbuild.
    let scratch = tempfile::tempdir().unwrap();
    write(
        scratch.path(),
        "package.json",
        r#"{"name":"both","devDependencies":{"vite":"5","esbuild":"0.25"}}"#,
    );
    assert_eq!(discover(scratch.path()).unwrap().bundler, Some(Bundler::Esbuild));
}

#[test]
fn every_bundler_states_how_to_make_it_emit_metadata() {
    for bundler in [Bundler::Esbuild, Bundler::Webpack, Bundler::Vite, Bundler::Rollup] {
        assert!(!bundler.how_to_emit_metadata().is_empty(), "{bundler:?} offers no remedy");
        assert!(!bundler.label().is_empty());
        assert!(bundler.best_possible_tier().attributes_anything());
    }
    assert_eq!(Bundler::Esbuild.best_possible_tier(), Tier::EsbuildMetafile);
    assert_eq!(Bundler::Webpack.best_possible_tier(), Tier::WebpackStats);
}

#[test]
fn a_directory_with_no_package_json_is_not_a_web_project() {
    let scratch = tempfile::tempdir().unwrap();
    let error = discover(scratch.path()).expect_err("nothing here");
    assert!(error.to_string().contains("no package.json"), "{error}");
}

#[test]
fn a_package_json_that_is_not_json_says_which_file_is_wrong() {
    let scratch = tempfile::tempdir().unwrap();
    write(scratch.path(), "package.json", "{ this is not json");
    let error = discover(scratch.path()).expect_err("unreadable");
    assert!(error.to_string().contains("package.json"), "{error}");
}

#[test]
fn a_package_json_with_no_name_falls_back_to_the_directory() {
    let scratch = tempfile::tempdir().unwrap();
    write(scratch.path(), "package.json", "{}");
    let project = discover(scratch.path()).unwrap();
    assert!(!project.name.is_empty(), "a target with no name cannot be selected");
}
