//! Detecting the cargo project or workspace and enumerating its targets
//! (`F0.1`).

use binmap_core::capability::{Capabilities, Capability};
use binmap_core::error::{Error, Result};
use binmap_core::evidence::ToolInvocation;
use binmap_core::tool::ToolRunner;
use binmap_core::traits::{Target, TargetFamily};
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Whether the directory holds one package or a workspace of them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectKind {
    Package { name: String },
    Workspace { members: Vec<String> },
}

/// Only the parts of `cargo metadata` we actually read. Deserializing the
/// whole document would couple us to fields cargo is free to change.
#[derive(Debug, Deserialize)]
struct Metadata {
    packages: Vec<MetadataPackage>,
    workspace_members: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct MetadataPackage {
    id: String,
    name: String,
    manifest_path: PathBuf,
    targets: Vec<MetadataTarget>,
}

#[derive(Debug, Deserialize)]
struct MetadataTarget {
    name: String,
    kind: Vec<String>,
}

/// What Binmap can do with a Rust target in Phase 0.
///
/// One capability, declared honestly. The nav rail reads this, so a target
/// here shows a Target view and a Profile Lab and nothing else — the Size,
/// Failure, Perf and Replay entries are absent rather than present and empty
/// (§2.5).
fn phase_zero_capabilities() -> Capabilities {
    [Capability::ConfigurationSweep].into_iter().collect()
}

/// Which target kinds produce a single artifact worth measuring.
///
/// Tests, benches and examples are excluded: they are built from the same
/// code but they are not what ships, and sweeping them would measure the wrong
/// binary convincingly.
fn is_measurable(kind: &str) -> bool {
    matches!(kind, "bin" | "lib" | "rlib" | "cdylib" | "staticlib")
}

/// Find the project at `root` and enumerate what it offers.
///
/// Runs `cargo metadata`, so the answer comes from cargo rather than from our
/// own reading of a manifest — and the invocation is recorded like any other.
pub fn discover(runner: &ToolRunner, root: &Path) -> Result<(ProjectKind, Vec<Target>)> {
    if !root.join("Cargo.toml").exists() {
        return Err(Error::NoProject(root.to_path_buf()));
    }

    let output = runner.run(
        ToolInvocation::new(
            "cargo",
            ["metadata", "--no-deps", "--format-version", "1", "--offline"],
        )
        .in_directory(root.display().to_string()),
    )?;

    // `--offline` fails on a project whose dependencies are not vendored yet.
    // That is worth one retry online rather than a dead end for the user.
    let output = if output.succeeded() {
        output
    } else {
        runner.run(
            ToolInvocation::new("cargo", ["metadata", "--no-deps", "--format-version", "1"])
                .in_directory(root.display().to_string()),
        )?
    };

    if !output.succeeded() {
        let reason = output
            .stderr
            .lines()
            .find(|line| line.trim_start().starts_with("error"))
            .unwrap_or("cargo metadata failed")
            .trim()
            .to_string();
        return Err(Error::Config(reason));
    }

    let metadata: Metadata = serde_json::from_str(&output.stdout)
        .map_err(|source| Error::serialization("reading cargo metadata", source))?;

    let members: Vec<&MetadataPackage> = metadata
        .packages
        .iter()
        .filter(|package| metadata.workspace_members.contains(&package.id))
        .collect();

    // A workspace with one member reads as a package: the distinction that
    // matters to the project view is how many targets it has to group, not
    // which manifest key declared them.
    let kind = if members.len() == 1 {
        ProjectKind::Package { name: members[0].name.clone() }
    } else {
        ProjectKind::Workspace {
            members: members.iter().map(|package| package.name.clone()).collect(),
        }
    };

    let mut targets = Vec::new();
    for package in members {
        for target in &package.targets {
            let Some(kind) = target.kind.iter().find(|kind| is_measurable(kind)) else {
                continue;
            };
            targets.push(Target {
                id: format!("{}::{}", package.name, target.name),
                name: target.name.clone(),
                family: TargetFamily::Rust,
                package: package.name.clone(),
                manifest: package.manifest_path.clone(),
                capabilities: phase_zero_capabilities(),
            });
            let _ = kind;
        }
    }

    targets.sort_by(|a, b| a.id.cmp(&b.id));
    Ok((kind, targets))
}

#[cfg(test)]
mod tests {
    use super::*;
    use binmap_core::evidence::EvidenceStore;

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
        assert!(ids.contains(&"binmap-gui::binmap"), "{ids:?}");
        // Test and bench targets are not what ships, so they are not offered.
        assert!(!ids.iter().any(|id| id.contains("::tests")), "{ids:?}");
        // The discovery itself is on the record.
        assert!(!store.is_empty());
    }

    #[test]
    fn every_target_states_what_binmap_can_do_with_it() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap();
        let runner = ToolRunner::new(EvidenceStore::new(), root);
        let (_, targets) = discover(&runner, root).unwrap();
        let target = targets.first().expect("our own workspace has targets");
        assert_eq!(target.capabilities.sentence(), "Binmap can sweep build configurations.");
        // Nothing claims a capability Phase 0 has not built.
        assert!(!target.capabilities.has(Capability::ReplayDebugging));
    }
}
