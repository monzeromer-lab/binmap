//! The evidence store, and the ordering the whole trust boundary rests on.
//!
//! Every tool invocation writes a record carrying an identifier *before* its
//! result is returned to anyone. Nothing else can mint an identifier, so a
//! claim citing one that the store never issued is detectable — which is what
//! makes the airlock in Mode B possible at all (`A1.2`).
//!
//! The usage is deliberately two-step and cannot be short-circuited:
//!
//! ```
//! # use binmap_core::evidence::{EvidenceStore, ToolInvocation};
//! let store = EvidenceStore::new();
//! let ticket = store.begin(ToolInvocation::new("cargo", ["build", "--release"]));
//! // ... the tool runs here, and the record already exists ...
//! let id = store.complete(ticket, "Finished `release` profile", 0);
//! assert!(store.issued(&id));
//! ```

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

/// A handle to one recorded tool invocation.
///
/// There is no public constructor and no `From<String>`: an identifier that
/// exists was issued by a store. Deserializing one is possible — a session
/// artifact is full of them — but a deserialized identifier is worthless until
/// a store confirms it issued that identifier, which is exactly what
/// [`Finding::new`](crate::finding::Finding::new) makes it do.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EvidenceId(String);

impl EvidenceId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for EvidenceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// What was run, and with what. Recorded before the run, so a tool that hangs
/// or crashes still leaves a trace of what was attempted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolInvocation {
    pub tool: String,
    pub arguments: Vec<String>,
    /// Set when the invocation is scoped to a directory that is not the
    /// project root.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub working_directory: Option<String>,
}

impl ToolInvocation {
    pub fn new<S: Into<String>>(
        tool: impl Into<String>,
        arguments: impl IntoIterator<Item = S>,
    ) -> Self {
        Self {
            tool: tool.into(),
            arguments: arguments.into_iter().map(Into::into).collect(),
            working_directory: None,
        }
    }

    pub fn in_directory(mut self, directory: impl Into<String>) -> Self {
        self.working_directory = Some(directory.into());
        self
    }

    /// The command as a user would type it. Shown in the Evidence tab so a
    /// claim can be reproduced by hand.
    pub fn command_line(&self) -> String {
        let mut line = self.tool.clone();
        for argument in &self.arguments {
            line.push(' ');
            if argument.contains(' ') {
                line.push('\'');
                line.push_str(argument);
                line.push('\'');
            } else {
                line.push_str(argument);
            }
        }
        line
    }
}

/// One complete record: what ran, what it said, and a digest of what it said.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evidence {
    pub id: EvidenceId,
    pub invocation: ToolInvocation,
    /// The verbatim output. The Evidence tab shows this and does not summarize
    /// it — a summary is a claim, and claims need evidence of their own.
    pub output: String,
    /// `sha256` of the output, hex-encoded. Lets a session artifact be checked
    /// for tampering without re-running anything.
    pub digest: String,
    /// The process exit status, where the tool is a process.
    pub exit_code: i32,
    /// The digest this record carried before an export redacted it.
    ///
    /// Set only on an exported artifact. Its presence is the statement that
    /// this record is not byte-for-byte what the tool produced — the output
    /// has had home directories, hostnames and anything that looked like a
    /// credential removed. The record still verifies, so the session opens;
    /// this is how the recipient knows it was altered anyway.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redacted_from: Option<String>,
}

impl Evidence {
    /// The digest this record should carry.
    ///
    /// It covers the identifier, the tool, its arguments, its working
    /// directory, the exit code and the output — the whole record, not only
    /// the output. Digesting the output alone let a forged record keep a valid
    /// digest while its command line said something else entirely, which an
    /// audit found and which defeats the whole point of recording the command.
    pub fn expected_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.id.0.as_bytes());
        hasher.update([0u8]);
        hasher.update(self.invocation.tool.as_bytes());
        for argument in &self.invocation.arguments {
            hasher.update([0u8]);
            hasher.update(argument.as_bytes());
        }
        hasher.update([0u8]);
        hasher.update(self.invocation.working_directory.as_deref().unwrap_or_default().as_bytes());
        hasher.update([0u8]);
        hasher.update(self.exit_code.to_le_bytes());
        hasher.update([0u8]);
        hasher.update(self.output.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Whether the record still hashes to the digest it carries.
    pub fn digest_matches(&self) -> bool {
        self.expected_digest() == self.digest
    }

    /// Whether an export altered this record's content.
    ///
    /// A redacted record still verifies — it was re-digested over what it now
    /// says — so this is the only way to know, and the Evidence tab shows it
    /// rather than presenting redacted output as verbatim.
    pub fn was_redacted(&self) -> bool {
        self.redacted_from.is_some()
    }
}

/// An invocation that has been recorded but has not produced output yet.
///
/// Holding one is proof that the record exists. It is not `Clone`, so a single
/// invocation cannot be completed twice.
#[derive(Debug)]
#[must_use = "the invocation is recorded but incomplete until `complete` is called"]
pub struct PendingEvidence {
    id: EvidenceId,
    invocation: ToolInvocation,
}

impl PendingEvidence {
    pub fn id(&self) -> &EvidenceId {
        &self.id
    }
}

/// The append-only record of everything that ran during a session.
#[derive(Debug, Clone, Default)]
pub struct EvidenceStore {
    inner: Arc<RwLock<Inner>>,
}

#[derive(Debug, Default)]
struct Inner {
    next: u64,
    /// Ordered so the Evidence tab and the exported artifact list invocations
    /// in the order they happened.
    records: BTreeMap<EvidenceId, Evidence>,
    pending: BTreeMap<EvidenceId, ToolInvocation>,
}

impl EvidenceStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record an invocation and mint its identifier. Call this *before* the
    /// tool runs, never after.
    pub fn begin(&self, invocation: ToolInvocation) -> PendingEvidence {
        let mut inner = self.inner.write().expect("evidence store poisoned");
        inner.next += 1;
        let id = EvidenceId(format!("ev-{:06}", inner.next));
        inner.pending.insert(id.clone(), invocation.clone());
        PendingEvidence { id, invocation }
    }

    /// Attach the tool's output to its record and return the identifier a
    /// finding may cite.
    pub fn complete(
        &self,
        pending: PendingEvidence,
        output: impl Into<String>,
        exit_code: i32,
    ) -> EvidenceId {
        let output = output.into();
        let mut evidence = Evidence {
            id: pending.id.clone(),
            invocation: pending.invocation,
            digest: String::new(),
            output,
            exit_code,
            redacted_from: None,
        };
        evidence.digest = evidence.expected_digest();
        let mut inner = self.inner.write().expect("evidence store poisoned");
        inner.pending.remove(&pending.id);
        inner.records.insert(pending.id.clone(), evidence);
        pending.id
    }

    /// Whether this store issued that identifier and holds its output. The
    /// airlock's single question.
    pub fn issued(&self, id: &EvidenceId) -> bool {
        self.inner.read().expect("evidence store poisoned").records.contains_key(id)
    }

    pub fn get(&self, id: &EvidenceId) -> Option<Evidence> {
        self.inner.read().expect("evidence store poisoned").records.get(id).cloned()
    }

    /// Every completed record, in the order the invocations were made.
    pub fn records(&self) -> Vec<Evidence> {
        self.inner.read().expect("evidence store poisoned").records.values().cloned().collect()
    }

    /// Invocations that were begun and never completed — a tool that hung, or
    /// a run that was cancelled. The environment panel surfaces these rather
    /// than pretending they did not happen.
    pub fn incomplete(&self) -> Vec<ToolInvocation> {
        self.inner.read().expect("evidence store poisoned").pending.values().cloned().collect()
    }

    pub fn len(&self) -> usize {
        self.inner.read().expect("evidence store poisoned").records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Re-populate a store from an imported session artifact.
    ///
    /// This is the one path by which a record enters without a tool having run
    /// here, and it is therefore the one place identifiers arrive from
    /// outside. It is not a hole in "nothing else can mint one": an adopted
    /// record must hash to the digest it carries, over its whole content —
    /// identifier, command line, exit code and output — so a forged record
    /// cannot be given an identifier that a store would honour without also
    /// forging a `sha256` preimage.
    ///
    /// Returns the identifiers it refused.
    pub fn adopt(&self, records: impl IntoIterator<Item = Evidence>) -> Vec<EvidenceId> {
        let mut refused = Vec::new();
        let mut inner = self.inner.write().expect("evidence store poisoned");
        for record in records {
            if !record.digest_matches() {
                refused.push(record.id.clone());
                continue;
            }
            if let Some(serial) =
                record.id.0.strip_prefix("ev-").and_then(|n| n.parse::<u64>().ok())
            {
                inner.next = inner.next.max(serial);
            }
            inner.records.insert(record.id.clone(), record);
        }
        refused
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_record_exists_before_the_output_does() {
        let store = EvidenceStore::new();
        let pending = store.begin(ToolInvocation::new("nm", ["-S", "target/release/app"]));
        // Nothing has been completed, but the invocation is already on record.
        assert_eq!(store.incomplete().len(), 1);
        assert!(!store.issued(pending.id()));

        let id = store.complete(pending, "0000000000001234 T main", 0);
        assert!(store.issued(&id));
        assert!(store.incomplete().is_empty());
    }

    #[test]
    fn the_digest_covers_the_command_line_not_only_the_output() {
        // A record whose output is untouched but whose command line has been
        // rewritten must not still verify. Digesting the output alone let a
        // forged invocation keep a valid digest, which defeats the point of
        // recording the command at all.
        let store = EvidenceStore::new();
        let pending = store.begin(ToolInvocation::new("size", ["app"]));
        let id = store.complete(pending, "text 1024", 0);

        let mut forged = store.get(&id).unwrap();
        forged.invocation = ToolInvocation::new("size", ["some-other-binary"]);
        assert!(!forged.digest_matches(), "a rewritten command line still verified");

        let mut relabelled = store.get(&id).unwrap();
        relabelled.exit_code = 1;
        assert!(!relabelled.digest_matches(), "a rewritten exit code still verified");
    }

    #[test]
    fn a_forged_record_cannot_be_adopted_into_a_live_store() {
        // adopt() is the only path by which an identifier arrives from
        // outside. It is not a hole in "nothing else can mint one": the record
        // has to hash to its own digest over its whole content.
        let store = EvidenceStore::new();
        let forged = Evidence {
            id: EvidenceId("ev-000001".into()),
            invocation: ToolInvocation::new("cargo", ["build"]),
            output: "it definitely worked".into(),
            digest: "0".repeat(64),
            exit_code: 0,
            redacted_from: None,
        };
        let refused = store.adopt([forged.clone()]);
        assert_eq!(refused, vec![forged.id.clone()]);
        assert!(!store.issued(&forged.id), "a forged record entered a live store");
    }

    #[test]
    fn output_is_digested_and_the_digest_is_checkable() {
        let store = EvidenceStore::new();
        let pending = store.begin(ToolInvocation::new("size", ["app"]));
        let id = store.complete(pending, "text data bss", 0);
        let mut evidence = store.get(&id).unwrap();
        assert!(evidence.digest_matches());
        evidence.output.push_str(" tampered");
        assert!(!evidence.digest_matches());
    }

    #[test]
    fn import_refuses_records_whose_digest_does_not_match() {
        let store = EvidenceStore::new();
        let pending = store.begin(ToolInvocation::new("size", ["app"]));
        let id = store.complete(pending, "honest", 0);
        let mut tampered = store.get(&id).unwrap();
        tampered.output = "dishonest".into();

        let fresh = EvidenceStore::new();
        let refused = fresh.adopt([tampered]);
        assert_eq!(refused, vec![id.clone()]);
        assert!(!fresh.issued(&id));
    }

    #[test]
    fn a_command_line_can_be_retyped_by_hand() {
        let invocation =
            ToolInvocation::new("cargo", ["build", "--config", "profile.release.lto = true"]);
        assert_eq!(invocation.command_line(), "cargo build --config 'profile.release.lto = true'");
    }
}
