//! The four traits artifact-specific logic is confined to.
//!
//! Roughly eighty per cent of the interface is shared across backends. These
//! traits are where the other twenty per cent lives, and adding a language is
//! a matter of implementing them — a phase of work, not a weekend (§2.5).

use crate::artifact::ArtifactSize;
use crate::capability::Capabilities;
use crate::configuration::BuildConfiguration;
use crate::evidence::EvidenceId;
use crate::location::{Location, SourceSpan};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// A target the user can select: one binary, library, bundle or assembly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Target {
    /// Stable across sessions; used as the session key.
    pub id: String,
    pub name: String,
    /// The language family the project view groups by.
    pub family: TargetFamily,
    pub package: String,
    pub manifest: PathBuf,
    pub capabilities: Capabilities,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TargetFamily {
    Rust,
    TypeScript,
    CSharp,
}

impl TargetFamily {
    pub fn label(self) -> &'static str {
        match self {
            TargetFamily::Rust => "Rust",
            TargetFamily::TypeScript => "TypeScript",
            TargetFamily::CSharp => "C#",
        }
    }
}

/// What the build system was asked to do, and what came back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildOutcome {
    pub configuration: BuildConfiguration,
    /// Absent when the build failed.
    pub artifact: Option<PathBuf>,
    pub succeeded: bool,
    pub duration: Duration,
    /// Warnings, so a configuration that builds but complains more than the
    /// baseline is visible rather than silently equal.
    pub warnings: usize,
    /// The evidence record for the build invocation itself.
    pub evidence: EvidenceId,
}

/// Finds targets and builds them under a configuration.
pub trait BuildSystem: Send + Sync {
    /// Enumerate what this project offers (`F0.1`).
    fn targets(&self, root: &Path) -> crate::Result<Vec<Target>>;

    /// Build one configuration. Implementations write only into the target
    /// directory they were configured with.
    fn build(
        &self,
        target: &Target,
        configuration: &BuildConfiguration,
    ) -> crate::Result<BuildOutcome>;

    /// The environment this configuration is built under.
    ///
    /// The verification gates run under exactly this environment, so a gate
    /// verdict is about the configuration that was built rather than about
    /// whatever the plain build command happens to produce. Getting this wrong
    /// means a ninety-six point sweep gates every candidate on the same
    /// baseline build and still reports "tests passing" — which is a wrong
    /// answer delivered confidently, and the failure this method exists to
    /// prevent.
    ///
    /// It must include the target directory, so the gates build where the
    /// sweep built and never in the user's own cache (`F0.8`).
    fn build_environment(
        &self,
        configuration: &BuildConfiguration,
    ) -> std::collections::BTreeMap<String, String>;
}

/// Reads a built artifact's structure.
pub trait ArtifactReader: Send + Sync {
    fn size(&self, artifact: &Path) -> crate::Result<ArtifactSize>;

    /// The units bytes attribute to: crates, packages, assemblies. Empty until
    /// Phase 1 for the native backend.
    fn units(&self, _artifact: &Path) -> crate::Result<Vec<UnitSize>> {
        Ok(Vec::new())
    }
}

/// Bytes attributed to one unit of the artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnitSize {
    pub location: Location,
    pub bytes: u64,
    /// How many symbols or modules rolled up into this figure.
    pub items: u32,
}

/// Maps the artifact back to source.
pub trait Symbolizer: Send + Sync {
    fn source_for(&self, artifact: &Path, address: u64) -> crate::Result<Option<SourceSpan>>;

    /// The human-readable form of a symbol name. For Rust this is v0
    /// demangling; for a managed backend the name arrives readable and this is
    /// the identity.
    fn demangle(&self, symbol: &str) -> String {
        symbol.to_string()
    }
}

/// Times a workload and knows how much of the result is noise.
pub trait MeasurementSource: Send + Sync {
    /// Run the user's benchmark against one artifact and return the samples.
    /// Samples, not a mean: the caller decides what is significant.
    fn samples(&self, artifact: &Path) -> crate::Result<Vec<Duration>>;
}
