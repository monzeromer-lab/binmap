//! Restoring a session a real sweep actually wrote.
//!
//! The unit tests restore a session they built in memory, which cannot catch
//! anything the *writing* path gets wrong. This reads the artifact a real
//! sweep left on disk.

use binmap_session::SessionStore;

#[test]
fn a_session_a_real_sweep_wrote_can_be_read_back() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .unwrap()
        .join("corpus/generics/target/binmap/sessions/generics__generics.binmap.json");

    if !path.exists() {
        eprintln!("no sweep has been run over corpus/generics; nothing to read back");
        return;
    }

    let store = SessionStore::new(path.parent().unwrap());
    let restored = store.read(&path).expect("the artifact parses");

    eprintln!(
        "findings {} of {} | evidence {} of {} | gates {} of {} | runs {} | tampered {}",
        restored.findings.len(),
        restored.artifact.findings.len(),
        restored.evidence.len(),
        restored.artifact.evidence.len(),
        restored.gates.len(),
        restored.artifact.gates.len(),
        restored.artifact.runs.len(),
        restored.tampered_evidence.len(),
    );
    if let Some(notice) = restored.refusal_notice() {
        eprintln!("notice: {notice}");
    }

    assert!(!restored.artifact.evidence.is_empty(), "the sweep recorded evidence");
    assert!(
        restored.tampered_evidence.is_empty(),
        "a session we wrote ourselves failed its own digest check"
    );
    assert_eq!(
        restored.findings.len(),
        restored.artifact.findings.len(),
        "findings the sweep grounded did not survive being read back"
    );
}
