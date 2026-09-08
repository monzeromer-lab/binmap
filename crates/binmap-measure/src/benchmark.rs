//! The user's benchmark, measured through `hyperfine` (`F0.5`).
//!
//! Until an audit found it, nothing in the workspace implemented
//! [`MeasurementSource`]. The trait existed, the hyperfine wrapper existed, and
//! no code path connected them — so runtime was never measured, the noise floor
//! was never established, `BenchmarkNotWorse` always reported "no benchmark is
//! declared", and the Pareto frontier ranked on size and build time only.
//!
//! Two rules this type exists to keep:
//!
//! - **We do not invent a workload.** Runtime is measured through the user's
//!   own benchmark command and through nothing else. A project without one
//!   simply has no runtime objective, which is different from having a fast one.
//! - **The artifact under test is what runs.** The command may name the binary
//!   by the placeholder `{artifact}`, which is substituted per configuration;
//!   without it the command is run as written, in the project root.

use crate::hyperfine::{self, Timing};
use crate::timing::Samples;
use binmap_core::config::BenchmarkCommand;
use binmap_core::error::Result;
use binmap_core::tool::ToolRunner;
use binmap_core::traits::MeasurementSource;
use std::path::Path;
use std::time::Duration;

/// Where the artifact's path is substituted into the user's command.
pub const ARTIFACT: &str = "{artifact}";

/// Runs the user's benchmark against one artifact and returns every sample.
pub struct HyperfineBenchmark {
    runner: ToolRunner,
    command: BenchmarkCommand,
    timing: Timing,
}

impl HyperfineBenchmark {
    /// Build one, if the machine can run it.
    ///
    /// Returns `None` when hyperfine is not installed, so the caller ends up
    /// with no benchmark rather than a benchmark that fails ninety-six times.
    /// The environment panel already says what is missing and what to type.
    pub fn new(runner: ToolRunner, command: BenchmarkCommand) -> Option<Self> {
        if !hyperfine::is_available(&runner) {
            return None;
        }
        let timing = Timing { warmup: 3, runs: command.samples.max(3) };
        Some(Self { runner, command, timing })
    }

    /// The arguments for one artifact, with `{artifact}` substituted.
    fn arguments_for(&self, artifact: &Path) -> Vec<String> {
        let path = artifact.display().to_string();
        self.command.arguments.iter().map(|argument| argument.replace(ARTIFACT, &path)).collect()
    }

    /// The program to run. A command whose program is the placeholder runs the
    /// artifact itself, which is the common case for a binary crate.
    fn program_for(&self, artifact: &Path) -> String {
        if self.command.program == ARTIFACT {
            artifact.display().to_string()
        } else {
            self.command.program.clone()
        }
    }

    pub fn samples_for(&self, artifact: &Path) -> Result<Samples> {
        let (samples, _evidence) = hyperfine::measure(
            &self.runner,
            &self.program_for(artifact),
            &self.arguments_for(artifact),
            Some(self.runner.root()),
            &self.timing,
        )?;
        Ok(samples)
    }
}

impl MeasurementSource for HyperfineBenchmark {
    fn samples(&self, artifact: &Path) -> Result<Vec<Duration>> {
        self.samples_for(artifact).map(|samples| samples.durations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use binmap_core::evidence::EvidenceStore;

    fn command(program: &str, arguments: &[&str]) -> BenchmarkCommand {
        BenchmarkCommand {
            program: program.into(),
            arguments: arguments.iter().map(|a| a.to_string()).collect(),
            samples: 10,
        }
    }

    fn benchmark(command: BenchmarkCommand) -> HyperfineBenchmark {
        HyperfineBenchmark {
            runner: ToolRunner::new(EvidenceStore::new(), "."),
            command,
            timing: Timing::default(),
        }
    }

    #[test]
    fn the_artifact_under_test_is_what_runs() {
        // Each configuration produces its own binary; timing the same one
        // ninety-six times would produce ninety-six identical numbers and one
        // very confident wrong answer.
        let b = benchmark(command(ARTIFACT, &["--bench", "route"]));
        let path = Path::new("/tmp/binmap/ols/release/app");
        assert_eq!(b.program_for(path), "/tmp/binmap/ols/release/app");
        assert_eq!(b.arguments_for(path), ["--bench", "route"]);
    }

    #[test]
    fn the_placeholder_is_substituted_wherever_it_appears() {
        let b = benchmark(command("hyperfine-target", &["run", ARTIFACT, "--iterations", "5"]));
        let path = Path::new("/tmp/app");
        assert_eq!(b.program_for(path), "hyperfine-target");
        assert_eq!(b.arguments_for(path), ["run", "/tmp/app", "--iterations", "5"]);
    }

    #[test]
    fn a_command_without_the_placeholder_is_run_as_written() {
        // `cargo bench --bench route` names no artifact and should not have one
        // spliced in.
        let b = benchmark(command("cargo", &["bench", "--bench", "route"]));
        assert_eq!(b.arguments_for(Path::new("/tmp/app")), ["bench", "--bench", "route"]);
    }

    #[test]
    fn the_sample_count_is_the_users_and_never_below_three() {
        // Below three the comparison reports inconclusive by construction, so
        // running fewer only wastes the user's time.
        let runner = ToolRunner::new(EvidenceStore::new(), ".");
        if let Some(b) = HyperfineBenchmark::new(runner.clone(), command("true", &[])) {
            assert_eq!(b.timing.runs, 10);
        }
        let mut one = command("true", &[]);
        one.samples = 1;
        if let Some(b) = HyperfineBenchmark::new(runner, one) {
            assert_eq!(b.timing.runs, 3);
        }
    }

    #[test]
    fn there_is_no_benchmark_when_hyperfine_is_absent() {
        // Not a benchmark that fails ninety-six times. The environment panel
        // already says what is missing and what to type.
        let runner = ToolRunner::new(EvidenceStore::new(), ".");
        let built = HyperfineBenchmark::new(runner.clone(), command("true", &[]));
        assert_eq!(built.is_some(), hyperfine::is_available(&runner));
    }
}
