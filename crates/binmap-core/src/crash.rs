//! What a crash looks like to the interface (`U2.1`–`U2.3`).
//!
//! In `binmap-core` for the reason given in `transcript.rs`: `§2.4` allows the
//! interface `binmap-core` and `binmap-session` and nothing else, so the Stack
//! Pane can only render a crash if its *shape* is here. The core reading, the
//! unwinding and the DWARF live in `binmap-crash`, which the interface may not
//! reach — rendering a stack costs no dependency on the crate that walked one.
//!
//! Everything a frame is uncertain about is carried rather than flattened.
//! `U2.3` asks for reconstructed values to be "marked when derived", and the
//! same rule applies a level up: a frame found by guessing at the frame
//! pointer, a binary that only probably matches the core, a line the compiler
//! inlined — each is a different kind of "less sure", and a pane that rendered
//! them identically would be the confident-wrong failure in visual form.

use serde::{Deserialize, Serialize};

/// How sure we are that a frame is where it says it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FrameConfidence {
    /// From the compiler's own unwind tables, or from the registers.
    Certain,
    /// From a frame-pointer walk, which optimised code frequently invalidates.
    Guessed,
}

impl FrameConfidence {
    pub fn label(self) -> &'static str {
        match self {
            FrameConfidence::Certain => "cfi",
            FrameConfidence::Guessed => "guessed",
        }
    }

    pub fn needs_a_badge(self) -> bool {
        matches!(self, FrameConfidence::Guessed)
    }
}

/// One line in the stack pane.
///
/// An inlined frame and a physical one are both entries, because that is how a
/// reader thinks about a stack — but `inlined` distinguishes them, because an
/// inlined frame never had a machine-level frame and showing it as though it
/// did misrepresents what happened.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StackEntry {
    /// Which physical frame this belongs to. Several entries share one when
    /// the compiler inlined.
    pub frame: usize,
    pub function: Option<String>,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub inlined: bool,
    pub confidence: FrameConfidence,
    /// The mapped file it is in, short form.
    pub module: Option<String>,
    pub address: u64,
}

impl StackEntry {
    /// Whether this is the reader's own code rather than a dependency's.
    ///
    /// The single most useful filter in a stack pane: twenty frames of
    /// `std::rt` around one frame of yours is the normal shape, and the one
    /// that matters is yours.
    pub fn is_probably_yours(&self, own: &[String]) -> bool {
        let Some(function) = &self.function else { return false };
        own.iter().any(|crate_name| function.starts_with(&format!("{crate_name}::")))
    }

    /// The one line a collapsed view shows.
    pub fn describe(&self) -> String {
        let function = self.function.as_deref().unwrap_or("<unknown>");
        match (&self.file, self.line) {
            (Some(file), Some(line)) => {
                let short = file.rsplit('/').next().unwrap_or(file);
                format!("{function} at {short}:{line}")
            }
            _ => format!("{function} (no line information)"),
        }
    }
}

/// How much of the stack could be trusted.
///
/// Named `Grounding` rather than `Provenance` deliberately: `Provenance` in
/// this codebase means how a *finding* was reached — Measured, Derived,
/// Inferred — and a second type with the same name and a different meaning in
/// the same crate is the kind of collision that survives review and confuses
/// everyone afterwards.
///
/// Shown once above the frames rather than repeated on each: a reader deciding
/// whether to act on a stack needs this before reading it, not after.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grounding {
    /// Whether the binary is proven to be the one that produced the core.
    pub binary_matches: bool,
    /// The sentence explaining that verdict.
    pub correspondence: String,
    /// Whether the load bias was corroborated by two derivations.
    pub bias_corroborated: bool,
    /// How many entries resolved to a source line.
    pub resolved: usize,
    pub total: usize,
    /// Why the walk stopped, when it did not reach the bottom.
    pub incomplete_because: Option<String>,
}

impl Grounding {
    pub fn coverage(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        self.resolved as f64 / self.total as f64
    }

    /// Whether this stack may be read without a caveat beside it.
    pub fn is_trustworthy(&self) -> bool {
        self.binary_matches && self.bias_corroborated && self.incomplete_because.is_none()
    }

    /// The caveats, in the order they matter.
    ///
    /// A list rather than a sentence, because several can apply at once and
    /// joining them into prose buries the first.
    pub fn caveats(&self) -> Vec<String> {
        let mut caveats = Vec::new();
        if !self.binary_matches {
            caveats.push(self.correspondence.clone());
        }
        if !self.bias_corroborated {
            caveats.push(
                "The load bias could not be cross-checked, so these addresses rest on one \
                 derivation rather than two."
                    .into(),
            );
        }
        if let Some(reason) = &self.incomplete_because {
            caveats.push(format!("The stack is incomplete: {reason}"));
        }
        if self.coverage() < 0.5 && self.total > 0 {
            caveats.push(format!(
                "Only {:.0}% of these frames resolved to a source line — this binary was \
                 probably built without full debug information.",
                self.coverage() * 100.0
            ));
        }
        caveats
    }
}

/// Everything the Stack Pane renders.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrashReport {
    /// What kind of death, in the product's own words rather than a signal
    /// number.
    pub title: String,
    /// The first question this kind of crash asks.
    pub what_to_look_at: String,
    /// Innermost first, as every debugger shows them.
    pub entries: Vec<StackEntry>,
    pub provenance: Grounding,
    /// The process, for the header.
    pub pid: i32,
    pub signal: i32,
}

impl CrashReport {
    /// The first entry that is the reader's own code.
    ///
    /// What a stack pane should select on open: the innermost frame of a
    /// panic is `pthread_kill`, and nobody opened a debugger to look at that.
    pub fn first_of_yours(&self, own: &[String]) -> Option<usize> {
        self.entries.iter().position(|entry| entry.is_probably_yours(own))
    }

    /// How many physical frames there are, as distinct from entries.
    pub fn frame_count(&self) -> usize {
        self.entries.iter().map(|entry| entry.frame).max().map(|last| last + 1).unwrap_or(0)
    }

    pub fn inlined_count(&self) -> usize {
        self.entries.iter().filter(|entry| entry.inlined).count()
    }

    /// The summary line above the frames.
    pub fn describe(&self) -> String {
        let mut line = format!("{} frames", self.frame_count());
        if self.inlined_count() > 0 {
            line.push_str(&format!(", {} of them inlined", self.inlined_count()));
        }
        line.push_str(&format!(", {:.0}% resolved to a line", self.provenance.coverage() * 100.0));
        line
    }
}
