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

/// One configuration, measured, as the Profile Lab renders it.
///
/// The engine held all of this and exposed none of it, so the interface had no
/// source for its table, its scatter or its noise-floor line. Everything here
/// is a measurement or a conclusion drawn from one; nothing is presentation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Measurement {
    /// The configuration's stable name, and the row's identity.
    pub id: String,
    /// The flags, as a user would read them.
    pub flags: String,
    /// The axes this configuration set, for the selected-configuration panel.
    pub settings: Vec<(String, String)>,
    /// `None` when the candidate did not build.
    pub size_bytes: Option<u64>,
    /// Against the baseline. Negative is smaller.
    pub size_delta: Option<i64>,
    /// `None` when no benchmark is declared — which is different from fast.
    pub runtime_nanos: Option<u64>,
    /// `None` when the sweep built concurrently and wall-clock time therefore
    /// measured the machine's load rather than the configuration.
    pub build_time_nanos: Option<u64>,
    pub gates: crate::gate::VerificationReport,
    /// Derived by Pareto dominance, against the machine's noise floor. Never
    /// flagged by whatever produced the point.
    pub on_frontier: bool,
    pub built: bool,
}

impl Measurement {
    pub fn passed(&self) -> bool {
        self.gates.passed()
    }

    /// The gate that rejected it, for the table's Gates column.
    pub fn rejected_by(&self) -> Option<crate::gate::Gate> {
        self.gates.rejected_by()
    }
}

/// One sweep, and everything the Profile Lab needs to draw it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SweepSummary {
    pub run: crate::event::RunId,
    pub target: String,
    /// What `cargo build --release` produces with no help from us. Every delta
    /// is against this.
    pub baseline_bytes: Option<u64>,
    /// How much this machine varies on its own, as a fraction.
    ///
    /// Shown beside every timing number rather than kept as an internal
    /// threshold: a user comparing two numbers deserves to know how far apart
    /// they have to be before the comparison means anything.
    pub noise_floor: Option<f64>,
    pub noise_floor_samples: usize,
    pub measured: Vec<Measurement>,
    pub complete: bool,
}

impl SweepSummary {
    pub fn frontier(&self) -> impl Iterator<Item = &Measurement> {
        self.measured.iter().filter(|m| m.on_frontier)
    }

    pub fn rejected(&self) -> usize {
        self.measured.iter().filter(|m| !m.passed()).count()
    }

    /// The noise floor as the interface states it: "0.9%".
    pub fn noise_floor_label(&self) -> Option<String> {
        self.noise_floor.map(|floor| format!("{:.1}%", floor * 100.0))
    }

    /// The best size against the baseline, as a fraction.
    pub fn best_reduction(&self) -> Option<f64> {
        let baseline = self.baseline_bytes? as f64;
        if baseline == 0.0 {
            return None;
        }
        let best = self
            .measured
            .iter()
            .filter(|m| m.passed() && m.built)
            .filter_map(|m| m.size_bytes)
            .min()? as f64;
        Some((baseline - best) / baseline)
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

    /// Read back what a previous session on this target measured.
    ///
    /// Returns how many runs were adopted. This is `U13`'s import half, which
    /// had no entry point at all: an hour-long sweep died with the window.
    /// Findings arrive already revalidated — import re-runs the grounding
    /// check and re-applies the provenance ceiling — so a hand-edited
    /// artifact cannot inject a claim.
    fn restore_session(&self, target: &str) -> usize;

    /// Every sweep this session has run or restored.
    ///
    /// The Profile Lab's whole source. The engine held the measurements and
    /// exposed none of them, so the table, the scatter and the noise-floor
    /// line had nothing to read.
    fn sweeps(&self) -> Vec<SweepSummary>;

    /// The session's trust tier, so the title bar cannot show one the engine
    /// is not actually running at.
    fn trust_tier(&self) -> crate::config::TrustTier;

    /// Turn a frontier point into a `Cargo.toml` edit, without writing
    /// anything. Writing it is a separate, tier-gated [`Request::Apply`].
    fn propose_configuration(&self, configuration: &BuildConfiguration) -> crate::Result<Proposal>;
}
