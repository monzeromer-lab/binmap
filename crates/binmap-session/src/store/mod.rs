//! Writing a session to disk and reading it back.

use crate::artifact::{ImportOutcome, SessionArtifact};
use crate::redact::{RedactionReport, Redactor};
use binmap_core::error::{Error, Result};
use std::path::{Path, PathBuf};

/// Where sessions live, and how they get there.
#[derive(Debug, Clone)]
pub struct SessionStore {
    directory: PathBuf,
    redactor: Redactor,
}

impl SessionStore {
    pub fn new(directory: impl Into<PathBuf>) -> Self {
        Self { directory: directory.into(), redactor: Redactor::from_environment() }
    }

    pub fn with_redactor(mut self, redactor: Redactor) -> Self {
        self.redactor = redactor;
        self
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// The session file for one target. Named by target id so reopening a
    /// project finds what it knew last time.
    pub fn path_for(&self, target_id: &str) -> PathBuf {
        let safe: String = target_id
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' })
            .collect();
        self.directory.join(format!("{safe}.binmap.json"))
    }

    /// Save a session for our own later use. Not redacted: this file stays on
    /// the user's machine, and redacting the user's own session would make the
    /// evidence tab lie to the person who produced it.
    pub fn save(&self, artifact: &SessionArtifact) -> Result<PathBuf> {
        std::fs::create_dir_all(&self.directory)
            .map_err(|source| Error::io(&self.directory, source))?;
        let path = self.path_for(&artifact.target.id);
        let json = serde_json::to_string_pretty(artifact)
            .map_err(|source| Error::serialization("writing the session", source))?;
        std::fs::write(&path, json).map_err(|source| Error::io(&path, source))?;
        Ok(path)
    }

    pub fn load(&self, target_id: &str) -> Result<Option<ImportOutcome>> {
        let path = self.path_for(target_id);
        if !path.exists() {
            return Ok(None);
        }
        self.read(&path).map(Some)
    }

    /// Read an artifact from anywhere — including one a colleague sent
    /// (`U13`).
    pub fn read(&self, path: &Path) -> Result<ImportOutcome> {
        let text = std::fs::read_to_string(path).map_err(|source| Error::io(path, source))?;
        let artifact: SessionArtifact = serde_json::from_str(&text)
            .map_err(|source| Error::serialization(format!("reading {}", path.display()), source))?;
        crate::artifact::import(artifact)
    }

    /// Export for someone else to read: redacted, and reporting what it
    /// changed.
    ///
    /// The redaction is written into the artifact itself, so the person who
    /// receives it is told the evidence has been altered rather than left to
    /// assume it is verbatim.
    pub fn export(
        &self,
        artifact: &SessionArtifact,
        path: &Path,
    ) -> Result<RedactionReport> {
        let mut report = RedactionReport::default();
        let mut copy = artifact.clone();

        for evidence in &mut copy.evidence {
            evidence.output = self.redactor.redact(&evidence.output, &mut report);
            evidence.invocation.arguments = evidence
                .invocation
                .arguments
                .iter()
                .map(|argument| self.redactor.redact(argument, &mut report))
                .collect();
            if let Some(directory) = &evidence.invocation.working_directory {
                evidence.invocation.working_directory =
                    Some(self.redactor.redact(directory, &mut report));
            }

            // Re-digest, and say so.
            //
            // Leaving the original digest looked principled — the mismatch
            // would be visible to anyone who checked — but it made an export
            // useless: import refuses every record whose digest does not
            // match, so the recipient opened a session with no evidence and
            // therefore no findings at all. An artifact that cannot be read is
            // not a more honest artifact.
            //
            // So the record is re-digested over its redacted content, and the
            // original digest is kept beside it. The recipient can see that a
            // record was altered, and by how much, and still has a session
            // they can open. `redacted` on the artifact says the same thing at
            // the top level.
            if evidence.digest != evidence.expected_digest() {
                evidence.redacted_from = Some(evidence.digest.clone());
                evidence.digest = evidence.expected_digest();
            }
        }

        if let Some(root) = &copy.target.root {
            let mut redacted_root = RedactionReport::default();
            copy.target.root = Some(PathBuf::from(
                self.redactor.redact(&root.display().to_string(), &mut redacted_root),
            ));
            for (key, count) in redacted_root.counts {
                *report.counts.entry(key).or_insert(0) += count;
            }
        }

        copy.redacted = if report.is_empty() { None } else { Some(report.clone()) };

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| Error::io(parent, source))?;
        }
        let json = serde_json::to_string_pretty(&copy)
            .map_err(|source| Error::serialization("writing the export", source))?;
        std::fs::write(path, json).map_err(|source| Error::io(path, source))?;
        Ok(report)
    }
}

#[cfg(test)]
mod tests;
