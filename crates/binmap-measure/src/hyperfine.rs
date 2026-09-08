//! Timing through `hyperfine` (TOOLING §4.2).
//!
//! Shelled out rather than reimplemented, because it is the kind of tool §1
//! describes: years of specialised work — warmup runs, minimum run counts,
//! shell-spawn correction, outlier detection — producing a report rather than
//! a structure we query in a loop. We run it once per configuration.
//!
//! **The JSON export is parsed, never the human-readable table.** The table is
//! for people and it is free to change; the export is a contract.

use binmap_core::error::{Error, Result};
use binmap_core::evidence::{EvidenceId, ToolInvocation};
use binmap_core::tool::ToolRunner;
use serde::Deserialize;
use std::path::Path;
use std::time::Duration;

/// How hyperfine should be run.
#[derive(Debug, Clone)]
pub struct Timing {
    /// Runs to discard before measuring, so page cache and CPU frequency have
    /// settled. A cold first run is the single largest source of a false win.
    pub warmup: u32,
    /// The minimum number of measured runs.
    pub runs: u32,
}

impl Default for Timing {
    fn default() -> Self {
        Self { warmup: 3, runs: 10 }
    }
}

/// Only the fields we read. hyperfine's export carries more, and depending on
/// all of it would couple us to a schema we do not own.
#[derive(Debug, Deserialize)]
struct Export {
    results: Vec<ExportResult>,
}

#[derive(Debug, Deserialize)]
struct ExportResult {
    /// Every individual run, in seconds. Samples, not a mean: the mean is
    /// hyperfine's summary and the shape of the distribution is what decides
    /// whether a difference is real.
    times: Vec<f64>,
}

/// Whether hyperfine is on this machine.
///
/// Checked before it is needed, so the environment panel can say what is
/// missing and what to type rather than a sweep failing an hour in.
pub fn is_available(runner: &ToolRunner) -> bool {
    runner.is_available("hyperfine")
}

/// Time one command, returning every individual run.
///
/// The command is the user's own benchmark. We do not invent a workload on
/// their behalf, so a project without one simply has no runtime objective.
pub fn measure(
    runner: &ToolRunner,
    program: &str,
    arguments: &[String],
    working_directory: Option<&Path>,
    timing: &Timing,
) -> Result<(crate::timing::Samples, EvidenceId)> {
    let export = tempfile::Builder::new()
        .prefix("binmap-hyperfine-")
        .suffix(".json")
        .tempfile()
        .map_err(|source| Error::io("hyperfine export", source))?;

    let mut command = program.to_string();
    for argument in arguments {
        command.push(' ');
        command.push_str(argument);
    }

    let args = vec![
        "--warmup".to_string(),
        timing.warmup.to_string(),
        "--min-runs".to_string(),
        timing.runs.to_string(),
        "--export-json".to_string(),
        export.path().display().to_string(),
        // No progress bar or summary table: we read the export, and a
        // half-drawn table in an evidence record helps nobody.
        "--style".to_string(),
        "none".to_string(),
        command,
    ];

    let mut invocation = ToolInvocation::new("hyperfine", args);
    if let Some(directory) = working_directory {
        invocation = invocation.in_directory(directory.display().to_string());
    }
    let output = runner.run(invocation)?;

    if !output.succeeded() {
        return Err(Error::ToolUnavailable {
            tool: "hyperfine".into(),
            reason: output.stderr.lines().next().unwrap_or("it exited non-zero").trim().to_string(),
        });
    }

    let text = std::fs::read_to_string(export.path())
        .map_err(|source| Error::io(export.path(), source))?;
    let export: Export = serde_json::from_str(&text)
        .map_err(|source| Error::serialization("reading hyperfine's JSON export", source))?;

    let times = export
        .results
        .first()
        .map(|result| result.times.clone())
        .filter(|times| !times.is_empty())
        .ok_or_else(|| Error::Other("hyperfine measured nothing".into()))?;

    let durations = times.into_iter().map(Duration::from_secs_f64);
    Ok((crate::timing::Samples::new(durations), output.evidence))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_json_export_is_what_is_parsed_not_the_table() {
        // hyperfine's own export shape, trimmed to what we read. The
        // human-readable table is free to change; this is the contract.
        let export: Export = serde_json::from_str(
            r#"{"results":[{"command":"./app","mean":0.0121,"stddev":0.0004,
                 "times":[0.0119,0.0121,0.0123,0.0120]}]}"#,
        )
        .unwrap();
        assert_eq!(export.results[0].times.len(), 4);
    }

    #[test]
    fn extra_fields_in_the_export_do_not_break_the_read() {
        // Deliberately tolerant: hyperfine adds fields between versions, and a
        // sweep should not fail because it learned to report something new.
        let export: Export = serde_json::from_str(
            r#"{"results":[{"times":[0.5],"exit_codes":[0],"a_field_from_the_future":true}]}"#,
        )
        .unwrap();
        assert_eq!(export.results[0].times, vec![0.5]);
    }

    #[test]
    fn a_missing_hyperfine_names_itself_rather_than_failing_obscurely() {
        use binmap_core::evidence::EvidenceStore;
        let store = EvidenceStore::new();
        let runner = ToolRunner::new(store.clone(), ".");
        if is_available(&runner) {
            eprintln!("hyperfine is installed here; the missing-tool path is untested");
            return;
        }

        let error = measure(&runner, "true", &[], None, &Timing::default()).unwrap_err();
        match error {
            binmap_core::Error::ToolUnavailable { tool, .. } => assert_eq!(tool, "hyperfine"),
            other => panic!("a missing tool should name itself, got: {other}"),
        }
        // And the attempt is on the record, like every other.
        assert!(!store.is_empty());
    }
}
