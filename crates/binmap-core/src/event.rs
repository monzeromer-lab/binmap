//! The streaming event model, and cancellation.
//!
//! Findings appear as they are discovered, not in a batch at the end, and
//! every long run is cancellable and keeps what it has already measured
//! (`U0.5`). Analyses run on a background executor and emit these events; the
//! interface applies them to entities on the foreground.

use crate::finding::Finding;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Identifies one long-running piece of work, so the interface can attribute
/// progress and cancel the right thing.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RunId(pub String);

impl std::fmt::Display for RunId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Everything the engine tells the interface.
///
/// The interface never asks the engine a question and waits for the answer; it
/// applies these as they arrive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineEvent {
    /// Work has started. `total` is the number of steps where that is known
    /// up front — for a sweep it is the matrix cardinality.
    Started { run: RunId, description: String, total: Option<usize> },

    /// One step finished. `completed` counts steps, not percent; the interface
    /// decides how to render it.
    Progress { run: RunId, completed: usize, message: String },

    /// A finding was discovered. It is already grounded — a finding cannot
    /// exist otherwise — so the interface renders it without checking.
    Finding { run: RunId, finding: Box<Finding> },

    /// Work finished on its own.
    Finished { run: RunId, summary: String },

    /// The user cancelled. Everything measured so far is kept, and the run can
    /// be resumed (`F0.8`).
    Cancelled { run: RunId, completed: usize },

    /// Work failed. The message names the thing that failed.
    Failed { run: RunId, error: String },
}

impl EngineEvent {
    pub fn run(&self) -> &RunId {
        match self {
            EngineEvent::Started { run, .. }
            | EngineEvent::Progress { run, .. }
            | EngineEvent::Finding { run, .. }
            | EngineEvent::Finished { run, .. }
            | EngineEvent::Cancelled { run, .. }
            | EngineEvent::Failed { run, .. } => run,
        }
    }

    /// Whether this is the last event for its run. The interface stops showing
    /// progress on exactly these.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            EngineEvent::Finished { .. }
                | EngineEvent::Cancelled { .. }
                | EngineEvent::Failed { .. }
        )
    }
}

/// Where an analysis sends its events.
///
/// A plain callback rather than a channel, so the engine crates do not have to
/// agree on an async runtime with the interface.
pub trait EventSink: Send + Sync {
    fn emit(&self, event: EngineEvent);
}

impl<F: Fn(EngineEvent) + Send + Sync> EventSink for F {
    fn emit(&self, event: EngineEvent) {
        self(event)
    }
}

/// An event sink that drops everything. Used by the headless harness where
/// only the final artifact matters.
pub struct DiscardEvents;

impl EventSink for DiscardEvents {
    fn emit(&self, _event: EngineEvent) {}
}

/// A sink that keeps every event, for tests and for the eval harness.
#[derive(Debug, Default, Clone)]
pub struct RecordedEvents(Arc<std::sync::Mutex<Vec<EngineEvent>>>);

impl RecordedEvents {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn events(&self) -> Vec<EngineEvent> {
        self.0.lock().expect("recorded events poisoned").clone()
    }

    pub fn findings(&self) -> Vec<Finding> {
        self.events()
            .into_iter()
            .filter_map(|event| match event {
                EngineEvent::Finding { finding, .. } => Some(*finding),
                _ => None,
            })
            .collect()
    }
}

impl EventSink for RecordedEvents {
    fn emit(&self, event: EngineEvent) {
        self.0.lock().expect("recorded events poisoned").push(event);
    }
}

/// Cooperative cancellation.
///
/// Every long loop checks this between steps. Cancelling never discards what
/// has already been measured — that is the difference between cancelling a
/// sweep and losing an afternoon.
#[derive(Debug, Clone, Default)]
pub struct Cancellation(Arc<AtomicBool>);

impl Cancellation {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }

    /// The form that reads well at the top of a loop body.
    pub fn check(&self) -> crate::Result<()> {
        if self.is_cancelled() { Err(crate::Error::Cancelled) } else { Ok(()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_recorded_run_ends_in_exactly_one_terminal_event() {
        let sink = RecordedEvents::new();
        let run = RunId("run-1".into());
        sink.emit(EngineEvent::Started {
            run: run.clone(),
            description: "sweep".into(),
            total: Some(2),
        });
        sink.emit(EngineEvent::Progress {
            run: run.clone(),
            completed: 1,
            message: "built".into(),
        });
        sink.emit(EngineEvent::Cancelled { run: run.clone(), completed: 1 });

        let terminal = sink.events().iter().filter(|e| e.is_terminal()).count();
        assert_eq!(terminal, 1);
    }

    #[test]
    fn cancellation_is_observed_by_the_loop_not_by_the_caller() {
        let cancellation = Cancellation::new();
        assert!(cancellation.check().is_ok());
        cancellation.cancel();
        assert!(matches!(cancellation.check(), Err(crate::Error::Cancelled)));
    }
}
