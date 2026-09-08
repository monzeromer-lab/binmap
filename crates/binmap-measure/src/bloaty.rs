//! `bloaty` as a correctness oracle (TOOLING §4.1).
//!
//! Not a source of truth for the product — attribution to a *generic origin*
//! across instantiations is the differentiator, and no general-purpose tool
//! does it, so parsing bloaty's text to re-derive structure would be worse
//! than reading the symbol table directly.
//!
//! What bloaty is for is disagreement. It has mature section and symbol
//! attribution, and if our totals differ from its by more than a small margin,
//! **we are wrong**. TOOLING asks for that to be a test, so it is one.
//!
//! Its role is cross-check only, never load-bearing: a machine without bloaty
//! runs every analysis unchanged and simply does not get the second opinion.

use binmap_core::artifact::ArtifactSize;
use binmap_core::error::Result;
use binmap_core::evidence::{EvidenceId, ToolInvocation};
use binmap_core::tool::ToolRunner;
use std::collections::BTreeMap;
use std::path::Path;

pub fn is_available(runner: &ToolRunner) -> bool {
    runner.is_available("bloaty")
}

/// Bytes per section, as bloaty sees them.
///
/// `-d sections --csv` gives a stable machine-readable shape; the pretty table
/// is for people.
pub fn sections(
    runner: &ToolRunner,
    artifact: &Path,
) -> Result<(BTreeMap<String, u64>, EvidenceId)> {
    let output = runner.run(ToolInvocation::new(
        "bloaty",
        ["-d", "sections", "-n", "0", "--csv", &artifact.display().to_string()],
    ))?;
    Ok((parse_csv(&output.stdout), output.evidence))
}

/// bloaty's CSV is `sections,vmsize,filesize` with a header row.
fn parse_csv(text: &str) -> BTreeMap<String, u64> {
    let mut sections = BTreeMap::new();
    for line in text.lines().skip(1) {
        let mut fields = line.rsplitn(3, ',');
        let (Some(file_size), Some(_vm_size), Some(name)) =
            (fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        let Ok(bytes) = file_size.trim().parse::<u64>() else { continue };
        sections.insert(name.trim().trim_matches('"').to_string(), bytes);
    }
    sections
}

/// How far our reading is from bloaty's, as a fraction of the file.
///
/// Returns `None` when bloaty is not installed — an absent second opinion is
/// not a disagreement.
pub fn disagreement(runner: &ToolRunner, size: &ArtifactSize) -> Result<Option<(f64, EvidenceId)>> {
    if !is_available(runner) {
        return Ok(None);
    }
    let (theirs, evidence) = sections(runner, &size.path)?;
    // bloaty reports its own synthetic rows for headers and padding, which we
    // do not model; compare only the sections we both name.
    let mut ours_total = 0u64;
    let mut theirs_total = 0u64;
    for section in size.sections.iter().filter(|section| section.occupies_file) {
        if let Some(bytes) = theirs.get(&section.name) {
            ours_total += section.bytes;
            theirs_total += bytes;
        }
    }
    if theirs_total == 0 {
        return Ok(None);
    }
    let difference = (ours_total as f64 - theirs_total as f64).abs() / theirs_total as f64;
    Ok(Some((difference, evidence)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use binmap_core::evidence::EvidenceStore;

    #[test]
    fn the_csv_is_parsed_and_the_header_is_not_a_section() {
        let csv = "sections,vmsize,filesize\n.text,102400,102400\n.rodata,20480,20480\n[Unmapped],0,3072\n";
        let sections = parse_csv(csv);
        assert_eq!(sections.get(".text"), Some(&102400));
        assert_eq!(sections.get(".rodata"), Some(&20480));
        assert!(!sections.contains_key("sections"), "the header is not a section");
    }

    #[test]
    fn a_section_name_containing_a_comma_survives_the_split() {
        // Splitting from the right is why: only the two numeric columns are
        // fixed, and a section name is whatever the linker called it.
        let sections = parse_csv("sections,vmsize,filesize\n.odd,name,10,20\n");
        assert_eq!(sections.get(".odd,name"), Some(&20));
    }

    /// TOOLING §4.1: "If your total attributed bytes disagree with bloaty's by
    /// more than a small margin, your attribution is wrong. Make that a test."
    ///
    /// Skipped rather than failed where bloaty is absent — an oracle that is
    /// not installed cannot contradict us, and the cross-check is explicitly
    /// not load-bearing.
    #[test]
    fn our_section_reading_agrees_with_bloaty_where_bloaty_is_installed() {
        let store = EvidenceStore::new();
        let runner = ToolRunner::new(store, ".");
        if !is_available(&runner) {
            eprintln!("bloaty is not installed; the cross-check has nothing to compare against");
            return;
        }

        let path = std::env::current_exe().unwrap();
        let (size, _) = crate::size::measure_size(&runner, &path).unwrap();
        let Some((difference, _)) = disagreement(&runner, &size).unwrap() else {
            return;
        };
        assert!(
            difference < 0.01,
            "our section attribution differs from bloaty's by {:.2}% — we are wrong",
            difference * 100.0
        );
    }
}
