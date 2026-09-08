//! Turning a configuration into a `Cargo.toml` edit, and showing the edit
//! before anything is written.
//!
//! Nothing in this module writes. It produces the new text and the diff, and
//! the apply dialog states what it will write; writing is a separate act at a
//! tier that permits it (`U9`, `N7`).
//!
//! `toml_edit` does the editing because it is format-preserving: comments,
//! ordering, spacing and the user's own quoting style all survive, so the diff
//! shows their file with a few values changed rather than a section we
//! regenerated. A line-based editor gets this wrong on inline tables, on
//! `[profile.release.package.*]`, and on any manifest whose author had
//! opinions about layout.

use binmap_core::configuration::BuildConfiguration;
use similar::TextDiff;
use std::path::Path;
use toml_edit::{DocumentMut, Item, Value};

/// Rewrite a manifest so `[profile.release]` carries this configuration.
///
/// Keys the configuration sets are replaced; keys it does not set are left
/// exactly alone. Returns the manifest unchanged when the configuration sets
/// nothing, and when the manifest cannot be parsed — refusing to guess is
/// better than writing a broken file.
pub fn with_release_profile(manifest: &str, configuration: &BuildConfiguration) -> String {
    let settings = release_values(configuration);
    if settings.is_empty() {
        return manifest.to_string();
    }

    let Ok(mut document) = manifest.parse::<DocumentMut>() else {
        return manifest.to_string();
    };

    let Some(profiles) =
        document.entry("profile").or_insert(Item::Table(Default::default())).as_table_mut()
    else {
        // `profile` exists and is not a table: the manifest is not shaped the
        // way cargo reads it, and guessing would write something worse.
        return manifest.to_string();
    };
    // `[profile.release]`, not `profile = { release = { … } }`.
    profiles.set_implicit(true);

    let Some(profile) =
        profiles.entry("release").or_insert(Item::Table(Default::default())).as_table_mut()
    else {
        return manifest.to_string();
    };

    for (key, value) in settings {
        profile[key] = Item::Value(value);
    }

    document.to_string()
}

/// Each axis as the TOML value cargo reads, paired with its key.
///
/// Typed values rather than rendered strings: `opt-level = 3` is an integer
/// and `opt-level = "s"` is a string, and the distinction is cargo's, not a
/// formatting preference.
fn release_values(configuration: &BuildConfiguration) -> Vec<(&'static str, Value)> {
    use binmap_core::config::OptLevel;

    let mut values: Vec<(&'static str, Value)> = Vec::new();
    if let Some(v) = configuration.opt_level {
        values.push((
            "opt-level",
            match v {
                OptLevel::Zero => Value::from(0),
                OptLevel::One => Value::from(1),
                OptLevel::Two => Value::from(2),
                OptLevel::Three => Value::from(3),
                other => Value::from(other.to_string()),
            },
        ));
    }
    if let Some(v) = configuration.lto {
        values.push((
            "lto",
            match v {
                binmap_core::config::Lto::Off => Value::from(false),
                other => Value::from(other.to_string()),
            },
        ));
    }
    if let Some(v) = configuration.codegen_units {
        values.push(("codegen-units", Value::from(v as i64)));
    }
    if let Some(v) = configuration.panic {
        values.push(("panic", Value::from(v.to_string())));
    }
    if let Some(v) = configuration.strip {
        values.push(("strip", Value::from(v.to_string())));
    }
    if let Some(v) = configuration.debug {
        values.push((
            "debug",
            match v {
                binmap_core::config::DebugInfo::None => Value::from(0),
                binmap_core::config::DebugInfo::Limited => Value::from(1),
                binmap_core::config::DebugInfo::Full => Value::from(2),
                other => Value::from(other.to_string()),
            },
        ));
    }
    if let Some(v) = configuration.overflow_checks {
        values.push(("overflow-checks", Value::from(v)));
    }
    values
}

/// A unified diff, as the proposal tab renders it.
pub fn unified_diff(path: &Path, before: &str, after: &str) -> String {
    if before == after {
        return String::new();
    }
    let display = path.display().to_string();
    TextDiff::from_lines(before, after)
        .unified_diff()
        .context_radius(3)
        .header(&format!("a/{display}"), &format!("b/{display}"))
        .to_string()
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
    fn an_existing_key_is_changed_in_place() {
        let manifest = "[package]\nname = \"app\"\n\n[profile.release]\nopt-level = 3\nlto = false\n";
        let updated = with_release_profile(manifest, &configuration());
        assert!(updated.contains("opt-level = \"s\""), "{updated}");
        assert!(updated.contains("lto = \"fat\""), "{updated}");
        assert!(updated.contains("strip = \"symbols\""), "{updated}");
        assert!(updated.contains("name = \"app\""));
    }

    #[test]
    fn comments_and_the_users_own_layout_survive_the_edit() {
        // The whole reason for toml_edit. A line-based editor loses these, and
        // a diff full of incidental churn is a diff nobody reads.
        let manifest = "\
[profile.release]
# We turned this down in 2024 after the incident.
opt-level  =  3
incremental = true   # keep, the CI cache depends on it
";
        let updated = with_release_profile(manifest, &configuration());
        assert!(updated.contains("# We turned this down in 2024 after the incident."));
        assert!(updated.contains("# keep, the CI cache depends on it"));
        assert!(updated.contains("incremental = true"));
    }

    #[test]
    fn types_are_cargos_not_ours() {
        // opt-level = 3 is an integer; opt-level = "s" is a string. lto = false
        // is a boolean, not the string "false".
        let numeric = BuildConfiguration { opt_level: Some(OptLevel::Three), ..Default::default() };
        let updated = with_release_profile("[profile.release]\n", &numeric);
        assert!(updated.contains("opt-level = 3"), "{updated}");
        assert!(!updated.contains("opt-level = \"3\""), "{updated}");

        let off = BuildConfiguration { lto: Some(Lto::Off), ..Default::default() };
        let updated = with_release_profile("[profile.release]\n", &off);
        assert!(updated.contains("lto = false"), "{updated}");
    }

    #[test]
    fn a_manifest_with_no_release_profile_gains_one() {
        let manifest = "[package]\nname = \"app\"\n";
        let updated = with_release_profile(manifest, &configuration());
        assert!(updated.contains("[profile.release]"), "{updated}");
        assert!(updated.contains("opt-level = \"s\""), "{updated}");
        assert!(updated.parse::<DocumentMut>().is_ok(), "the result must still parse");
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
    fn a_per_package_override_is_not_disturbed() {
        // [profile.release.package."*"] is a sub-table of the one we edit, and
        // a line-based editor either mangles it or stops at it.
        let manifest = "\
[profile.release]
opt-level = 3

[profile.release.package.\"*\"]
opt-level = 2
";
        let updated = with_release_profile(manifest, &configuration());
        assert!(updated.contains("[profile.release.package.\"*\"]"), "{updated}");
        let overridden = updated.rsplit("package").next().unwrap_or(&updated);
        assert!(overridden.contains("opt-level = 2"), "{updated}");
    }

    #[test]
    fn a_manifest_we_cannot_parse_is_returned_unchanged_rather_than_guessed_at() {
        let broken = "[profile.release\nopt-level = ";
        assert_eq!(with_release_profile(broken, &configuration()), broken);
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

        assert!(diff.contains("--- a/Cargo.toml"), "{diff}");
        assert!(diff.contains("+++ b/Cargo.toml"), "{diff}");
        assert!(diff.contains("-opt-level = 3"), "{diff}");
        assert!(diff.contains("+opt-level = \"s\""), "{diff}");
    }
}
