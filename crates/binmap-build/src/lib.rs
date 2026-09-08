//! Finding the user's project, building it under a configuration, and sweeping
//! the matrix over it.
//!
//! Two constraints shape everything here. Nothing writes to the user's
//! `Cargo.toml` — a configuration is put into effect with `--config` arguments,
//! and applying one for real is a separate, tier-gated act. And nothing writes
//! to the user's target directory, so a sweep never costs them the incremental
//! cache they will want back the moment it finishes (`F0.8`).

pub mod cargo;
pub mod engine;
pub mod environment;
pub mod project;
pub mod sweep;

pub use cargo::CargoBuildSystem;
pub use engine::BinmapEngine;
pub use environment::probe_all;
pub use project::{ProjectKind, discover};
pub use sweep::{Sweep, SweepOptions, SweepState};
