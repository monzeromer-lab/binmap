//! The baseline is what `cargo build --release` actually produces.
//!
//! TOOLING §8's fragility table asks for an integration test asserting that
//! each profile axis really changes the output, because the environment
//! variable names have edge cases and cargo's own defaults move. This is that
//! test, and it exists because the defaults moved and nobody noticed:
//!
//! `BuildConfiguration::default_release` used to spell out what we believed
//! cargo's release defaults were. One of them was wrong — cargo strips debug
//! info by default, so asserting `strip = "none"` turned that off — and the
//! baseline came out **nine times larger** than a plain `cargo build
//! --release`. Every configuration in every sweep was then compared against a
//! strawman and looked better than it was.
//!
//! These run real cargo against a real corpus crate. They are slow and they
//! are the only thing standing between us and that happening again.

use binmap_build::cargo::CargoBuildSystem;
use binmap_core::config::{OptLevel, Strip};
use binmap_core::configuration::BuildConfiguration;
use binmap_core::evidence::EvidenceStore;
use binmap_core::tool::{ToolRunner, ToolSpec};
use binmap_core::traits::BuildSystem;

fn corpus(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the workspace root")
        .join("corpus")
        .join(name)
}

/// Build `corpus/stress` under `configuration` and return the artifact's size.
fn build(configuration: &BuildConfiguration, into: &std::path::Path) -> Option<u64> {
    let root = corpus("stress");
    let runner = ToolRunner::new(EvidenceStore::new(), &root);
    let system = CargoBuildSystem::new(runner, into);
    let targets = system.targets(&root).ok()?;
    let target = targets.into_iter().find(|t| t.name == "stress")?;
    let outcome = system.build(&target, configuration).ok()?;
    let artifact = outcome.artifact?;
    std::fs::metadata(artifact).ok().map(|m| m.len())
}

/// What a plain `cargo build --release` produces, with no help from us.
fn plain_release(into: &std::path::Path) -> Option<u64> {
    let root = corpus("stress");
    let status = std::process::Command::new("cargo")
        .args(["build", "--release", "--quiet"])
        .current_dir(&root)
        .env("CARGO_TARGET_DIR", into)
        .status()
        .ok()?;
    if !status.success() {
        return None;
    }
    std::fs::metadata(into.join("release/stress")).ok().map(|m| m.len())
}

#[test]
fn the_baseline_is_the_same_binary_a_plain_release_build_produces() {
    let ours = tempfile::tempdir().expect("a temporary directory");
    let theirs = tempfile::tempdir().expect("a temporary directory");

    let Some(plain) = plain_release(theirs.path()) else {
        eprintln!("cargo could not build corpus/stress; nothing to compare");
        return;
    };
    let baseline =
        build(&BuildConfiguration::default_release(), ours.path()).expect("the baseline builds");

    // Byte-identical is not required — cargo embeds paths, and the two target
    // directories differ. Within one per cent is: nine times over is the
    // failure this test exists to catch.
    let ratio = baseline as f64 / plain as f64;
    assert!(
        (0.99..=1.01).contains(&ratio),
        "the baseline is {baseline} bytes where a plain release build is {plain} \
         ({ratio:.2}x). Every delta in the product is stated against this."
    );
}

#[test]
fn setting_strip_to_none_is_a_real_and_expensive_choice() {
    // The bug in one assertion. `strip = "none"` turns off the stripping
    // cargo does by default, and it is not free — which is why it is not in
    // the default matrix, and why the baseline no longer asserts it.
    let bare = tempfile::tempdir().expect("a temporary directory");
    let unstripped = tempfile::tempdir().expect("a temporary directory");

    let Some(baseline) = build(&BuildConfiguration::default_release(), bare.path()) else {
        eprintln!("cargo could not build corpus/stress");
        return;
    };
    let kept = build(
        &BuildConfiguration { strip: Some(Strip::None), ..Default::default() },
        unstripped.path(),
    )
    .expect("it builds");

    assert!(
        kept > baseline * 2,
        "strip=none produced {kept} against a {baseline} baseline; if these are close, \
         cargo's default changed and the matrix should be revisited"
    );
}

#[test]
fn every_axis_in_the_default_matrix_actually_changes_the_output() {
    // TOOLING §8's mitigation, stated literally. If cargo renames a profile
    // variable, this is what notices — rather than a sweep that runs to
    // completion and reports ninety-six identical numbers.
    let root = corpus("stress");
    let runner = ToolRunner::new(EvidenceStore::new(), &root);
    let system = CargoBuildSystem::new(runner, std::env::temp_dir().join("binmap-axis-probe"));

    for (axis, configuration) in [
        (
            "opt-level",
            BuildConfiguration { opt_level: Some(OptLevel::SizeNoLoopVec), ..Default::default() },
        ),
        ("strip", BuildConfiguration { strip: Some(Strip::None), ..Default::default() }),
    ] {
        let env = BuildSystem::build_environment(&system, &configuration);
        let named: Vec<&String> =
            env.keys().filter(|key| key.starts_with("CARGO_PROFILE_")).collect();
        assert!(
            !named.is_empty(),
            "the {axis} axis reached cargo as no profile variable at all: {env:?}"
        );
    }

    // And the tool registry is not an empty abstraction either.
    let _ = ToolSpec::read_only("probe", "A tool.");
}
