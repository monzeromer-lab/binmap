//! One resolved point in the sweep matrix.
//!
//! A [`SweepMatrix`](crate::config::SweepMatrix) describes a space;
//! a [`BuildConfiguration`] is a single point in it, and it knows how to state
//! itself both to cargo and to a person.

use crate::config::{BuildStd, DebugInfo, Lto, OptLevel, PanicStrategy, Strip, SweepMatrix};
use serde::{Deserialize, Serialize};

/// A configuration cargo can be asked to build.
///
/// Every field is optional: `None` means "leave the profile's own value
/// alone", which is what makes a one-axis sweep possible without restating the
/// whole profile.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
pub struct BuildConfiguration {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opt_level: Option<OptLevel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lto: Option<Lto>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codegen_units: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub panic: Option<PanicStrategy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strip: Option<Strip>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub debug: Option<DebugInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overflow_checks: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_std: Option<BuildStd>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_cpu: Option<String>,
}

impl BuildConfiguration {
    /// What `cargo build --release` does with no help from us. The baseline
    /// every result is stated against.
    pub fn default_release() -> Self {
        Self {
            opt_level: Some(OptLevel::Three),
            lto: Some(Lto::Off),
            codegen_units: Some(16),
            panic: Some(PanicStrategy::Unwind),
            strip: Some(Strip::None),
            debug: Some(DebugInfo::None),
            overflow_checks: Some(false),
            build_std: None,
            target_cpu: None,
        }
    }

    /// The `--config` arguments that put this configuration into effect
    /// without editing the user's `Cargo.toml`.
    ///
    /// Nothing in a sweep writes to the user's manifest. Applying a
    /// configuration is a separate, tier-gated act.
    pub fn cargo_config_args(&self) -> Vec<String> {
        let mut args = Vec::new();
        let mut push = |key: &str, value: String| {
            args.push("--config".to_string());
            args.push(format!("profile.release.{key}={value}"));
        };
        if let Some(v) = self.opt_level {
            push("opt-level", v.as_toml().to_string());
        }
        if let Some(v) = self.lto {
            push("lto", v.as_toml().to_string());
        }
        if let Some(v) = self.codegen_units {
            push("codegen-units", v.to_string());
        }
        if let Some(v) = self.panic {
            push("panic", v.as_toml().to_string());
        }
        if let Some(v) = self.strip {
            push("strip", v.as_toml().to_string());
        }
        if let Some(v) = self.debug {
            push("debug", v.as_toml().to_string());
        }
        if let Some(v) = self.overflow_checks {
            push("overflow-checks", v.to_string());
        }
        args
    }

    /// Flags that go to rustc rather than into the profile.
    pub fn rustflags(&self) -> Vec<String> {
        let mut flags = Vec::new();
        if let Some(cpu) = &self.target_cpu {
            flags.push(format!("-Ctarget-cpu={cpu}"));
        }
        flags
    }

    /// The `-Z build-std` arguments, when the extended sweep is on. These need
    /// a nightly toolchain, which the environment probe confirms before the
    /// axis is offered.
    pub fn unstable_args(&self) -> Vec<String> {
        match self.build_std {
            None | Some(BuildStd::Off) => Vec::new(),
            Some(other) => vec![format!("-Zbuild-std={}", other.as_toml())],
        }
    }

    /// The rows of the configuration table, as `(axis, value)` pairs. Only the
    /// axes this configuration actually set.
    pub fn settings(&self) -> Vec<(&'static str, String)> {
        let mut rows: Vec<(&'static str, String)> = Vec::new();
        if let Some(v) = self.opt_level {
            rows.push(("opt-level", v.to_string()));
        }
        if let Some(v) = self.lto {
            rows.push(("lto", v.to_string()));
        }
        if let Some(v) = self.codegen_units {
            rows.push(("codegen-units", v.to_string()));
        }
        if let Some(v) = self.panic {
            rows.push(("panic", v.to_string()));
        }
        if let Some(v) = self.strip {
            rows.push(("strip", v.to_string()));
        }
        if let Some(v) = self.debug {
            rows.push(("debug", v.to_string()));
        }
        if let Some(v) = self.overflow_checks {
            rows.push(("overflow-checks", v.to_string()));
        }
        if let Some(v) = self.build_std {
            rows.push(("build-std", v.to_string()));
        }
        if let Some(v) = &self.target_cpu {
            rows.push(("target-cpu", v.clone()));
        }
        rows
    }

    /// A short, stable, filesystem-safe name. Used as the per-configuration
    /// target subdirectory, so a resumed sweep finds its own cached builds.
    pub fn name(&self) -> String {
        let mut name = String::new();
        for (axis, value) in self.settings() {
            if !name.is_empty() {
                name.push('-');
            }
            let axis: String = axis.split('-').filter_map(|part| part.chars().next()).collect();
            let value: String =
                value.chars().filter(|c| c.is_ascii_alphanumeric()).take(6).collect();
            name.push_str(&axis);
            name.push_str(&value);
        }
        if name.is_empty() { "profile-default".to_string() } else { name }
    }

    /// The one-line form the Profile Lab shows beside a point.
    pub fn describe(&self) -> String {
        let settings = self.settings();
        if settings.is_empty() {
            return "the profile's own settings".to_string();
        }
        settings.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(" ")
    }

    /// Expand a matrix into every configuration it describes.
    ///
    /// An empty axis contributes `None` — the profile's own value — rather
    /// than nothing, so the product never collapses to zero.
    pub fn expand(matrix: &SweepMatrix) -> Vec<BuildConfiguration> {
        fn axis<T: Copy>(values: &[T]) -> Vec<Option<T>> {
            if values.is_empty() { vec![None] } else { values.iter().copied().map(Some).collect() }
        }
        let target_cpus: Vec<Option<String>> = if matrix.target_cpu.is_empty() {
            vec![None]
        } else {
            matrix.target_cpu.iter().cloned().map(Some).collect()
        };

        let mut out = Vec::with_capacity(matrix.cardinality());
        for opt_level in axis(&matrix.opt_level) {
            for lto in axis(&matrix.lto) {
                for codegen_units in axis(&matrix.codegen_units) {
                    for panic in axis(&matrix.panic) {
                        for strip in axis(&matrix.strip) {
                            for debug in axis(&matrix.debug) {
                                for overflow_checks in axis(&matrix.overflow_checks) {
                                    for build_std in axis(&matrix.build_std) {
                                        for target_cpu in &target_cpus {
                                            out.push(BuildConfiguration {
                                                opt_level,
                                                lto,
                                                codegen_units,
                                                panic,
                                                strip,
                                                debug,
                                                overflow_checks,
                                                build_std,
                                                target_cpu: target_cpu.clone(),
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expansion_produces_exactly_the_matrix_cardinality() {
        let matrix = SweepMatrix::default();
        assert_eq!(BuildConfiguration::expand(&matrix).len(), matrix.cardinality());
    }

    #[test]
    fn expanded_configurations_are_distinct() {
        let configurations = BuildConfiguration::expand(&SweepMatrix::default());
        let unique: std::collections::BTreeSet<_> = configurations.iter().collect();
        assert_eq!(unique.len(), configurations.len());
    }

    #[test]
    fn names_are_distinct_so_cached_builds_do_not_collide() {
        let names: std::collections::BTreeSet<String> =
            BuildConfiguration::expand(&SweepMatrix::default()).iter().map(|c| c.name()).collect();
        assert_eq!(names.len(), SweepMatrix::default().cardinality());
    }

    #[test]
    fn a_sweep_configures_cargo_without_touching_the_manifest() {
        let configuration = BuildConfiguration {
            opt_level: Some(OptLevel::Size),
            lto: Some(Lto::Fat),
            ..BuildConfiguration::default()
        };
        assert_eq!(
            configuration.cargo_config_args(),
            [
                "--config",
                "profile.release.opt-level=\"s\"",
                "--config",
                "profile.release.lto=\"fat\"",
            ]
        );
    }

    #[test]
    fn an_unset_axis_says_nothing_to_cargo() {
        assert!(BuildConfiguration::default().cargo_config_args().is_empty());
        assert_eq!(BuildConfiguration::default().name(), "profile-default");
    }
}
