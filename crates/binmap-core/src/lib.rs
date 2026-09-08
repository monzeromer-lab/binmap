//! The vocabulary the whole product is built from.
//!
//! Nothing here computes anything. It defines the [`Finding`] model every
//! analysis produces and every view renders, the [`Evidence`] that grounds it,
//! the backend traits artifact-specific logic is confined to, and the facade
//! the interface reaches analyses through.
//!
//! Two invariants live in this crate and are enforced by construction:
//!
//! - A [`Finding`] cannot exist without at least one piece of evidence.
//! - An [`EvidenceId`] can only be minted by an [`EvidenceStore`], before the
//!   tool output it describes is returned to anyone.

pub mod artifact;
pub mod capability;
pub mod config;
pub mod configuration;
pub mod error;
pub mod event;
pub mod evidence;
pub mod facade;
pub mod finding;
pub mod gate;
pub mod location;
pub mod tool;
pub mod traits;

pub use error::{Error, Result};
pub use evidence::{Evidence, EvidenceId, EvidenceStore, ToolInvocation};
pub use finding::{Confidence, Finding, FindingKind, Impact, Provenance};
pub use location::Location;

pub use artifact::{ArtifactSize, Section};
pub use capability::{Capabilities, Capability};
pub use config::{ProjectConfig, SweepMatrix, TrustTier};
pub use configuration::BuildConfiguration;
pub use event::{Cancellation, EngineEvent, EventSink, RunId};
pub use facade::{Engine, Measurement, Probe, ProbeStatus, Proposal, Request, SweepSummary};
pub use finding::FindingDraft;
pub use gate::{Gate, GateOutcome, GateResult, VerificationReport};
pub use tool::{ToolRegistry, ToolRunner, ToolSpec};
pub use traits::{
    ArtifactReader, BuildSystem, MeasurementSource, Symbolizer, Target, TargetFamily,
};
