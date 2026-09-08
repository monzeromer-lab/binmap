//! Measuring one built artifact.

use binmap_core::artifact::ArtifactSize;
use binmap_core::error::{Error, Result};
use binmap_core::evidence::{EvidenceId, ToolInvocation};
use binmap_core::tool::ToolRunner;
use std::path::Path;

/// Measure an artifact, recording the measurement as evidence.
///
/// The total is the file's size on disk, because that is the number the user's
/// disk and their release page both report. The section table is supporting
/// detail, and where the two disagree — headers, padding, alignment — the
/// difference is visible rather than reconciled silently.
pub fn measure_size(runner: &ToolRunner, artifact: &Path) -> Result<(ArtifactSize, EvidenceId)> {
    let invocation = ToolInvocation::new("binmap:measure-size", [artifact.display().to_string()]);
    let pending = runner.store().begin(invocation);

    let metadata = match std::fs::metadata(artifact) {
        Ok(metadata) => metadata,
        Err(source) => {
            // Complete the record rather than dropping it. A dropped
            // PendingEvidence stays in `incomplete()` for the life of the
            // session and reads as a tool that hung — which is a different
            // and more alarming thing than a file that was not there.
            runner.store().complete(pending, format!("failed to read: {source}"), -1);
            return Err(Error::io(artifact, source));
        }
    };
    let total_bytes = metadata.len();
    let sections = match crate::sections::read(artifact) {
        Ok(sections) => sections,
        Err(error) => {
            runner.store().complete(pending, format!("failed to read sections: {error}"), -1);
            return Err(error);
        }
    };

    let mut report = format!("{}\ntotal {total_bytes}\n", artifact.display());
    for section in &sections {
        let stored = if section.occupies_file { "file" } else { "memory-only" };
        report.push_str(&format!("{:<24} {:>12} {stored}\n", section.name, section.bytes));
    }
    let evidence = runner.store().complete(pending, report, 0);

    let mut size = ArtifactSize::new(artifact, total_bytes);
    size.sections = sections;
    Ok((size, evidence))
}

/// The difference between what the section table accounts for and what the
/// file actually weighs: headers, padding and alignment.
///
/// Reported rather than hidden, because an unexplained gap in a size report is
/// how a user stops trusting the whole report.
pub fn unaccounted_bytes(size: &ArtifactSize) -> i64 {
    size.total_bytes as i64 - size.section_bytes() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use binmap_core::evidence::EvidenceStore;

    #[test]
    fn measuring_records_evidence_a_user_could_check_by_hand() {
        let store = EvidenceStore::new();
        let runner = ToolRunner::new(store.clone(), ".");
        let path = std::env::current_exe().unwrap();

        let (size, evidence) = measure_size(&runner, &path).unwrap();
        assert_eq!(size.total_bytes, std::fs::metadata(&path).unwrap().len());
        assert!(store.issued(&evidence));

        let record = store.get(&evidence).unwrap();
        assert!(record.output.contains(&format!("total {}", size.total_bytes)));
        assert!(record.digest_matches());
    }

    #[test]
    fn the_gap_between_sections_and_the_file_is_reported_not_hidden() {
        let store = EvidenceStore::new();
        let runner = ToolRunner::new(store, ".");
        let (size, _) = measure_size(&runner, &std::env::current_exe().unwrap()).unwrap();
        // Headers and alignment mean the sections never quite add up, and the
        // shortfall is a number we can state rather than a discrepancy.
        assert!(unaccounted_bytes(&size) >= 0);
    }

    #[test]
    fn measuring_something_that_is_not_there_fails_with_the_path() {
        let store = EvidenceStore::new();
        let runner = ToolRunner::new(store.clone(), ".");
        let error = measure_size(&runner, Path::new("/nonexistent/artifact")).unwrap_err();
        assert!(error.to_string().contains("/nonexistent/artifact"));

        // And leaves no phantom behind. A dropped PendingEvidence stays in
        // `incomplete()` for the life of the session and reads as a tool that
        // hung, which is more alarming than what actually happened.
        assert!(store.incomplete().is_empty(), "a failed measurement left a phantom record");
        assert_eq!(store.len(), 1, "the attempt is still on the record");
    }
}
