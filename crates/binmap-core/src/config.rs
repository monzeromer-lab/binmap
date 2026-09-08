//! Trust tiers, the sweep matrix, and the settings a session runs under.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// What the application is allowed to do without asking again.
///
/// Ordered, so "requires at least" is a comparison. Raising the tier is always
/// a deliberate act by the user: an action above the current tier asks for the
/// raise, names the gates it will run, and states what it will write (`U9`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TrustTier {
    /// Read the project, run measurements, write nothing outside our own
    /// target directory. The default, and enough for the whole of Phase 0's
    /// analysis.
    #[default]
    Observe,
    /// Additionally: generate patches and show their diffs. Still writes
    /// nothing to the user's tree.
    Propose,
    /// Additionally: write an approved change into the working tree, having
    /// run the gates first.
    Apply,
    /// Additionally: commit. Never the default, and never reached by
    /// inference — `N7`.
    Commit,
}

impl TrustTier {
    pub fn label(self) -> &'static str {
        match self {
            TrustTier::Observe => "Observe",
            TrustTier::Propose => "Propose",
            TrustTier::Apply => "Apply",
            TrustTier::Commit => "Commit",
        }
    }

    /// The tier's own description of what it permits, as the tier dialog shows
    /// it. Written as a sentence because a user deciding whether to raise a
    /// tier needs a sentence, not a permission bit.
    pub fn permits(self) -> &'static str {
        match self {
            TrustTier::Observe => "Read the project and measure it. Writes only to Binmap's own target directory.",
            TrustTier::Propose => "Also generate patches and show their diffs. Your working tree is not touched.",
            TrustTier::Apply => "Also write an approved change into your working tree, after the gates pass.",
            TrustTier::Commit => "Also commit approved changes. Nothing reaches this tier on its own.",
        }
    }

    pub const ALL: [TrustTier; 4] =
        [TrustTier::Observe, TrustTier::Propose, TrustTier::Apply, TrustTier::Commit];

    /// Check an action against the session's tier, producing the error that
    /// states the reason rather than making the action disappear.
    pub fn require(self, required: TrustTier, action: &str) -> crate::Result<()> {
        if self >= required {
            Ok(())
        } else {
            Err(crate::Error::TierTooLow {
                action: action.to_string(),
                required,
                current: self,
            })
        }
    }
}

impl std::fmt::Display for TrustTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// The user's benchmark, where they have one. Runtime is measured through this
/// and through nothing else — we do not invent a workload on their behalf.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchmarkCommand {
    pub program: String,
    #[serde(default)]
    pub arguments: Vec<String>,
    /// How many times to run it per configuration. The noise floor is measured
    /// with the same count so the two are comparable.
    #[serde(default = "default_samples")]
    pub samples: u32,
}

fn default_samples() -> u32 {
    7
}

/// The axes of the sweep, and the values to try on each.
///
/// An empty axis means "leave it at the profile default", which is different
/// from "try every value" — a sweep that silently expanded would be a sweep
/// the user did not ask for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SweepMatrix {
    pub opt_level: Vec<OptLevel>,
    pub lto: Vec<Lto>,
    pub codegen_units: Vec<u32>,
    pub panic: Vec<PanicStrategy>,
    pub strip: Vec<Strip>,
    pub debug: Vec<DebugInfo>,
    pub overflow_checks: Vec<bool>,
    /// `F0.3`: the extended sweep. Off by default because it needs a nightly
    /// toolchain and the rust-src component, and a sweep that fails on the
    /// user's stable toolchain is worse than one that did not offer the axis.
    #[serde(default)]
    pub build_std: Vec<BuildStd>,
    #[serde(default)]
    pub target_cpu: Vec<String>,
}

impl Default for SweepMatrix {
    /// The default matrix: the axes that pay for themselves on almost every
    /// crate, sized so a first sweep finishes rather than impresses.
    fn default() -> Self {
        Self {
            opt_level: vec![OptLevel::Three, OptLevel::Two, OptLevel::Size, OptLevel::SizeNoLoopVec],
            lto: vec![Lto::Off, Lto::Thin, Lto::Fat],
            codegen_units: vec![16, 1],
            panic: vec![PanicStrategy::Unwind, PanicStrategy::Abort],
            strip: vec![Strip::None, Strip::Symbols],
            debug: vec![DebugInfo::None],
            overflow_checks: vec![false],
            build_std: Vec::new(),
            target_cpu: Vec::new(),
        }
    }
}

impl SweepMatrix {
    /// How many configurations this matrix describes. Shown before the sweep
    /// starts, because the honest answer to "how long will this take" is this
    /// number times one build.
    pub fn cardinality(&self) -> usize {
        fn axis(len: usize) -> usize {
            len.max(1)
        }
        axis(self.opt_level.len())
            * axis(self.lto.len())
            * axis(self.codegen_units.len())
            * axis(self.panic.len())
            * axis(self.strip.len())
            * axis(self.debug.len())
            * axis(self.overflow_checks.len())
            * axis(self.build_std.len())
            * axis(self.target_cpu.len())
    }
}

macro_rules! cargo_value {
    ($name:ident { $($variant:ident => $serialized:literal),* $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        pub enum $name { $($variant),* }

        impl $name {
            /// The value as it appears in `Cargo.toml` and on the `--config`
            /// command line.
            pub fn as_toml(self) -> &'static str {
                match self { $(Self::$variant => $serialized),* }
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_toml().trim_matches('"'))
            }
        }
    };
}

cargo_value!(OptLevel {
    Zero => "0",
    One => "1",
    Two => "2",
    Three => "3",
    Size => "\"s\"",
    SizeNoLoopVec => "\"z\"",
});

cargo_value!(Lto {
    Off => "false",
    Thin => "\"thin\"",
    Fat => "\"fat\"",
});

cargo_value!(PanicStrategy {
    Unwind => "\"unwind\"",
    Abort => "\"abort\"",
});

cargo_value!(Strip {
    None => "\"none\"",
    Debuginfo => "\"debuginfo\"",
    Symbols => "\"symbols\"",
});

cargo_value!(DebugInfo {
    None => "0",
    LineTablesOnly => "\"line-tables-only\"",
    Limited => "1",
    Full => "2",
});

cargo_value!(BuildStd {
    Off => "off",
    Core => "core,alloc",
    PanicImmediateAbort => "core,alloc,panic_immediate_abort",
});

/// Everything a session needs to know about the project it was opened on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectConfig {
    /// The directory holding the workspace or package manifest.
    pub root: PathBuf,
    /// The package to sweep, when the root is a workspace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,
    /// The binary or library target within that package.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub benchmark: Option<BenchmarkCommand>,
    /// The test command, where the user's is not `cargo test`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test_command: Option<BenchmarkCommand>,
    #[serde(default)]
    pub matrix: SweepMatrix,
    /// Our target directory, kept separate so the user's own cache is never
    /// disturbed (`F0.8`).
    pub target_directory: PathBuf,
    /// The cap on concurrent builds. Defaults to half the machine's parallelism,
    /// because a sweep that saturates the machine also invalidates its own
    /// timing measurements.
    pub parallelism: usize,
    #[serde(default)]
    pub trust_tier: TrustTier,
}

impl ProjectConfig {
    /// Open a project at `root`.
    ///
    /// The path is made absolute here. Every tool runs with the project root
    /// as its working directory, so a relative target directory would be
    /// resolved against the root a second time and the sweep would build into
    /// `project/project/target` — which it did, until this line existed.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        let root = root.canonicalize().unwrap_or(root);
        let target_directory = root.join("target").join("binmap");
        Self {
            root,
            package: None,
            target: None,
            benchmark: None,
            test_command: None,
            matrix: SweepMatrix::default(),
            target_directory,
            parallelism: default_parallelism(),
            trust_tier: TrustTier::default(),
        }
    }
}

fn default_parallelism() -> usize {
    std::thread::available_parallelism().map(|n| (n.get() / 2).max(1)).unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blocked_action_states_its_reason() {
        let error = TrustTier::Observe.require(TrustTier::Apply, "write Cargo.toml").unwrap_err();
        let message = error.to_string();
        assert!(message.contains("write Cargo.toml"), "{message}");
        assert!(message.contains("Apply") && message.contains("Observe"), "{message}");
    }

    #[test]
    fn the_default_matrix_states_its_own_size() {
        // 4 opt-levels x 3 lto x 2 cgu x 2 panic x 2 strip x 1 debug x 1 overflow
        assert_eq!(SweepMatrix::default().cardinality(), 96);
    }

    #[test]
    fn an_empty_axis_counts_as_the_profile_default_not_as_zero() {
        let matrix = SweepMatrix {
            opt_level: vec![OptLevel::Three],
            lto: Vec::new(),
            codegen_units: Vec::new(),
            panic: Vec::new(),
            strip: Vec::new(),
            debug: Vec::new(),
            overflow_checks: Vec::new(),
            build_std: Vec::new(),
            target_cpu: Vec::new(),
        };
        assert_eq!(matrix.cardinality(), 1);
    }

    #[test]
    fn cargo_values_serialize_the_way_cargo_reads_them() {
        assert_eq!(OptLevel::Size.as_toml(), "\"s\"");
        assert_eq!(Lto::Off.as_toml(), "false");
        assert_eq!(OptLevel::Size.to_string(), "s");
    }
}
