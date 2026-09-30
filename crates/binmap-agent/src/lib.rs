//! The model layer: the tool registry, the loop that uses it, and the gate
//! that decides what a model is allowed to have concluded.
//!
//! §2.4 draws the dependency the wrong way round from what most people
//! expect: **the model may not lead an analysis.** This crate may depend on
//! the analysis crates; they may never depend on it. An analysis that wants
//! intelligence emits a structured question, and orchestration decides whether
//! to answer it with a model.
//!
//! The null backend is not a stub, it is a configuration `§6.1` lists beside
//! the real ones, and the entire test suite runs on it. That is what keeps the
//! deterministic core deterministic: a test that could reach a model would not
//! be a test. Phase 1 adds the OpenAI-compatible backend, which covers every
//! provider in the table except Claude, and the loop that drives it.

pub mod acp;
pub mod anthropic;
pub mod backend;
pub mod gate;
pub mod mcp;
pub mod native;
pub mod openai;
pub mod provider;
pub mod registry;

pub use backend::{
    CompletionRequest, CompletionResponse, Message, ModelBackend, NullBackend, Role, Usage,
};
/// Re-exported from `binmap-core`, where the shape has to live so the
/// interface can render it without depending on this crate (`§2.4`).
pub use binmap_core::transcript::{self, Origin, StopReason, Transcript, TranscriptEvent};
pub use gate::{Claim, Gate, Rejection};
pub use native::{AgentConfig, Session, SessionOutcome, Spend};
pub use openai::{HttpTransport, OpenAiCompatibleBackend, UnavailableTransport, UreqTransport};
pub use provider::{ApiShape, Capabilities, PROVIDERS, ProviderSpec, Quirks};
pub use registry::{Registry, Tool, ToolCall, ToolOutcome};
