use super::*;
use crate::artifact::{SessionArtifact, TargetMetadata};
use binmap_core::evidence::{EvidenceStore, ToolInvocation};
use binmap_core::finding::{Confidence, Finding, FindingDraft, FindingKind, Provenance};
use binmap_core::traits::TargetFamily;

fn metadata() -> TargetMetadata {
    TargetMetadata {
        id: "app::app".into(),
        name: "app".into(),
        package: "app".into(),
        family: TargetFamily::Rust,
        commit: Some("abc1234".into()),
        dirty: false,
        root: Some("/home/ada/projects/app".into()),
    }
}

/// A session with one grounded finding, as an engine would produce it.
fn session() -> (SessionArtifact, EvidenceStore) {
    let store = EvidenceStore::new();
    let pending = store.begin(ToolInvocation::new("cargo", ["build", "--release"]));
    let id = store.complete(pending, "   Compiling app v0.1.0 (/home/ada/projects/app)", 0);

    let finding = Finding::new(
        FindingDraft::new("cfg-ols", FindingKind::Configuration, "opt-level=s: measured")
            .cite(id)
            .confidence(Confidence::Certain)
            .provenance(Provenance::Measured),
        &store,
    )
    .unwrap();

    let artifact = SessionArtifact::new(metadata())
        .with_findings(vec![finding])
        .with_evidence(store.records());
    (artifact, store)
}

fn temp_directory() -> tempfile::TempDir {
    tempfile::tempdir().expect("a temporary directory")
}

fn redactor() -> Redactor {
    Redactor::default().with_home("/home/ada").with_username("ada").with_hostname("engine")
}

#[test]
fn a_session_round_trips_through_disk_with_its_findings_intact() {
    let temporary = temp_directory();
    let directory = temporary.path();
    let store = SessionStore::new(directory).with_redactor(redactor());
    let (artifact, _) = session();

    store.save(&artifact).unwrap();
    let restored = store.load("app::app").unwrap().expect("it was saved");

    assert_eq!(restored.findings.len(), 1);
    assert!(restored.tampered_evidence.is_empty());
    assert!(restored.ungrounded_findings.is_empty());
    assert!(restored.refusal_notice().is_none());
    // The evidence store came back populated, so the Inspector has something
    // to show without re-running anything.
    assert_eq!(restored.evidence.len(), 1);
}

#[test]
fn a_session_that_was_never_saved_loads_as_nothing_rather_than_as_an_error() {
    let temporary = temp_directory();
    let directory = temporary.path();
    let store = SessionStore::new(directory);
    assert!(store.load("never::saved").unwrap().is_none());
}

#[test]
fn tampering_with_evidence_on_disk_drops_the_findings_that_cite_it_by_name() {
    let temporary = temp_directory();
    let directory = temporary.path();
    let store = SessionStore::new(directory).with_redactor(redactor());
    let (artifact, _) = session();
    let path = store.save(&artifact).unwrap();

    // Somebody edits the file to say the build printed something it did not.
    let text = std::fs::read_to_string(&path).unwrap();
    let edited = text.replace("Compiling app v0.1.0", "Compiling app v9.9.9");
    std::fs::write(&path, edited).unwrap();

    let restored = store.read(&path).unwrap();
    assert_eq!(restored.tampered_evidence.len(), 1);
    assert!(restored.findings.is_empty(), "a finding may not outlive its evidence");
    assert_eq!(restored.ungrounded_findings, vec!["cfg-ols".to_string()]);
    let notice = restored.refusal_notice().expect("the user is told");
    assert!(notice.contains("cfg-ols"), "{notice}");
}

#[test]
fn an_export_redacts_and_reports_what_it_changed() {
    let temporary = temp_directory();
    let directory = temporary.path();
    let store = SessionStore::new(directory).with_redactor(redactor());
    let (artifact, _) = session();

    let path = directory.join("shared.binmap.json");
    let report = store.export(&artifact, &path).unwrap();
    assert!(!report.is_empty());

    let text = std::fs::read_to_string(&path).unwrap();
    assert!(!text.contains("/home/ada"), "the home directory survived the export");
    assert!(text.contains("~/projects/app"));
}

#[test]
fn an_exported_session_can_actually_be_opened_by_the_person_it_was_sent_to() {
    // This is the point of U13, and it was broken: export redacted the output
    // and kept the old digest, so import refused every record and dropped
    // every finding as ungrounded. The recipient opened an empty session.
    let temporary = temp_directory();
    let directory = temporary.path();
    let store = SessionStore::new(directory).with_redactor(redactor());
    let (artifact, _) = session();
    let path = directory.join("shared.binmap.json");
    store.export(&artifact, &path).unwrap();

    let restored = store.read(&path).unwrap();
    assert!(restored.tampered_evidence.is_empty(), "a redacted record was refused");
    assert_eq!(restored.findings.len(), 1, "the finding did not survive the round trip");
    assert!(restored.refusal_notice().is_none());
}

#[test]
fn an_exported_artifact_admits_that_its_evidence_was_altered() {
    // It opens, and it still says so. A record that was redacted is
    // re-digested over what it now says — otherwise the session is unreadable
    // — and keeps the digest it had, which is how the recipient knows the
    // output is not byte-for-byte what the tool produced.
    let temporary = temp_directory();
    let directory = temporary.path();
    let store = SessionStore::new(directory).with_redactor(redactor());
    let (artifact, _) = session();
    let path = directory.join("shared.binmap.json");
    store.export(&artifact, &path).unwrap();

    let restored = store.read(&path).unwrap();
    assert!(restored.artifact.redacted.is_some(), "the artifact does not admit redaction");
    let described = restored.artifact.redacted.as_ref().unwrap().describe();
    assert!(described.starts_with("Redacted: "), "{described}");

    let altered: Vec<&binmap_core::evidence::Evidence> =
        restored.artifact.evidence.iter().filter(|e| e.was_redacted()).collect();
    assert_eq!(altered.len(), 1, "the altered record does not carry its original digest");
    assert!(altered[0].digest_matches(), "a redacted record must still verify");
    assert_ne!(altered[0].redacted_from.as_deref(), Some(altered[0].digest.as_str()));
}

#[test]
fn a_saved_session_is_not_redacted_because_it_is_the_users_own() {
    // Redacting the user's own session would make the Evidence tab lie to the
    // person who produced it.
    let temporary = temp_directory();
    let directory = temporary.path();
    let store = SessionStore::new(directory).with_redactor(redactor());
    let (artifact, _) = session();

    store.save(&artifact).unwrap();
    let restored = store.load("app::app").unwrap().expect("it was saved");
    assert!(restored.artifact.evidence.iter().all(|e| !e.was_redacted()));
    assert!(restored.artifact.evidence[0].output.contains("/home/ada"));
}

#[test]
fn a_session_from_a_newer_schema_is_refused_with_a_sentence() {
    let temporary = temp_directory();
    let directory = temporary.path();
    let store = SessionStore::new(directory);
    let (mut artifact, _) = session();
    artifact.schema_version = crate::SCHEMA_VERSION + 1;
    artifact.binmap_version = "0.9.0".into();

    let path = directory.join("future.binmap.json");
    std::fs::write(&path, serde_json::to_string(&artifact).unwrap()).unwrap();

    let error = store.read(&path).unwrap_err().to_string();
    assert!(error.contains("0.9.0"), "{error}");
    assert!(error.contains("this build reads up to"), "{error}");
}

#[test]
fn run_state_is_carried_opaquely_and_read_back_in_its_owners_shape() {
    #[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
    struct SomeEngineState {
        measured: usize,
        cancelled: bool,
    }

    let (artifact, _) = session();
    let state = SomeEngineState { measured: 42, cancelled: true };
    let artifact = artifact.with_run("run-1", "sweep", &state).unwrap();

    let restored: SomeEngineState = artifact.run_state("run-1").unwrap().unwrap();
    assert_eq!(restored, state);
    assert!(artifact.run_state::<SomeEngineState>("run-2").is_none());
}

#[test]
fn a_gate_report_citing_evidence_nobody_issued_is_refused() {
    // Gate outcomes were adopted verbatim: a hand-written report could say
    // "TestsPass, 412 passed" and cite an identifier no tool ever produced.
    // They go through the same airlock as findings now.
    use binmap_core::gate::{Gate, GateOutcome, GateResult, VerificationReport};

    let temporary = temp_directory();
    let directory = temporary.path();
    let store = SessionStore::new(directory).with_redactor(redactor());
    let (artifact, _) = session();

    let honest = artifact.evidence[0].id.clone();
    // An identifier that never came from a store. There is no constructor,
    // so this is exactly how one arrives in practice: out of a file.
    let forged: binmap_core::evidence::EvidenceId = serde_json::from_str("\"ev-999999\"").unwrap();

    let artifact = artifact.with_gates(vec![
        VerificationReport {
            candidate: "honest".into(),
            outcomes: vec![
                GateOutcome::new(Gate::TestsPass, GateResult::Passed, "412 passed").citing(honest),
            ],
        },
        VerificationReport {
            candidate: "invented".into(),
            outcomes: vec![
                GateOutcome::new(Gate::TestsPass, GateResult::Passed, "412 passed").citing(forged),
            ],
        },
    ]);

    store.save(&artifact).unwrap();
    let restored = store.load("app::app").unwrap().expect("it was saved");

    assert_eq!(restored.gates.len(), 1, "the invented report was adopted");
    assert_eq!(restored.gates[0].candidate, "honest");
    assert_eq!(restored.refused_gates, 1);
    let notice = restored.refusal_notice().expect("the user is told");
    assert!(notice.contains("never issued"), "{notice}");
}
