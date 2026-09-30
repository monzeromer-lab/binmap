//! Detecting the cargo project or workspace and enumerating its targets
//! (`F0.1`).

use binmap_core::capability::{Capabilities, Capability};
use binmap_core::error::{Error, Result};
use binmap_core::evidence::ToolInvocation;
use binmap_core::tool::ToolRunner;
use binmap_core::traits::{Target, TargetFamily};
use cargo_metadata::{MetadataCommand, Package, TargetKind};
use std::path::Path;

/// The profiles a project declares, plus the ones cargo always has.
///
/// `F0.1` asks for targets *and profiles*. Only targets were enumerated, so a
/// project whose shipping profile is a custom one — `dist` is the common name,
/// and cargo has supported custom profiles since 1.57 — could not be swept at
/// all. The sweep still defaults to `release`, because that is what almost
/// everyone ships; this is what makes the other case reachable.
pub fn profiles(root: &Path) -> Vec<String> {
    // Cargo's own, which exist whether or not the manifest mentions them.
    let mut found = vec!["release".to_string(), "dev".to_string()];

    let manifest = root.join("Cargo.toml");
    let Ok(text) = std::fs::read_to_string(&manifest) else { return found };
    let Ok(document) = text.parse::<toml_edit::DocumentMut>() else { return found };
    let Some(profiles) = document.get("profile").and_then(toml_edit::Item::as_table) else {
        return found;
    };

    for (name, _) in profiles.iter() {
        if !found.iter().any(|existing| existing == name) {
            found.push(name.to_string());
        }
    }
    found
}

/// Give colliding targets distinct identifiers.
///
/// A package very commonly has a library and a binary with the same name —
/// `src/lib.rs` and `src/main.rs` — and both then answer to `package::name`.
/// Selecting one got the other: attribution asked for the binary, received the
/// library, and refused itself for not supporting size attribution.
///
/// Only the colliding ones gain a kind, because `stress::stress` reads better
/// than `stress::bin::stress` and most packages have no collision.
fn disambiguate(targets: &mut [Target]) {
    let mut seen: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for target in targets.iter() {
        *seen.entry(target.id.clone()).or_default() += 1;
    }
    for target in targets.iter_mut() {
        if seen.get(&target.id).is_some_and(|count| *count > 1) {
            target.id = format!("{}::{}::{}", target.package, target.kind, target.name);
        }
    }
}

/// Whether the directory holds one package or a workspace of them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectKind {
    Package { name: String },
    Workspace { members: Vec<String> },
}

/// What Binmap can do with a Rust target in Phase 0.
///
/// One capability, declared honestly. The nav rail reads this, so a target
/// here shows a Target view and a Profile Lab and nothing else — the Size,
/// Failure, Perf and Replay entries are absent rather than present and empty
/// (§2.5).
/// What Binmap can do with one particular target.
///
/// Derived rather than constant. It was a constant — every Rust target got
/// `[ConfigurationSweep]` — which made the capability model a decoration:
/// the nav rail filtered on a value that never varied, so nothing was ever
/// actually absent for a reason.
///
/// Three things decide it, and each is a real distinction:
///
/// - **The target kind.** A binary and a `cdylib` are single artifacts with a
///   symbol table; an `rlib` is an archive of object files, and attributing
///   bytes in one answers a different question. A `lib` is only built as an
///   rlib unless something links it.
/// - **The compilation target.** A `.wasm` module is served over a network, so
///   its compressed size is the number that matters and its uncompressed size
///   mostly is not. It also has no ELF symbol table.
/// - **What this build can actually do.** A capability declared for a phase
///   that has not shipped is a promise the interface will fail to keep, so
///   this only names what exists today.
fn capabilities_for(kind: &TargetKind, wasm: bool) -> Capabilities {
    let mut found = vec![
        // True of every cargo target: the profile matrix applies whatever the
        // artifact turns out to be.
        Capability::ConfigurationSweep,
    ];

    if wasm {
        // Size is the whole story for something served over a network, and it
        // is the compressed number that matters.
        found.push(Capability::CompressedSize);
        return found.into_iter().collect();
    }

    // A single linked artifact has a symbol table, so its bytes can be
    // attributed and its generics grouped. An rlib is an archive of object
    // files: attributing bytes inside one answers a different question, so it
    // is not claimed.
    if matches!(kind, TargetKind::Bin | TargetKind::CDyLib | TargetKind::StaticLib) {
        found.push(Capability::SizeAttribution);
        found.push(Capability::Monomorphization);
    }

    found.into_iter().collect()
}

/// Which target kinds produce a single artifact worth measuring.
///
/// Tests, benches and examples are excluded: they are built from the same
/// code but they are not what ships, and sweeping them would measure the wrong
/// binary convincingly.
fn is_measurable(kind: &TargetKind) -> bool {
    matches!(
        kind,
        TargetKind::Bin
            | TargetKind::Lib
            | TargetKind::RLib
            | TargetKind::CDyLib
            | TargetKind::StaticLib
    )
}

/// The compilation target this project builds for by default, if it names one.
///
/// From `.cargo/config.toml`'s `build.target`. A project that names one is
/// cross-compiling, and that changes what the gates can do: `cargo test`
/// inherits the setting, so the tests are built for a machine that is not this
/// one and cannot be run here.
pub fn default_target(root: &Path) -> Option<String> {
    let config = root.join(".cargo").join("config.toml");
    std::fs::read_to_string(config)
        .ok()
        .and_then(|text| text.parse::<toml_edit::DocumentMut>().ok())
        .and_then(|document| {
            document
                .get("build")
                .and_then(|build| build.get("target"))
                .and_then(|target| target.as_str().map(str::to_string))
        })
}

/// Whether this project builds for WebAssembly by default.
///
/// Read from `.cargo/config.toml`, which is where a wasm-only crate says so —
/// and the corpus has one, so this is not hypothetical.
fn builds_for_wasm(root: &Path) -> bool {
    default_target(root).is_some_and(|triple| triple.starts_with("wasm"))
}

/// Find the project at `root` and enumerate what it offers.
///
/// `cargo_metadata` is the workspace model (TOOLING §3.1): packages, targets,
/// features, the dependency graph and the target directory. Hand-rolling the
/// subset we need would work right up until cargo changed something.
///
/// The invocation is still recorded like any other, because the evidence store
/// is not an optional courtesy.
pub fn discover(runner: &ToolRunner, root: &Path) -> Result<(ProjectKind, Vec<Target>)> {
    let manifest = root.join("Cargo.toml");
    if !manifest.exists() {
        return Err(Error::NoProject(root.to_path_buf()));
    }

    // Recorded before the read, so a project that cannot be enumerated still
    // leaves a trace of the attempt.
    let pending = runner.store().begin(
        ToolInvocation::new("cargo", ["metadata", "--no-deps", "--format-version", "1"])
            .in_directory(root.display().to_string()),
    );

    let metadata = match MetadataCommand::new().manifest_path(&manifest).no_deps().exec() {
        Ok(metadata) => metadata,
        Err(error) => {
            let reason = error.to_string();
            runner.store().complete(pending, &reason, 1);
            return Err(Error::Config(reason));
        }
    };

    let members: Vec<&Package> = metadata.workspace_packages().into_iter().collect();

    let mut report = String::new();
    let wasm = builds_for_wasm(root);
    let mut targets = Vec::new();
    for package in &members {
        for target in &package.targets {
            if !target.kind.iter().any(is_measurable) {
                continue;
            }
            report.push_str(&format!("{} {}\n", package.name, target.name));
            let kind = target
                .kind
                .iter()
                .find(|kind| is_measurable(kind))
                .map(|kind| kind.to_string())
                .unwrap_or_else(|| "bin".to_string());

            targets.push(Target {
                // Disambiguated below, once every target is known: a package
                // with a lib and a bin of the same name needs the kind, and
                // one without it reads better without.
                id: format!("{}::{}", package.name, target.name),
                name: target.name.to_string(),
                kind,
                family: TargetFamily::Rust,
                package: package.name.to_string(),
                manifest: package.manifest_path.clone().into_std_path_buf(),
                capabilities: capabilities_for(
                    target.kind.first().unwrap_or(&TargetKind::Bin),
                    wasm,
                ),
            });
        }
    }
    runner.store().complete(pending, report, 0);

    // A workspace with one member reads as a package: the distinction that
    // matters to the project view is how many targets it has to group, not
    // which manifest key declared them.
    let kind = if members.len() == 1 {
        ProjectKind::Package { name: members[0].name.to_string() }
    } else {
        ProjectKind::Workspace {
            members: members.iter().map(|package| package.name.to_string()).collect(),
        }
    };

    disambiguate(&mut targets);
    targets.sort_by(|a, b| a.id.cmp(&b.id));
    Ok((kind, targets))
}

#[cfg(test)]
mod tests {
    use super::*;
    use binmap_core::evidence::EvidenceStore;

    #[test]
    fn cargos_own_profiles_are_always_offered() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("Cargo.toml"), "[package]\nname = \"x\"\n").unwrap();
        let found = profiles(directory.path());
        assert!(found.contains(&"release".to_string()));
        assert!(found.contains(&"dev".to_string()));
    }

    #[test]
    fn a_custom_profile_is_discovered_from_the_manifest() {
        // The case F0.1 covers and nothing reached: a project that ships from
        // `dist` rather than `release`.
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(
            directory.path().join("Cargo.toml"),
            "[package]\nname = \"x\"\n\n[profile.dist]\ninherits = \"release\"\nlto = true\n",
        )
        .unwrap();
        let found = profiles(directory.path());
        assert!(found.contains(&"dist".to_string()), "{found:?}");
        // And the built-ins are not duplicated by a manifest that names them.
        assert_eq!(found.iter().filter(|p| *p == "release").count(), 1);
    }

    #[test]
    fn our_own_workspace_declares_a_release_profile_and_it_is_not_listed_twice() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap();
        let found = profiles(root);
        assert_eq!(found.iter().filter(|p| *p == "release").count(), 1, "{found:?}");
    }

    #[test]
    fn a_directory_with_no_manifest_says_so_by_path() {
        let runner = ToolRunner::new(EvidenceStore::new(), "/tmp");
        let error = discover(&runner, Path::new("/tmp")).unwrap_err();
        assert!(matches!(error, Error::NoProject(_)));
        assert!(error.to_string().contains("/tmp"));
    }

    #[test]
    fn our_own_workspace_enumerates_as_a_workspace() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap();
        let store = EvidenceStore::new();
        let runner = ToolRunner::new(store.clone(), root);
        let (kind, targets) = discover(&runner, root).unwrap();

        match kind {
            ProjectKind::Workspace { members } => {
                assert!(members.contains(&"binmap-core".to_string()), "{members:?}");
            }
            other => panic!("expected a workspace, got {other:?}"),
        }

        let ids: Vec<&str> = targets.iter().map(|t| t.id.as_str()).collect();
        assert!(ids.contains(&"binmap::binmap"), "{ids:?}");
        // Test and bench targets are not what ships, so they are not offered.
        assert!(!ids.iter().any(|id| id.contains("::tests")), "{ids:?}");
        // The discovery itself is on the record.
        assert!(!store.is_empty());
    }

    #[test]
    fn a_wasm_target_declares_that_compressed_size_is_what_matters() {
        // The corpus has one, and it is the case the capability model exists
        // for: a .wasm module is served over a network, so the compressed
        // number is the one that counts.
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .unwrap()
            .join("corpus/wasm");
        if !root.exists() {
            return;
        }
        assert!(builds_for_wasm(&root), "corpus/wasm builds for wasm by default");

        let runner = ToolRunner::new(EvidenceStore::new(), &root);
        let (_, targets) = discover(&runner, &root).unwrap();
        let target = targets.first().expect("it has a target");
        assert!(target.capabilities.has(Capability::CompressedSize));
        assert!(target.capabilities.has(Capability::ConfigurationSweep));
    }

    #[test]
    fn a_binary_can_have_its_bytes_attributed_and_an_rlib_cannot() {
        // An rlib is an archive of object files; attributing bytes inside one
        // answers a different question from attributing a linked binary's.
        let binary = capabilities_for(&TargetKind::Bin, false);
        assert!(binary.has(Capability::SizeAttribution));
        assert!(binary.has(Capability::Monomorphization));

        let library = capabilities_for(&TargetKind::Lib, false);
        assert!(!library.has(Capability::SizeAttribution));
    }

    #[test]
    fn a_native_target_does_not_claim_compressed_size() {
        // It is not served over a network, so the number would be noise.
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap();
        let runner = ToolRunner::new(EvidenceStore::new(), root);
        let (_, targets) = discover(&runner, root).unwrap();
        let target = targets.first().expect("our own workspace has targets");
        assert!(!target.capabilities.has(Capability::CompressedSize));
    }

    #[test]
    fn nothing_claims_a_capability_this_build_has_not_shipped() {
        // A capability declared for a phase that has not landed puts an entry
        // in the nav rail that opens on nothing.
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap();
        let runner = ToolRunner::new(EvidenceStore::new(), root);
        let (_, targets) = discover(&runner, root).unwrap();
        for target in targets {
            for unshipped in [
                Capability::CrashAnalysis,
                Capability::Disassembly,
                Capability::PerformanceAttribution,
                Capability::ReplayDebugging,
            ] {
                assert!(
                    !target.capabilities.has(unshipped),
                    "{} claims {unshipped}, which Phase 0 has not built",
                    target.id
                );
            }
        }
    }

    #[test]
    fn every_target_states_what_binmap_can_do_with_it() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap();
        let runner = ToolRunner::new(EvidenceStore::new(), root);
        let (_, targets) = discover(&runner, root).unwrap();
        let target = targets.first().expect("our own workspace has targets");
        assert!(
            target.capabilities.sentence().starts_with("Binmap can sweep build configurations"),
            "{}",
            target.capabilities.sentence()
        );
        // Nothing claims a capability Phase 0 has not built.
        assert!(!target.capabilities.has(Capability::ReplayDebugging));
    }
}
