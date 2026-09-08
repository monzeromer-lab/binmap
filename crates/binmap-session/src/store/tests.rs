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

fn temp_directory(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("binmap-session-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn redactor() -> Redactor {
    Redactor::default().with_home("/home/ada").with_username("ada").with_hostname("engine")
}

#[test]
fn a_session_round_trips_through_disk_with_its_findings_intact() {
    let directory = temp_directory("round-trip");
    let store = SessionStore::new(&directory).with_redactor(redactor());
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

    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn a_session_that_was_never_saved_loads_as_nothing_rather_than_as_an_error() {
    let directory = temp_directory("absent");
    let store = SessionStore::new(&directory);
    assert!(store.load("never::saved").unwrap().is_none());
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn tampering_with_evidence_on_disk_drops_the_findings_that_cite_it_by_name() {
    let directory = temp_directory("tampered");
    let store = SessionStore::new(&directory).with_redactor(redactor());
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

    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn an_export_redacts_and_reports_what_it_changed() {
    let directory = temp_directory("export");
    let store = SessionStore::new(&directory).with_redactor(redactor());
    let (artifact, _) = session();

    let path = directory.join("shared.binmap.json");
    let report = store.export(&artifact, &path).unwrap();
    assert!(!report.is_empty());

    let text = std::fs::read_to_string(&path).unwrap();
    assert!(!text.contains("/home/ada"), "the home directory survived the export");
    assert!(text.contains("~/projects/app"));

    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn an_exported_artifact_admits_that_its_evidence_was_altered() {
    let directory = temp_directory("admits");
    let store = SessionStore::new(&directory).with_redactor(redactor());
    let (artifact, _) = session();
    let path = directory.join("shared.binmap.json");
    store.export(&artifact, &path).unwrap();

    // Reading it back, the digests no longer match — which is correct, and is
    // exactly how the recipient learns the records were edited rather than
    // being handed altered evidence that looks pristine.
    let restored = store.read(&path).unwrap();
    assert!(restored.artifact.redacted.is_some());
    assert_eq!(restored.tampered_evidence.len(), 1);
    let described = restored.artifact.redacted.as_ref().unwrap().describe();
    assert!(described.starts_with("Redacted: "), "{described}");

    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn a_session_from_a_newer_schema_is_refused_with_a_sentence() {
    let directory = temp_directory("newer");
    let store = SessionStore::new(&directory);
    let (mut artifact, _) = session();
    artifact.schema_version = crate::SCHEMA_VERSION + 1;
    artifact.binmap_version = "0.9.0".into();

    let path = directory.join("future.binmap.json");
    std::fs::write(&path, serde_json::to_string(&artifact).unwrap()).unwrap();

    let error = store.read(&path).unwrap_err().to_string();
    assert!(error.contains("0.9.0"), "{error}");
    assert!(error.contains("this build reads up to"), "{error}");

    let _ = std::fs::remove_dir_all(&directory);
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
