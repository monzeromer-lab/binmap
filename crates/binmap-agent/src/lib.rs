//! The model layer: the tool registry, the loop that uses it, and the gate
//! that decides what a model is allowed to have concluded.
//!
//! §2.4 draws the dependency the wrong way round from what most people
//! expect: **the model may not lead an analysis.** This crate may depend on
//! the analysis crates; they may never depend on it. An analysis that wants
//! intelligence emits a structured question, and orchestration decides whether
//! to answer it with a model.
//!
//! In Phase 0 the backend is null: the registry and the evidence store exist,
//! and nothing calls a model. That is not a stub — it is the configuration §7
//! asks for, and the suite runs under it, which is what keeps the
//! deterministic core deterministic.

pub mod gate;
pub mod registry;

pub use gate::{Claim, Gate, Rejection};
pub use registry::{Registry, Tool, ToolCall, ToolOutcome};
