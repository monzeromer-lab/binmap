//! The seam between the interface and everything that computes.
//!
//! Dropping the IPC boundary removed the thing that structurally prevented a
//! view from calling an analysis, so the separation is enforced by the
//! dependency graph instead: `binmap-gui` depends on this crate and on
//! `binmap-session`, and reaches analyses through [`Engine`]. It cannot call an
//! analysis because it does not depend on one (§2.4).
//!
//! Every method here either returns immediately or returns a [`RunId`] and
//! streams. Nothing blocks the interface.

use crate::configuration::BuildConfiguration;
use crate::event::{Cancellation, EventSink, RunId};
use crate::evidence::Evidence;
use crate::finding::Finding;
use crate::traits::Target;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// One probe of the environment, and the exact command that fixes it.
///
/// `U10`'s whole point: never assume a capability that has not been confirmed,
/// and when one is missing, say what to type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Probe {
    /// The group the environment panel files this under.
    pub group: String,
    pub name: String,
    pub status: ProbeStatus,
    /// What was found, in one line: a version, a path, or what was missing.
    pub detail: String,
    /// The command that fixes it. `None` when nothing is wrong.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remedy: Option<String>,
    /// The label for the button beside the fix, where the fix can be applied
    /// from inside the application. `U10`: every fix that can be applied
    /// in-app has a button next to it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
}

impl Probe {
    pub fn present(
        group: impl Into<String>,
        name: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            group: group.into(),
            name: name.into(),
            status: ProbeStatus::Present,
            detail: detail.into(),
            remedy: None,
            action: None,
        }
    }

    /// Something is wrong, and this is the command that fixes it.
    ///
    /// The remedy is not optional here: a probe that reports a problem without
    /// saying what to type has told the user there is a problem and left them
    /// with it.
    pub fn needs(
        group: impl Into<String>,
        name: impl Into<String>,
        status: ProbeStatus,
        detail: impl Into<String>,
        remedy: impl Into<String>,
    ) -> Self {
        Self {
            group: group.into(),
            name: name.into(),
            status,
            detail: detail.into(),
            remedy: Some(remedy.into()),
            action: None,
        }
    }

    /// Attach the label for the in-app button.
    pub fn with_action(mut self, action: impl Into<String>) -> Self {
        self.action = Some(action.into());
        self
    }

    pub fn needs_attention(&self) -> bool {
        self.status != ProbeStatus::Present
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeStatus {
    /// Present and usable.
    Present,
    /// Absent, and something depends on it. The remedy is populated.
    Missing,
    /// Present but not usable as-is — wrong version, or a kernel setting in the
    /// way. The remedy is populated.
    Unusable,
}

impl ProbeStatus {
    pub fn label(self) -> &'static str {
        match self {
            ProbeStatus::Present => "Present",
            ProbeStatus::Missing => "Missing",
            ProbeStatus::Unusable => "Unusable",
        }
    }
}

/// A change the user may apply, and everything they need to decide.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Proposal {
    pub id: String,
    /// The finding this proposal acts on.
    pub finding: String,
    pub summary: String,
    /// The unified diff, exactly as it would be written.
    pub diff: String,
    /// The files it touches, so the apply dialog can state what it will write.
    pub writes: Vec<std::path::PathBuf>,
}

/// A run the engine can be asked to start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    /// Sweep the configured matrix over the selected target (`F0.2`, `F0.3`).
    Sweep { target: String },
    /// Resume a sweep that was cancelled, skipping configurations already
    /// measured (`F0.8`).
    ResumeSweep { run: RunId },
    /// Measure the machine's noise floor by timing one unchanged binary
    /// repeatedly. Runs before any runtime comparison, and its result is shown
    /// beside every timing number.
    NoiseFloor { target: String },
    /// Verify a proposal through the five gates without applying it.
    Verify { proposal: String },
    /// Write a verified proposal into the working tree. Requires
    /// [`TrustTier::Tune`](crate::config::TrustTier::Tune).
    Apply { proposal: String },
}

/// What the interface may ask of the engine.
///
/// Object-safe: the interface holds `Arc<dyn Engine>` and never names a
/// concrete engine type.
pub trait Engine: Send + Sync {
    /// The targets this project offers, grouped by family in the project view.
    fn targets(&self) -> crate::Result<Vec<Target>>;

    /// Every probe, re-run. The environment panel's re-check action calls
    /// this, which is what makes a missing tool recoverable without
    /// restarting.
    fn probe_environment(&self) -> Vec<Probe>;

    /// Start work. Returns as soon as the run is registered; results arrive on
    /// `events`.
    fn start(
        &self,
        request: Request,
        events: Arc<dyn EventSink>,
    ) -> crate::Result<(RunId, Cancellation)>;

    /// Every finding discovered so far, for a view that opened after the run
    /// began.
    fn findings(&self) -> Vec<Finding>;

    /// The evidence behind a finding, for the Inspector's Evidence tab.
    fn evidence(&self, finding: &str) -> Vec<Evidence>;

    /// The proposals currently on offer.
    fn proposals(&self) -> Vec<Proposal>;

    /// Turn a frontier point into a `Cargo.toml` edit, without writing
    /// anything. Writing it is a separate, tier-gated [`Request::Apply`].
    fn propose_configuration(&self, configuration: &BuildConfiguration) -> crate::Result<Proposal>;
}
