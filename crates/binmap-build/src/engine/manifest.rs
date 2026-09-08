//! Turning a configuration into a `Cargo.toml` edit, and showing the edit
//! before anything is written.
//!
//! Nothing in this module writes. It produces the new text and the diff, and
//! the apply dialog states what it will write; writing is a separate act at a
//! tier that permits it (`U9`, `N7`).

use binmap_core::configuration::BuildConfiguration;
use std::path::Path;

/// Rewrite a manifest so `[profile.release]` carries this configuration.
///
/// Keys the configuration sets are replaced in place, keeping their position
/// and any comment on the line after them; keys it does not set are left
/// exactly alone. A user who reads the diff should see their own file with a
/// few values changed, not a section we regenerated.
pub fn with_release_profile(manifest: &str, configuration: &BuildConfiguration) -> String {
    let settings = configuration.settings();
    if settings.is_empty() {
        return manifest.to_string();
    }
    let wanted: Vec<(String, String)> = settings
        .iter()
        .map(|(axis, _)| (*axis).to_string())
        .zip(toml_values(configuration))
        .collect();

    let mut lines: Vec<String> = manifest.lines().map(str::to_string).collect();
    let section = find_section(&lines, "[profile.release]");

    let Some((start, end)) = section else {
        // No release profile at all: append one rather than guess where it
        // should have gone.
        let mut out = manifest.trim_end().to_string();
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str("[profile.release]\n");
        for (key, value) in &wanted {
            out.push_str(&format!("{key} = {value}\n"));
        }
        return out;
    };

    let mut still_needed: Vec<&(String, String)> = wanted.iter().collect();
    for line in lines.iter_mut().take(end).skip(start + 1) {
        let Some(equals) = line.find('=') else { continue };
        let key = line[..equals].trim().trim_matches('"').to_string();
        if let Some(position) = still_needed.iter().position(|(wanted, _)| *wanted == key) {
            let (key, value) = still_needed.remove(position);
            *line = format!("{key} = {value}");
        }
    }

    // Anything the profile did not already mention is added at the end of the
    // section, in the order the axes are declared.
    let additions: Vec<String> =
        still_needed.iter().map(|(key, value)| format!("{key} = {value}")).collect();
    if !additions.is_empty() {
        let mut insert_at = end;
        while insert_at > start + 1 && lines[insert_at - 1].trim().is_empty() {
            insert_at -= 1;
        }
        for (offset, addition) in additions.into_iter().enumerate() {
            lines.insert(insert_at + offset, addition);
        }
    }

    let mut out = lines.join("\n");
    if manifest.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// The TOML rendering of each axis, in the same order as
/// [`BuildConfiguration::settings`].
fn toml_values(configuration: &BuildConfiguration) -> Vec<String> {
    let mut values = Vec::new();
    if let Some(v) = configuration.opt_level {
        values.push(v.as_toml().to_string());
    }
    if let Some(v) = configuration.lto {
        values.push(v.as_toml().to_string());
    }
    if let Some(v) = configuration.codegen_units {
        values.push(v.to_string());
    }
    if let Some(v) = configuration.panic {
        values.push(v.as_toml().to_string());
    }
    if let Some(v) = configuration.strip {
        values.push(v.as_toml().to_string());
    }
    if let Some(v) = configuration.debug {
        values.push(v.as_toml().to_string());
    }
    if let Some(v) = configuration.overflow_checks {
        values.push(v.to_string());
    }
    if let Some(v) = configuration.build_std {
        values.push(format!("\"{}\"", v.as_toml()));
    }
    if let Some(v) = &configuration.target_cpu {
        values.push(format!("\"{v}\""));
    }
    values
}

/// The half-open line range of a TOML section, from its header to the line
/// before the next header.
fn find_section(lines: &[String], header: &str) -> Option<(usize, usize)> {
    let start = lines.iter().position(|line| line.trim() == header)?;
    let end = lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find(|(_, line)| line.trim_start().starts_with('['))
        .map(|(index, _)| index)
        .unwrap_or(lines.len());
    Some((start, end))
}

/// A unified diff, in the form the proposal tab renders.
///
/// Written here rather than pulled in as a dependency because the input is one
/// small file and the output has one reader — and a diff nobody can read is
/// worse than no diff.
pub fn unified_diff(path: &Path, before: &str, after: &str) -> String {
    if before == after {
        return String::new();
    }
    let before: Vec<&str> = before.lines().collect();
    let after: Vec<&str> = after.lines().collect();

    let mut body = Vec::new();
    let mut changes = Vec::new();
    let common = before.len().min(after.len());
    for index in 0..common.max(before.len()).max(after.len()) {
        match (before.get(index), after.get(index)) {
            (Some(old), Some(new)) if old == new => body.push(format!(" {old}")),
            (Some(old), Some(new)) => {
                body.push(format!("-{old}"));
                body.push(format!("+{new}"));
                changes.push(index);
            }
            (Some(old), None) => {
                body.push(format!("-{old}"));
                changes.push(index);
            }
            (None, Some(new)) => {
                body.push(format!("+{new}"));
                changes.push(index);
            }
            (None, None) => break,
        }
    }

    // Three lines of context around the changed region, as a reader expects.
    let (first, last) = match (changes.first(), changes.last()) {
        (Some(first), Some(last)) => (*first, *last),
        _ => return String::new(),
    };
    let from = first.saturating_sub(3);
    let to = (last + 4).min(body.len());

    let display = path.display();
    let mut out = format!("--- a/{display}\n+++ b/{display}\n");
    out.push_str(&format!("@@ -{},{} +{},{} @@\n", from + 1, to - from, from + 1, to - from));
    for line in &body[from..to] {
        out.push_str(line);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use binmap_core::config::{Lto, OptLevel, Strip};

    fn configuration() -> BuildConfiguration {
        BuildConfiguration {
            opt_level: Some(OptLevel::Size),
            lto: Some(Lto::Fat),
            strip: Some(Strip::Symbols),
            ..BuildConfiguration::default()
        }
    }

    #[test]
    fn an_existing_key_is_changed_in_place_rather_than_appended() {
        let manifest = "[package]\nname = \"app\"\n\n[profile.release]\nopt-level = 3\nlto = false\n";
        let updated = with_release_profile(manifest, &configuration());
        assert_eq!(
            updated,
            "[package]\nname = \"app\"\n\n[profile.release]\nopt-level = \"s\"\nlto = \"fat\"\nstrip = \"symbols\"\n"
        );
    }

    #[test]
    fn a_manifest_with_no_release_profile_gains_one() {
        let manifest = "[package]\nname = \"app\"\n";
        let updated = with_release_profile(manifest, &configuration());
        assert!(updated.contains("[profile.release]\nopt-level = \"s\"\nlto = \"fat\"\nstrip = \"symbols\"\n"));
        assert!(updated.starts_with("[package]\nname = \"app\"\n"));
    }

    #[test]
    fn keys_the_configuration_does_not_set_are_left_exactly_alone() {
        let manifest =
            "[profile.release]\nopt-level = 3\nincremental = true\ndebug-assertions = false\n";
        let updated = with_release_profile(manifest, &configuration());
        assert!(updated.contains("incremental = true"));
        assert!(updated.contains("debug-assertions = false"));
    }

    #[test]
    fn a_later_section_is_not_swallowed_by_the_profile() {
        let manifest = "[profile.release]\nopt-level = 3\n\n[dependencies]\nserde = \"1\"\n";
        let updated = with_release_profile(manifest, &configuration());
        assert!(updated.contains("[dependencies]\nserde = \"1\""));
        // The additions land inside the profile, before the blank line.
        let profile = updated.split("[dependencies]").next().unwrap();
        assert!(profile.contains("lto = \"fat\""), "{profile}");
    }

    #[test]
    fn a_configuration_that_sets_nothing_produces_no_edit_and_no_diff() {
        let manifest = "[profile.release]\nopt-level = 3\n";
        let updated = with_release_profile(manifest, &BuildConfiguration::default());
        assert_eq!(updated, manifest);
        assert_eq!(unified_diff(Path::new("Cargo.toml"), manifest, &updated), "");
    }

    #[test]
    fn the_diff_names_the_file_and_shows_both_sides() {
        let manifest = "[profile.release]\nopt-level = 3\nlto = false\n";
        let updated = with_release_profile(manifest, &configuration());
        let diff = unified_diff(Path::new("Cargo.toml"), manifest, &updated);

        assert!(diff.starts_with("--- a/Cargo.toml\n+++ b/Cargo.toml\n@@"), "{diff}");
        assert!(diff.contains("-opt-level = 3"), "{diff}");
        assert!(diff.contains("+opt-level = \"s\""), "{diff}");
        assert!(diff.contains("+strip = \"symbols\""), "{diff}");
    }
}
