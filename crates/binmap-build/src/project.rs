//! Detecting the cargo project or workspace and enumerating its targets
//! (`F0.1`).

use binmap_core::capability::{Capabilities, Capability};
use binmap_core::error::{Error, Result};
use binmap_core::evidence::ToolInvocation;
use binmap_core::tool::ToolRunner;
use binmap_core::traits::{Target, TargetFamily};
use cargo_metadata::{MetadataCommand, Package, TargetKind};
use std::path::Path;

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
fn phase_zero_capabilities() -> Capabilities {
    [Capability::ConfigurationSweep].into_iter().collect()
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
    let mut targets = Vec::new();
    for package in &members {
        for target in &package.targets {
            if !target.kind.iter().any(is_measurable) {
                continue;
            }
            report.push_str(&format!("{} {}\n", package.name, target.name));
            targets.push(Target {
                id: format!("{}::{}", package.name, target.name),
                name: target.name.to_string(),
                family: TargetFamily::Rust,
                package: package.name.to_string(),
                manifest: package.manifest_path.clone().into_std_path_buf(),
                capabilities: phase_zero_capabilities(),
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
        assert!(ids.contains(&"binmap::binmap"), "{ids:?}");
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
