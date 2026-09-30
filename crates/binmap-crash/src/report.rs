//! Turning an analysis into what the interface renders (`U2.1`–`U2.3`).
//!
//! `binmap_core::crash::CrashReport` is the shape; this fills it. The
//! translation lives here rather than in the interface because everything it
//! decides is a judgement about the *analysis* — whether a frame was guessed,
//! whether the stack is complete, whether the binary is proven — and those are
//! not the interface's to make.

use crate::bias::LoadBias;
use crate::correspondence::Correspondence;
use crate::dump::Thread;
use crate::symbolize::Resolved;
use crate::unwind::{Method, Stack};
use binmap_core::crash::{CrashReport, FrameConfidence, Grounding, StackEntry};

/// Assemble a report from everything the analysis produced.
pub fn assemble(
    thread: &Thread,
    stack: &Stack,
    resolved: &[Resolved],
    crash: &crate::classify::Crash,
    correspondence: &Correspondence,
    bias: &LoadBias,
) -> CrashReport {
    let mut entries = Vec::new();

    for (index, (frame, locations)) in stack.frames.iter().zip(resolved).enumerate() {
        let confidence = match frame.method {
            Method::FramePointer => FrameConfidence::Guessed,
            Method::Cfi | Method::Registers => FrameConfidence::Certain,
        };
        let module =
            frame.module.as_deref().and_then(|path| path.rsplit('/').next()).map(str::to_string);

        if locations.is_empty() {
            // A frame with nothing behind it is still a frame. Omitting it
            // would silently renumber everything below and make the stack
            // shorter than it was.
            entries.push(StackEntry {
                frame: index,
                function: None,
                file: None,
                line: None,
                inlined: false,
                confidence,
                module,
                address: frame.runtime_address,
            });
            continue;
        }

        for location in &locations.locations {
            entries.push(StackEntry {
                frame: index,
                function: location.function.clone(),
                file: location.file.clone(),
                line: location.line,
                inlined: location.inlined,
                confidence,
                module: module.clone(),
                address: frame.runtime_address,
            });
        }
    }

    let resolved_count = entries.iter().filter(|entry| entry.line.is_some()).count();
    let provenance = Grounding {
        binary_matches: matches!(correspondence, Correspondence::Certain { .. }),
        correspondence: correspondence.describe(),
        bias_corroborated: !bias.derivation.needs_a_caveat(),
        resolved: resolved_count,
        total: entries.len(),
        // Only recorded when the walk did *not* reach the bottom: a complete
        // stack has no caveat to make.
        incomplete_because: (!stack.stopped_because.is_complete())
            .then(|| stack.stopped_because.describe()),
    };

    CrashReport {
        title: crash.title(),
        what_to_look_at: crash.what_to_look_at().to_string(),
        entries,
        provenance,
        pid: thread.pid,
        signal: thread.signal,
    }
}
