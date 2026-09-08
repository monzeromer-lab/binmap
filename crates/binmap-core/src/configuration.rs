//! One resolved point in the sweep matrix.
//!
//! A [`SweepMatrix`](crate::config::SweepMatrix) describes a space;
//! a [`BuildConfiguration`] is a single point in it, and it knows how to state
//! itself both to cargo and to a person.

use crate::config::{BuildStd, DebugInfo, Lto, OptLevel, PanicStrategy, Strip, SweepMatrix};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

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
    ///
    /// It sets **nothing**, and that is the point. Writing out cargo's
    /// documented release defaults instead looks equivalent and is not: cargo
    /// strips debuginfo in release by default, so passing an explicit
    /// `strip = "none"` produces a binary nine times larger than the one the
    /// user actually ships — and every reduction stated against it would be a
    /// wrong number delivered confidently. The only baseline we can defend is
    /// the build the user would get without us.
    pub fn default_release() -> Self {
        Self::default()
    }

    /// Cargo's documented release defaults, written out.
    ///
    /// Useful for showing a user what a profile currently implies. Never a
    /// baseline: see [`default_release`](Self::default_release).
    pub fn documented_release_defaults() -> Self {
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

    /// The environment that puts this configuration into effect without
    /// editing the user's `Cargo.toml`.
    ///
    /// This is the primary mechanism (TOOLING §3.2). Cargo reads profile
    /// settings from `CARGO_PROFILE_<PROFILE>_<SETTING>`, uppercased with
    /// hyphens as underscores, and the consequences are what make a sweep
    /// tractable: it is a loop over environment maps, the working tree is
    /// never modified, runs parallelise across distinct target directories,
    /// and cancellation leaves nothing to clean up.
    ///
    /// Values are unquoted here, unlike their TOML spellings — `lto=fat`, not
    /// `lto="fat"`.
    pub fn cargo_profile_env(&self) -> BTreeMap<String, String> {
        let mut env = BTreeMap::new();
        let mut set = |setting: &str, value: String| {
            env.insert(
                format!("CARGO_PROFILE_RELEASE_{}", setting.replace('-', "_").to_uppercase()),
                value,
            );
        };
        if let Some(v) = self.opt_level {
            set("opt-level", v.to_string());
        }
        if let Some(v) = self.lto {
            set("lto", v.to_string());
        }
        if let Some(v) = self.codegen_units {
            set("codegen-units", v.to_string());
        }
        if let Some(v) = self.panic {
            set("panic", v.to_string());
        }
        if let Some(v) = self.strip {
            set("strip", v.to_string());
        }
        if let Some(v) = self.debug {
            set("debug", v.to_string());
        }
        if let Some(v) = self.overflow_checks {
            set("overflow-checks", v.to_string());
        }
        env
    }

    /// The same settings as `--config` arguments.
    ///
    /// TOOLING §3.2 records both mechanisms. The environment is what the sweep
    /// uses; this exists because it is what a user reproduces a result with by
    /// hand, and it appears beside every point in the Profile Lab.
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

    /// The `-Z build-std` arguments, when the extended sweep is on.
    ///
    /// Three things here were wrong until an audit found them, and each one
    /// alone makes the axis fail:
    ///
    /// - `panic_immediate_abort` is a build-std **feature**, not a crate. It
    ///   goes to `-Zbuild-std-features`, and passing it as a crate name makes
    ///   cargo fail to find a crate by that name.
    /// - `-Zbuild-std` requires an explicit `--target`, because rebuilding the
    ///   standard library for the host without naming it is not something
    ///   cargo will do.
    /// - Both require nightly, which the environment probe confirms before the
    ///   axis is offered rather than after ninety-six builds have failed
    ///   identically.
    ///
    /// `target` is the triple to build for; `None` yields no arguments at all,
    /// because a build-std sweep without one cannot work and silently
    /// producing a broken command line is worse than producing none.
    pub fn unstable_args(&self, target: Option<&str>) -> Vec<String> {
        let Some(build_std) = self.build_std else { return Vec::new() };
        if build_std == BuildStd::Off {
            return Vec::new();
        }
        let Some(target) = target else { return Vec::new() };

        let mut args = vec![
            "-Zbuild-std=core,alloc,std,panic_abort".to_string(),
            format!("--target={target}"),
        ];
        if build_std == BuildStd::PanicImmediateAbort {
            // The lever this axis exists for: it removes the panic formatting
            // machinery entirely, which is dramatic on small binaries.
            args.push("-Zbuild-std-features=panic_immediate_abort".to_string());
        }
        args
    }

    /// Whether this configuration needs a nightly toolchain.
    pub fn needs_nightly(&self) -> bool {
        !matches!(self.build_std, None | Some(BuildStd::Off))
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

    /// A short, stable, filesystem-safe name.
    ///
    /// This one string is the per-configuration target subdirectory, the key a
    /// resumed sweep looks up, and the id of a point on the frontier — so two
    /// configurations sharing it is not a cosmetic problem. The readable part
    /// abbreviates each axis and truncates its value, which collided for long
    /// values like `target-cpu=x86-64-v3` against `target-cpu=x86-64-v4`; the
    /// digest suffix is what makes collision impossible rather than unlikely.
    pub fn name(&self) -> String {
        let settings = self.settings();
        if settings.is_empty() {
            return "profile-default".to_string();
        }

        let mut readable = String::new();
        for (axis, value) in &settings {
            if !readable.is_empty() {
                readable.push('-');
            }
            let axis: String = axis.split('-').filter_map(|part| part.chars().next()).collect();
            let value: String =
                value.chars().filter(|c| c.is_ascii_alphanumeric()).take(6).collect();
            readable.push_str(&axis);
            readable.push_str(&value);
        }

        // Over the full, untruncated settings, so nothing the readable part
        // dropped can make two configurations agree.
        let mut hasher = Sha256::new();
        for (axis, value) in &settings {
            hasher.update(axis.as_bytes());
            hasher.update([0u8]);
            hasher.update(value.as_bytes());
            hasher.update([0u8]);
        }
        let digest = format!("{:x}", hasher.finalize());
        format!("{readable}-{}", &digest[..8])
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
    fn long_values_that_share_a_prefix_do_not_share_a_name() {
        // The readable part truncates to six characters, so these were the
        // same string — and that string is the target directory, the resume
        // key and the frontier point id all at once.
        let a = BuildConfiguration { target_cpu: Some("x86-64-v3".into()), ..Default::default() };
        let b = BuildConfiguration { target_cpu: Some("x86-64-v4".into()), ..Default::default() };
        assert_ne!(a.name(), b.name(), "two target-cpu points collapsed to one directory");
    }

    #[test]
    fn a_name_is_stable_across_runs() {
        // It is a cache key and a resume key, so it must not depend on
        // anything that varies between processes.
        let configuration =
            BuildConfiguration { opt_level: Some(OptLevel::Size), ..Default::default() };
        assert_eq!(configuration.name(), configuration.clone().name());
    }

    #[test]
    fn build_std_passes_a_feature_as_a_feature_and_demands_a_target() {
        let configuration = BuildConfiguration {
            build_std: Some(BuildStd::PanicImmediateAbort),
            ..Default::default()
        };
        // Without a target triple there is no workable command line, so there
        // is none at all rather than a broken one.
        assert!(configuration.unstable_args(None).is_empty());

        let args = configuration.unstable_args(Some("x86_64-unknown-linux-gnu"));
        assert!(
            args.contains(&"-Zbuild-std-features=panic_immediate_abort".to_string()),
            "{args:?}"
        );
        assert!(args.contains(&"--target=x86_64-unknown-linux-gnu".to_string()), "{args:?}");
        // The feature must not appear as a crate name.
        assert!(
            !args.iter().any(|a| a.starts_with("-Zbuild-std=") && a.contains("panic_immediate")),
            "a build-std feature was passed as a crate: {args:?}"
        );
        assert!(configuration.needs_nightly());
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

    #[test]
    fn the_environment_spells_settings_the_way_cargo_reads_them() {
        // TOOLING §3.2 flags the naming edge cases: opt-level becomes
        // OPT_LEVEL, and env values are unquoted where TOML values are not.
        let configuration = BuildConfiguration {
            opt_level: Some(OptLevel::SizeNoLoopVec),
            lto: Some(Lto::Fat),
            codegen_units: Some(1),
            ..BuildConfiguration::default()
        };
        let env = configuration.cargo_profile_env();
        assert_eq!(env.get("CARGO_PROFILE_RELEASE_OPT_LEVEL").map(String::as_str), Some("z"));
        assert_eq!(env.get("CARGO_PROFILE_RELEASE_LTO").map(String::as_str), Some("fat"));
        assert_eq!(env.get("CARGO_PROFILE_RELEASE_CODEGEN_UNITS").map(String::as_str), Some("1"));
        // Unset axes say nothing, so the profile's own value stands.
        assert!(!env.contains_key("CARGO_PROFILE_RELEASE_PANIC"));
    }

    #[test]
    fn the_baseline_sets_no_environment_at_all() {
        assert!(BuildConfiguration::default_release().cargo_profile_env().is_empty());
    }

    #[test]
    fn the_baseline_is_the_build_the_user_would_get_without_us() {
        // Not cargo's documented defaults written out: cargo strips debuginfo
        // in release, so an explicit strip="none" measures a binary nobody
        // ships and inflates every reduction stated against it.
        let baseline = BuildConfiguration::default_release();
        assert!(baseline.cargo_config_args().is_empty());
        assert_ne!(baseline, BuildConfiguration::documented_release_defaults());
        assert_eq!(BuildConfiguration::documented_release_defaults().strip, Some(Strip::None));
    }
}
