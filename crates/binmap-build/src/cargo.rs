//! Building one configuration with cargo.
//!
//! Nothing here edits the user's `Cargo.toml`. A configuration is put into
//! effect with `--config` arguments and a `RUSTFLAGS` environment, so a sweep
//! of ninety-six configurations leaves the manifest exactly as it found it.

use binmap_core::configuration::BuildConfiguration;
use binmap_core::error::{Error, Result};
use binmap_core::evidence::ToolInvocation;
use binmap_core::tool::ToolRunner;
use binmap_core::traits::{BuildOutcome, BuildSystem, Target};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Cargo, driven against one project.
pub struct CargoBuildSystem {
    runner: ToolRunner,
    /// The target triple, needed by `-Zbuild-std`. Detected from the
    /// toolchain, because a build-std sweep without one cannot work.
    host_triple: Option<String>,
    /// Our own target directory. The user's stays untouched, so the
    /// incremental cache they will want back the moment the sweep finishes is
    /// still there.
    target_directory: PathBuf,
}

impl CargoBuildSystem {
    pub fn new(runner: ToolRunner, target_directory: impl Into<PathBuf>) -> Self {
        // Cargo runs with the project root as its working directory, so a
        // relative target directory would be resolved against that root and
        // the sweep would build into `project/project/target`.
        let target_directory = target_directory.into();
        let target_directory = if target_directory.is_absolute() {
            target_directory
        } else {
            runner.root().join(target_directory)
        };
        let host_triple = Self::host_triple(&runner);
        Self { runner, target_directory, host_triple }
    }

    /// Ask rustc what it builds for. `-Zbuild-std` needs an explicit
    /// `--target`, and guessing the triple is how a sweep produces ninety-six
    /// identical failures.
    fn host_triple(runner: &ToolRunner) -> Option<String> {
        let output = runner.run(ToolInvocation::new("rustc", ["-vV"])).ok()?;
        output
            .stdout
            .lines()
            .find_map(|line| line.strip_prefix("host: "))
            .map(|triple| triple.trim().to_string())
    }

    pub fn runner(&self) -> &ToolRunner {
        &self.runner
    }

    /// Where this configuration's build products go.
    ///
    /// Per configuration, so a resumed sweep finds the builds it already did
    /// rather than repeating them, and so two configurations never overwrite
    /// each other's artifact.
    pub fn target_directory_for(&self, configuration: &BuildConfiguration) -> PathBuf {
        self.target_directory.join(configuration.name())
    }

    /// The exact arguments this configuration is built with. Public because
    /// the Profile Lab shows them beside each point, and a user who wants to
    /// reproduce a number should be able to retype it.
    pub fn build_arguments(
        &self,
        target: &Target,
        configuration: &BuildConfiguration,
    ) -> Vec<String> {
        let mut arguments = vec!["build".to_string(), "--release".to_string()];
        arguments.extend(configuration.unstable_args(self.host_triple.as_deref()));
        arguments.push("--package".into());
        arguments.push(target.package.clone());
        // The target directory travels in the environment rather than on the
        // command line, so the gates inherit it with everything else.
        // JSON on stdout for the artifact paths, rendered diagnostics on
        // stderr so the Builds gate can count warnings the way a person does.
        arguments.push("--message-format".into());
        arguments.push("json-render-diagnostics".into());
        arguments
    }

    /// The command a user would retype to reproduce one point by hand.
    ///
    /// The sweep configures cargo through the environment; this is the same
    /// configuration spelled as `--config` arguments, and it is what the
    /// Profile Lab shows beside a point.
    pub fn reproduction_command(
        &self,
        target: &Target,
        configuration: &BuildConfiguration,
    ) -> String {
        let mut parts = vec!["cargo".to_string(), "build".to_string(), "--release".to_string()];
        parts.extend(configuration.unstable_args(self.host_triple.as_deref()));
        parts.push("--package".into());
        parts.push(target.package.clone());
        parts.extend(configuration.cargo_config_args());
        parts.join(" ")
    }

    /// The environment one configuration is built under.
    ///
    /// TOOLING §3.2: a sweep is a loop over environment maps. The working tree
    /// is never modified, which is what makes cancellation leave nothing to
    /// clean up.
    fn env_for(&self, configuration: &BuildConfiguration) -> BTreeMap<String, String> {
        let mut env = configuration.cargo_profile_env();
        let flags = configuration.rustflags();
        if !flags.is_empty() {
            env.insert("RUSTFLAGS".to_string(), flags.join(" "));
        }
        // Per configuration, and never the user's own. This is in the
        // environment rather than on the command line precisely so the gates
        // inherit it: a gate that built in the user's cache would both
        // disturb it and measure the wrong thing.
        env.insert(
            "CARGO_TARGET_DIR".to_string(),
            self.target_directory_for(configuration).display().to_string(),
        );
        env
    }

    /// Exposed so the sweep's own integration test can assert that each axis
    /// actually changes the build — the mitigation TOOLING §8 asks for against
    /// cargo's profile-variable naming shifting under us.
    pub fn environment_for(&self, configuration: &BuildConfiguration) -> BTreeMap<String, String> {
        self.env_for(configuration)
    }
}

/// The compiler-artifact messages cargo emits on stdout, reduced to what we
/// read.
#[derive(Debug, Deserialize)]
struct CargoMessage {
    reason: String,
    #[serde(default)]
    target: Option<MessageTarget>,
    #[serde(default)]
    executable: Option<PathBuf>,
    #[serde(default)]
    filenames: Vec<PathBuf>,
}

#[derive(Debug, Deserialize)]
struct MessageTarget {
    name: String,
}

/// Pick the artifact this target produced out of cargo's message stream.
///
/// Reading the path from cargo rather than guessing `target/release/<name>`
/// keeps us correct for libraries, for renamed binaries, and for whatever
/// cargo does next.
fn artifact_from_messages(stdout: &str, target_name: &str) -> Option<PathBuf> {
    let mut found = None;
    for line in stdout.lines() {
        let Ok(message) = serde_json::from_str::<CargoMessage>(line) else { continue };
        if message.reason != "compiler-artifact" {
            continue;
        }
        if message.target.as_ref().is_none_or(|t| t.name != target_name) {
            continue;
        }
        if let Some(executable) = message.executable {
            found = Some(executable);
        } else if let Some(first) = message.filenames.into_iter().next() {
            found = Some(first);
        }
    }
    found
}

fn count_warnings(stderr: &str) -> usize {
    stderr.lines().filter(|line| line.trim_start().starts_with("warning:")).count()
}

impl BuildSystem for CargoBuildSystem {
    fn targets(&self, root: &Path) -> Result<Vec<Target>> {
        crate::project::discover(&self.runner, root).map(|(_, targets)| targets)
    }

    fn build_environment(&self, configuration: &BuildConfiguration) -> BTreeMap<String, String> {
        self.env_for(configuration)
    }

    fn build(
        &self,
        target: &Target,
        configuration: &BuildConfiguration,
    ) -> Result<BuildOutcome> {
        let arguments = self.build_arguments(target, configuration);
        let invocation = ToolInvocation::new("cargo", arguments)
            .in_directory(self.runner.root().display().to_string());
        let output = self.runner.run_with_env(invocation, &self.env_for(configuration))?;

        let artifact = if output.succeeded() {
            let found = artifact_from_messages(&output.stdout, &target.name);
            if found.is_none() {
                // The build reported success and produced nothing we can find.
                // Better to say so than to measure a stale artifact from a
                // previous configuration.
                return Err(Error::Other(format!(
                    "cargo reported success for `{}` but named no artifact",
                    target.name
                )));
            }
            found
        } else {
            None
        };

        Ok(BuildOutcome {
            configuration: configuration.clone(),
            artifact,
            succeeded: output.succeeded(),
            duration: output.duration,
            warnings: count_warnings(&output.stderr),
            evidence: output.evidence,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use binmap_core::capability::Capabilities;
    use binmap_core::config::{Lto, OptLevel};
    use binmap_core::evidence::EvidenceStore;
    use binmap_core::traits::TargetFamily;

    fn system() -> CargoBuildSystem {
        CargoBuildSystem::new(ToolRunner::new(EvidenceStore::new(), "."), "/tmp/binmap-target")
    }

    fn target() -> Target {
        Target {
            id: "app::app".into(),
            name: "app".into(),
            family: TargetFamily::Rust,
            package: "app".into(),
            manifest: "Cargo.toml".into(),
            capabilities: Capabilities::none(),
        }
    }

    #[test]
    fn a_relative_target_directory_does_not_nest_inside_the_project() {
        // Cargo runs in the project root, so a relative path would be resolved
        // there a second time — the sweep built into `project/project/target`
        // until this was fixed.
        let system = CargoBuildSystem::new(
            ToolRunner::new(EvidenceStore::new(), "/home/ada/app"),
            "target/binmap",
        );
        let directory = system.target_directory_for(&BuildConfiguration::default());
        assert!(directory.is_absolute(), "{}", directory.display());
        assert_eq!(directory, PathBuf::from("/home/ada/app/target/binmap/profile-default"));
    }

    #[test]
    fn a_sweep_never_touches_the_users_target_directory() {
        let system = system();
        let env = system.environment_for(&BuildConfiguration::default_release());
        let directory = env.get("CARGO_TARGET_DIR").expect("always set");
        assert!(directory.starts_with("/tmp/binmap-target"), "{directory}");
    }

    #[test]
    fn the_gates_inherit_the_environment_the_build_used() {
        // The whole point of putting the target directory in the environment:
        // a gate command the caller wrote knows nothing about configurations,
        // and must still build where the sweep built, under the same profile.
        let system = system();
        let configuration =
            BuildConfiguration { opt_level: Some(OptLevel::Size), ..Default::default() };
        let env = BuildSystem::build_environment(&system, &configuration);
        assert_eq!(env.get("CARGO_PROFILE_RELEASE_OPT_LEVEL").map(String::as_str), Some("s"));
        assert!(env.contains_key("CARGO_TARGET_DIR"));
    }

    #[test]
    fn each_configuration_builds_into_its_own_directory_so_a_resume_finds_it() {
        let system = system();
        let small = BuildConfiguration { opt_level: Some(OptLevel::Size), ..Default::default() };
        let fat = BuildConfiguration { lto: Some(Lto::Fat), ..Default::default() };
        assert_ne!(
            system.target_directory_for(&small),
            system.target_directory_for(&fat)
        );
    }

    #[test]
    fn the_configuration_reaches_cargo_through_the_environment_not_a_manifest_edit() {
        let system = system();
        let configuration =
            BuildConfiguration { opt_level: Some(OptLevel::Size), ..Default::default() };
        let env = system.environment_for(&configuration);
        assert_eq!(env.get("CARGO_PROFILE_RELEASE_OPT_LEVEL").map(String::as_str), Some("s"));
        // Nothing on the command line mentions the manifest, and nothing
        // writes to it.
        let arguments = system.build_arguments(&target(), &configuration);
        assert!(!arguments.iter().any(|a| a.contains("Cargo.toml")));
    }

    #[test]
    fn the_reproduction_command_is_something_a_user_can_retype() {
        let system = system();
        let configuration =
            BuildConfiguration { opt_level: Some(OptLevel::Size), ..Default::default() };
        let command = system.reproduction_command(&target(), &configuration);
        assert!(command.starts_with("cargo build --release"), "{command}");
        assert!(command.contains("--config profile.release.opt-level=\"s\""), "{command}");
    }

    #[test]
    fn the_artifact_path_comes_from_cargo_rather_than_from_a_guess() {
        let stdout = concat!(
            r#"{"reason":"compiler-artifact","target":{"name":"other"},"executable":"/x/other"}"#,
            "\n",
            r#"{"reason":"compiler-artifact","target":{"name":"app"},"executable":"/x/app"}"#,
            "\n",
            r#"{"reason":"build-finished","success":true}"#,
        );
        assert_eq!(artifact_from_messages(stdout, "app"), Some(PathBuf::from("/x/app")));
        assert_eq!(artifact_from_messages(stdout, "absent"), None);
    }

    #[test]
    fn a_library_is_found_through_its_filenames_since_it_has_no_executable() {
        let stdout =
            r#"{"reason":"compiler-artifact","target":{"name":"lib"},"filenames":["/x/liblib.rlib"]}"#;
        assert_eq!(artifact_from_messages(stdout, "lib"), Some(PathBuf::from("/x/liblib.rlib")));
    }
}
