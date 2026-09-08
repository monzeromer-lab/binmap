//! Running an external tool, and the registry that describes the ones we run.
//!
//! This is where the evidence ordering stops being a convention and starts
//! being the only way to run anything: [`ToolRunner::run`] records the
//! invocation, then spawns the process. There is no path through this module
//! that produces output without a record, which is what the rest of the trust
//! boundary is built on.

use crate::error::{Error, Result};
use crate::evidence::{EvidenceId, EvidenceStore, ToolInvocation};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

/// What a tool did, alongside the identifier a finding may cite for it.
#[derive(Debug, Clone)]
pub struct ToolOutput {
    pub evidence: EvidenceId,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub duration: Duration,
}

impl ToolOutput {
    pub fn succeeded(&self) -> bool {
        self.exit_code == 0
    }

    /// stdout and stderr, concatenated in that order.
    ///
    /// Not interleaved: interleaving is not reproducible between runs, and a
    /// record that cannot be reproduced is worse than one that is plainly
    /// ordered.
    pub fn combined(&self) -> String {
        match (self.stdout.is_empty(), self.stderr.is_empty()) {
            (_, true) => self.stdout.clone(),
            (true, false) => self.stderr.clone(),
            (false, false) => format!("{}\n{}", self.stdout, self.stderr),
        }
    }
}

/// Runs external tools against one project, recording every one.
#[derive(Debug, Clone)]
pub struct ToolRunner {
    store: EvidenceStore,
    root: PathBuf,
}

impl ToolRunner {
    pub fn new(store: EvidenceStore, root: impl Into<PathBuf>) -> Self {
        Self { store, root: root.into() }
    }

    pub fn store(&self) -> &EvidenceStore {
        &self.store
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Run a tool. The record exists before the process is spawned, so a tool
    /// that hangs or is killed still leaves a trace of what was attempted.
    pub fn run(&self, invocation: ToolInvocation) -> Result<ToolOutput> {
        self.run_with_env(invocation, &BTreeMap::new())
    }

    pub fn run_with_env(
        &self,
        invocation: ToolInvocation,
        env: &BTreeMap<String, String>,
    ) -> Result<ToolOutput> {
        let directory = invocation
            .working_directory
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| self.root.clone());

        let mut command = Command::new(&invocation.tool);
        command.args(&invocation.arguments).current_dir(&directory);
        for (key, value) in env {
            command.env(key, value);
        }

        // The record is written here, before anything runs.
        let pending = self.store.begin(invocation.clone());
        let started = Instant::now();
        let output = command.output();
        let duration = started.elapsed();

        match output {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
                let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
                let exit_code = output.status.code().unwrap_or(-1);
                let combined = match (stdout.is_empty(), stderr.is_empty()) {
                    (_, true) => stdout.clone(),
                    (true, false) => stderr.clone(),
                    (false, false) => format!("{stdout}\n{stderr}"),
                };
                let evidence = self.store.complete(pending, combined, exit_code);
                Ok(ToolOutput { evidence, stdout, stderr, exit_code, duration })
            }
            Err(source) => {
                // A tool that could not be spawned is still a fact about the
                // machine, and it is recorded as one.
                let reason = source.to_string();
                self.store.complete(pending, format!("failed to spawn: {reason}"), -1);
                Err(Error::ToolUnavailable { tool: invocation.tool, reason })
            }
        }
    }

    /// Whether a tool can be spawned at all. The environment panel's question.
    pub fn is_available(&self, tool: &str) -> bool {
        Command::new(tool)
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }
}

/// What one tool is, in the terms an external agent needs.
///
/// The description string is the entire briefing an agent arriving over MCP
/// gets, which is why it is a full sentence and not an identifier. A high
/// rejection rate in Mode B means these are unclear, not that the agent is bad.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    /// Whether running it changes anything outside our own target directory.
    /// The trust tier gates on this, whoever called the tool.
    pub side_effects: bool,
    pub required_tier: crate::config::TrustTier,
}

impl ToolSpec {
    pub fn read_only(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            side_effects: false,
            required_tier: crate::config::TrustTier::Observe,
        }
    }

    pub fn writes(
        name: impl Into<String>,
        description: impl Into<String>,
        required_tier: crate::config::TrustTier,
    ) -> Self {
        Self { name: name.into(), description: description.into(), side_effects: true, required_tier }
    }
}

/// One registry, two exposures: natively to the Mode A loop, and over MCP to
/// external agents. A tool written twice means the abstraction has broken (§3).
#[derive(Debug, Clone, Default)]
pub struct ToolRegistry {
    specs: BTreeMap<String, ToolSpec>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a tool. Returns an error rather than overwriting, because two
    /// registrations of one name is the abstraction breaking, not a preference.
    pub fn register(&mut self, spec: ToolSpec) -> Result<()> {
        if self.specs.contains_key(&spec.name) {
            return Err(Error::Other(format!("`{}` is already registered", spec.name)));
        }
        self.specs.insert(spec.name.clone(), spec);
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<&ToolSpec> {
        self.specs.get(name)
    }

    pub fn specs(&self) -> impl Iterator<Item = &ToolSpec> {
        self.specs.values()
    }

    /// Check a call against the session's tier before it runs — the same check
    /// whether the caller is our own loop or an external agent.
    pub fn authorize(&self, name: &str, tier: crate::config::TrustTier) -> Result<&ToolSpec> {
        let spec = self
            .get(name)
            .ok_or_else(|| Error::Other(format!("`{name}` is not a registered tool")))?;
        tier.require(spec.required_tier, &spec.name)?;
        Ok(spec)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TrustTier;

    #[test]
    fn every_run_leaves_a_record_even_when_the_tool_is_missing() {
        let store = EvidenceStore::new();
        let runner = ToolRunner::new(store.clone(), ".");
        let error = runner
            .run(ToolInvocation::new("binmap-tool-that-does-not-exist", ["--version"]))
            .unwrap_err();
        assert!(matches!(error, Error::ToolUnavailable { .. }));
        assert_eq!(store.len(), 1, "the attempt is a fact about the machine");
        assert!(store.records()[0].output.contains("failed to spawn"));
    }

    #[test]
    fn output_is_captured_and_cited_by_identifier() {
        let store = EvidenceStore::new();
        let runner = ToolRunner::new(store.clone(), ".");
        let output = runner.run(ToolInvocation::new("echo", ["binmap"])).unwrap();
        assert!(output.succeeded());
        assert_eq!(output.stdout.trim(), "binmap");
        assert!(store.issued(&output.evidence));
        assert_eq!(store.get(&output.evidence).unwrap().output.trim(), "binmap");
    }

    #[test]
    fn a_tool_above_the_session_tier_is_refused_before_it_runs() {
        let mut registry = ToolRegistry::new();
        registry
            .register(ToolSpec::writes(
                "apply_patch",
                "Write a verified patch into the working tree.",
                TrustTier::Tune,
            ))
            .unwrap();
        let error = registry.authorize("apply_patch", TrustTier::Propose).unwrap_err();
        assert!(matches!(error, Error::TierTooLow { .. }));
        assert!(registry.authorize("apply_patch", TrustTier::Tune).is_ok());
    }

    #[test]
    fn registering_one_name_twice_is_an_error_not_a_replacement() {
        let mut registry = ToolRegistry::new();
        let spec = ToolSpec::read_only("read_symbols", "List the artifact's symbols with sizes.");
        registry.register(spec.clone()).unwrap();
        assert!(registry.register(spec).is_err());
    }
}
