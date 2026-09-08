//! `binmap.json`: the stable contract, versioned from the first release.
//!
//! The artifact carries findings, evidence, gate results and target metadata,
//! and it is the whole of what one session knows. Two things make it worth
//! having rather than merely tidy:
//!
//! - **Import re-runs the gates that construction ran.** A finding arriving
//!   from a file is not more trusted than one arriving from an agent: its
//!   evidence digests are checked, and a finding citing evidence that did not
//!   survive that check is refused and named.
//! - **Export redacts.** A session artifact is something a user sends to a
//!   colleague or attaches to a bug report, and it is full of absolute paths
//!   from their home directory and whatever their build printed. The redaction
//!   pass runs on export and reports what it changed (`U13`).
//!
//! This crate depends on `binmap-core` and nothing else, which is what lets the
//! interface depend on it without gaining the ability to compute (§2.4).

pub mod artifact;
pub mod redact;
pub mod store;

pub use artifact::{ImportOutcome, RunRecord, SessionArtifact, TargetMetadata, SCHEMA_VERSION};
pub use redact::{Redaction, RedactionReport};
pub use store::SessionStore;
