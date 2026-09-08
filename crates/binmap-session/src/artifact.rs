//! The schema.

use binmap_core::error::{Error, Result};
use binmap_core::evidence::{Evidence, EvidenceId, EvidenceStore};
use binmap_core::finding::Finding;
use binmap_core::gate::VerificationReport;
use binmap_core::traits::{Target, TargetFamily};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Versioned from the first release, so a file written today can be read by a
/// build shipped later — or refused with a sentence rather than a parse error.
pub const SCHEMA_VERSION: u32 = 1;

/// What the artifact says about the thing that was analysed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetMetadata {
    pub id: String,
    pub name: String,
    pub package: String,
    pub family: TargetFamily,
    /// The commit the analysis ran against, and whether the tree was dirty.
    /// A number measured on an uncommitted tree cannot be reproduced by
    /// anyone else, and the artifact says so rather than implying otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    #[serde(default)]
    pub dirty: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<PathBuf>,
}

impl TargetMetadata {
    pub fn of(target: &Target) -> Self {
        Self {
            id: target.id.clone(),
            name: target.name.clone(),
            package: target.package.clone(),
            family: target.family,
            commit: None,
            dirty: false,
            root: None,
        }
    }
}

/// One run's own state, kept opaque.
///
/// A sweep's state is `binmap-build`'s business, and this crate — which the
/// interface depends on — must not learn its shape. It is carried as JSON,
/// stored and returned unchanged, and only the engine that wrote it knows how
/// to read it back.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunRecord {
    pub id: String,
    /// What kind of run this was, so the engine that owns it can recognise it.
    pub kind: String,
    pub state: serde_json::Value,
}

/// Everything one session knows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionArtifact {
    pub schema_version: u32,
    /// The build that wrote it. Shown when a file is refused, so the user is
    /// told which version to open it with.
    pub binmap_version: String,
    /// Seconds since the Unix epoch. Deliberately not a formatted timestamp:
    /// a timezone in a data file is a bug waiting for a colleague in another
    /// one.
    pub created_unix: u64,
    pub target: TargetMetadata,
    #[serde(default)]
    pub findings: Vec<Finding>,
    #[serde(default)]
    pub evidence: Vec<Evidence>,
    #[serde(default)]
    pub gates: Vec<VerificationReport>,
    #[serde(default)]
    pub runs: Vec<RunRecord>,
    /// Set when this artifact was written by an export that redacted
    /// something. Import surfaces it, because a redacted evidence record is
    /// not the same as the one the tool produced.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redacted: Option<crate::redact::RedactionReport>,
}

impl SessionArtifact {
    pub fn new(target: TargetMetadata) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            binmap_version: env!("CARGO_PKG_VERSION").to_string(),
            created_unix: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            target,
            findings: Vec::new(),
            evidence: Vec::new(),
            gates: Vec::new(),
            runs: Vec::new(),
            redacted: None,
        }
    }

    pub fn with_findings(mut self, findings: Vec<Finding>) -> Self {
        self.findings = findings;
        self
    }

    pub fn with_evidence(mut self, evidence: Vec<Evidence>) -> Self {
        self.evidence = evidence;
        self
    }

    pub fn with_gates(mut self, gates: Vec<VerificationReport>) -> Self {
        self.gates = gates;
        self
    }

    pub fn with_run(
        mut self,
        id: impl Into<String>,
        kind: impl Into<String>,
        state: &impl Serialize,
    ) -> Result<Self> {
        let state = serde_json::to_value(state)
            .map_err(|source| Error::serialization("serializing run state", source))?;
        self.runs.push(RunRecord { id: id.into(), kind: kind.into(), state });
        Ok(self)
    }

    /// Read one run's state back, in whatever shape its owner expects.
    pub fn run_state<T: serde::de::DeserializeOwned>(&self, id: &str) -> Option<Result<T>> {
        let record = self.runs.iter().find(|run| run.id == id)?;
        Some(
            serde_json::from_value(record.state.clone())
                .map_err(|source| Error::serialization(format!("reading run `{id}`"), source)),
        )
    }
}

/// What came back from an import, including what did not.
#[derive(Debug, Clone)]
pub struct ImportOutcome {
    pub artifact: SessionArtifact,
    /// A populated store holding every evidence record whose digest checked
    /// out.
    pub evidence: EvidenceStore,
    /// The findings that survived revalidation.
    pub findings: Vec<Finding>,
    /// Evidence refused because its recorded output no longer hashes to its
    /// recorded digest.
    pub tampered_evidence: Vec<EvidenceId>,
    /// Findings refused because the evidence they cite did not survive.
    /// Reported by name, never dropped silently.
    pub ungrounded_findings: Vec<String>,
}

impl ImportOutcome {
    /// The sentence the restored-session notice shows when something was
    /// refused.
    pub fn refusal_notice(&self) -> Option<String> {
        if self.tampered_evidence.is_empty() && self.ungrounded_findings.is_empty() {
            return None;
        }
        let mut parts = Vec::new();
        if !self.tampered_evidence.is_empty() {
            parts.push(format!(
                "{} evidence record(s) no longer match their digest",
                self.tampered_evidence.len()
            ));
        }
        if !self.ungrounded_findings.is_empty() {
            parts.push(format!(
                "{} finding(s) were dropped for citing them: {}",
                self.ungrounded_findings.len(),
                self.ungrounded_findings.join(", ")
            ));
        }
        Some(parts.join("; "))
    }
}

/// Read an artifact back into a usable session.
///
/// Import is not construction, so it runs the same checks construction ran.
/// A file is not a more trustworthy source than an agent.
pub fn import(artifact: SessionArtifact) -> Result<ImportOutcome> {
    if artifact.schema_version > SCHEMA_VERSION {
        return Err(Error::Config(format!(
            "this session was written by Binmap {} at schema version {}; this build reads up to \
             version {SCHEMA_VERSION}",
            artifact.binmap_version, artifact.schema_version
        )));
    }

    let evidence = EvidenceStore::new();
    let tampered = evidence.adopt(artifact.evidence.clone());

    let mut findings = Vec::new();
    let mut ungrounded = Vec::new();
    for finding in &artifact.findings {
        match finding.revalidate(&evidence) {
            Ok(()) => findings.push(finding.clone()),
            Err(_) => ungrounded.push(finding.id.clone()),
        }
    }

    Ok(ImportOutcome {
        artifact,
        evidence,
        findings,
        tampered_evidence: tampered,
        ungrounded_findings: ungrounded,
    })
}
