//! One error type for the engine, and the rule that every failure names the
//! thing that failed rather than the layer that noticed.

use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no cargo project at {0}")]
    NoProject(PathBuf),

    #[error("{path} is not readable: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("{0}")]
    Config(String),

    /// A finding was constructed citing no evidence. This is the grounding
    /// invariant (R2) and it is a bug in the caller, never a model failure.
    #[error("finding `{0}` cites no evidence")]
    Ungrounded(String),

    /// A finding cited an evidence id the store never issued. In Mode B this is
    /// the airlock rejecting an external claim; the rejection is recorded, not
    /// swallowed.
    #[error("finding `{finding}` cites evidence `{evidence}`, which was never issued")]
    UnknownEvidence { finding: String, evidence: String },

    #[error("`{tool}` is not available: {reason}")]
    ToolUnavailable { tool: String, reason: String },

    #[error("{action} requires trust tier {required}; the session is at {current}")]
    TierTooLow {
        action: String,
        required: crate::config::TrustTier,
        current: crate::config::TrustTier,
    },

    #[error("the target does not support {0}")]
    Unsupported(crate::capability::Capability),

    #[error("cancelled")]
    Cancelled,

    #[error("{context}: {source}")]
    Serialization {
        context: String,
        #[source]
        source: serde_json::Error,
    },

    #[error("{0}")]
    Other(String),
}

impl Error {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Error::Io { path: path.into(), source }
    }

    pub fn serialization(context: impl Into<String>, source: serde_json::Error) -> Self {
        Error::Serialization { context: context.into(), source }
    }
}
