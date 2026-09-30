//! Mode B's permission ladder and filesystem scope (`A2.1`, `A2.4`, `§9.3`).
//!
//! An external agent is a subprocess with capabilities, asking the *client* to
//! act on its behalf. Everything here is about what we refuse, because the
//! agent is the one caller we did not write and every assumption about its
//! behaviour is one it will eventually violate.

use binmap_agent::acp::{
    AgentCommand, Decision, Requested, decide, judge_path, transcript_event, within_project,
};
use binmap_core::config::TrustTier;
use binmap_core::transcript::{CallStatus, Origin, TranscriptEvent};
use serde_json::json;

// --- the permission ladder (§9.3) ------------------------------------------

#[test]
fn reading_is_permitted_at_every_tier() {
    // Every tier's floor is Observe, and Observe measures and explains.
    for tier in TrustTier::ALL {
        assert_eq!(decide(tier, Requested::Read), Decision::Allow, "{tier:?}");
    }
}

#[test]
fn observe_refuses_every_write_without_asking() {
    // "Auto-deny anything beyond read-only." Not a prompt: at Observe the user
    // has already said no.
    for requested in [Requested::BuildConfiguration, Requested::SourceEdit, Requested::Execute] {
        let decision = decide(TrustTier::Observe, requested);
        assert!(matches!(decision, Decision::Refuse { .. }), "{requested:?} → {decision:?}");
        assert!(!decision.needs_a_person(), "Observe does not ask");
    }
}

#[test]
fn propose_asks_before_anything_writes() {
    // "Auto-allow read-only; prompt for anything that writes."
    for requested in [Requested::BuildConfiguration, Requested::SourceEdit, Requested::Execute] {
        let decision = decide(TrustTier::Propose, requested);
        assert!(decision.needs_a_person(), "{requested:?} → {decision:?}");
    }
}

#[test]
fn tune_writes_build_configuration_and_asks_about_source() {
    // "Auto-allow build-config changes; prompt for source edits." The
    // distinction is the whole reason Tune exists as a separate tier.
    assert_eq!(decide(TrustTier::Tune, Requested::BuildConfiguration), Decision::Allow);
    assert!(decide(TrustTier::Tune, Requested::SourceEdit).needs_a_person());
    assert!(
        decide(TrustTier::Tune, Requested::Execute).needs_a_person(),
        "running a command is not a build-configuration change"
    );
}

#[test]
fn autonomous_allows_within_the_workspace() {
    // "Auto-allow within the workspace; still gated by the verifier."
    for requested in [Requested::BuildConfiguration, Requested::SourceEdit, Requested::Execute] {
        assert_eq!(decide(TrustTier::Autonomous, requested), Decision::Allow, "{requested:?}");
    }
}

#[test]
fn an_unrecognised_request_is_never_auto_allowed() {
    // A protocol extension we have not seen is exactly the thing not to grant
    // blind — including at Autonomous, where everything else is allowed.
    for tier in TrustTier::ALL {
        let decision = decide(tier, Requested::Unknown);
        assert!(!decision.is_allowed(), "{tier:?} auto-allowed something unknown");
    }
    assert!(decide(TrustTier::Autonomous, Requested::Unknown).needs_a_person());
}

#[test]
fn every_refusal_and_prompt_says_which_tier_decided_it() {
    // A user seeing "denied" with no reason cannot tell whether to raise the
    // tier or to stop.
    for tier in TrustTier::ALL {
        for requested in [
            Requested::BuildConfiguration,
            Requested::SourceEdit,
            Requested::Execute,
            Requested::Unknown,
        ] {
            match decide(tier, requested) {
                Decision::Allow => {}
                Decision::Ask { because } | Decision::Refuse { because } => {
                    assert!(!because.is_empty(), "{tier:?}/{requested:?} gives no reason");
                }
            }
        }
    }
    // And a tier that could be raised says so.
    match decide(TrustTier::Observe, Requested::SourceEdit) {
        Decision::Refuse { because } => assert!(because.contains("Observe"), "{because}"),
        other => panic!("{other:?}"),
    }
}

// --- filesystem scope (§9.3) ------------------------------------------------

#[test]
fn a_path_inside_the_project_is_allowed_for_reading() {
    let scratch = tempfile::tempdir().unwrap();
    let inside = scratch.path().join("src");
    std::fs::create_dir_all(&inside).unwrap();
    std::fs::write(inside.join("main.rs"), "fn main() {}").unwrap();

    assert!(within_project(scratch.path(), &inside.join("main.rs")));
    assert_eq!(judge_path(scratch.path(), &inside.join("main.rs"), false), Decision::Allow);
}

#[test]
fn an_agent_reaching_outside_the_project_is_refused_not_prompted() {
    // §9.3 verbatim: "An external agent asking to read ~/.ssh/id_rsa should
    // get a refusal and a visible warning, not a permission prompt." A prompt
    // makes it the user's mistake to click through.
    let scratch = tempfile::tempdir().unwrap();

    let decision = judge_path(scratch.path(), Path::new("/etc/passwd"), false);
    assert!(matches!(decision, Decision::Refuse { .. }), "{decision:?}");
    assert!(!decision.needs_a_person(), "this is not a decision to delegate");
    match decision {
        Decision::Refuse { because } => {
            assert!(because.contains("outside the project"), "{because}");
            assert!(because.contains("in a hurry"), "and says why it is not a prompt: {because}");
        }
        other => panic!("{other:?}"),
    }
}

use std::path::Path;

#[test]
fn a_traversal_out_of_the_project_is_refused() {
    // `project/../../../etc/passwd` is inside the project only if nobody
    // resolves it.
    let scratch = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(scratch.path().join("src")).unwrap();

    let traversal = scratch.path().join("src/../../../../etc/passwd");
    assert!(!within_project(scratch.path(), &traversal), "traversal escaped");
}

#[test]
fn a_file_that_does_not_exist_yet_is_still_scoped() {
    // A write is exactly the case where the path cannot be canonicalized,
    // which is where a naive check falls open.
    let scratch = tempfile::tempdir().unwrap();

    assert!(
        within_project(scratch.path(), &scratch.path().join("new/file.rs")),
        "a new file inside the project is inside it"
    );
    assert!(
        !within_project(scratch.path(), &scratch.path().join("../escaped/file.rs")),
        "a new file outside it is not"
    );
}

#[test]
fn a_traversal_hidden_in_a_path_that_does_not_exist_is_refused() {
    // The subtle version: the tail cannot be canonicalized, so `..` inside it
    // would be normalised away by a resolver that only looked at the existing
    // prefix — and would escape the moment the file was created.
    let scratch = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(scratch.path().join("src")).unwrap();

    let sneaky = scratch.path().join("src/does-not-exist/../../../../../tmp/evil");
    assert!(!within_project(scratch.path(), &sneaky), "a `..` in the unresolved tail escaped");
}

#[test]
fn a_write_inside_the_project_is_asked_about_rather_than_allowed() {
    // Being inside the project is necessary, not sufficient: the tier decides
    // whether a write happens, and the path check only decides whether the
    // question is worth asking.
    let scratch = tempfile::tempdir().unwrap();
    let decision = judge_path(scratch.path(), &scratch.path().join("Cargo.toml"), true);
    assert!(decision.needs_a_person(), "{decision:?}");
}

// --- launching agents (§2) --------------------------------------------------

#[test]
fn launch_commands_are_data_rather_than_hardcoded() {
    // §2: "Do not hardcode agent launch commands." Some use a subcommand,
    // some a flag, some a separate adapter binary.
    let known = AgentCommand::known();
    assert!(known.len() >= 4, "the ecosystem is the point, not two agents");

    let patterns: std::collections::BTreeSet<bool> =
        known.iter().map(|agent| agent.arguments.is_empty()).collect();
    assert_eq!(patterns.len(), 2, "both launch patterns are represented");

    let gemini = known.iter().find(|agent| agent.id == "gemini").expect("gemini");
    assert_eq!(gemini.arguments, vec!["--experimental-acp"], "a flag");
    let goose = known.iter().find(|agent| agent.id == "goose").expect("goose");
    assert_eq!(goose.arguments, vec!["acp"], "a subcommand");
}

#[test]
fn an_agent_that_is_not_installed_reports_itself_as_such() {
    // The picker shows installed or install, so a user is never offered a
    // reasoner that cannot start.
    let absent = AgentCommand {
        id: "nope".into(),
        display: "Not Installed".into(),
        program: "definitely-not-a-real-program-xyzzy".into(),
        arguments: Vec::new(),
    };
    assert!(!absent.is_installed());

    // And something that certainly is.
    let present = AgentCommand {
        id: "sh".into(),
        display: "sh".into(),
        program: "sh".into(),
        arguments: Vec::new(),
    };
    assert!(present.is_installed(), "sh is on the path of any machine running this");
}

// --- session/update → transcript (§9.2) -------------------------------------

#[test]
fn a_message_chunk_becomes_a_transcript_message_marked_external() {
    // §1: an external agent's grounding guarantee is weaker, and the UI must
    // say so. It cannot if the origin is lost here.
    let event = transcript_event(&json!({
        "sessionUpdate": "agent_message_chunk",
        "content": {"type": "text", "text": "looking at the symbol table"}
    }))
    .expect("a message");

    match event {
        TranscriptEvent::Message { text, origin } => {
            assert_eq!(text, "looking at the symbol table");
            assert_eq!(origin, Origin::External);
            assert!(origin.needs_badge());
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_thought_chunk_is_a_thought_rather_than_a_claim() {
    let event = transcript_event(&json!({
        "sessionUpdate": "agent_thought_chunk",
        "content": {"type": "text", "text": "mulling"}
    }))
    .expect("a thought");
    assert!(matches!(event, TranscriptEvent::Thought { .. }), "{event:?}");
}

#[test]
fn a_tool_call_and_its_update_share_an_identifier() {
    // ACP sends them separately, so the transcript has to find the earlier
    // entry — which it can only do if the id survives both translations.
    let call = transcript_event(&json!({
        "sessionUpdate": "tool_call",
        "toolCallId": "call_7",
        "title": "read_symbols",
        "rawInput": {"target": "app"}
    }))
    .expect("a call");
    let update = transcript_event(&json!({
        "sessionUpdate": "tool_call_update",
        "toolCallId": "call_7",
        "status": "completed",
        "content": {"type": "text", "text": "412 symbols"}
    }))
    .expect("an update");

    match (call, update) {
        (
            TranscriptEvent::ToolCall { id: called, tool, arguments, origin },
            TranscriptEvent::ToolResult { id: updated, status, summary, .. },
        ) => {
            assert_eq!(called, updated, "the identifier survives both translations");
            assert_eq!(tool, "read_symbols");
            assert_eq!(arguments["target"], "app");
            assert_eq!(origin, Origin::External);
            assert_eq!(status, CallStatus::Succeeded);
            assert_eq!(summary, "412 symbols");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_failed_tool_call_is_marked_failed() {
    let event = transcript_event(&json!({
        "sessionUpdate": "tool_call_update",
        "toolCallId": "c",
        "status": "failed"
    }))
    .expect("an update");
    match event {
        TranscriptEvent::ToolResult { status, .. } => assert_eq!(status, CallStatus::Failed),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_plan_is_shown_rather_than_dropped() {
    // It is the closest thing Mode B has to Mode A's hypothesis, and §1 says
    // the UI must be able to show that the guarantee is weaker — which it
    // cannot do by showing nothing.
    let event = transcript_event(&json!({
        "sessionUpdate": "plan",
        "entries": [
            {"content": "read the symbol table", "status": "completed"},
            {"content": "group the generics", "status": "pending"}
        ]
    }))
    .expect("a plan");

    match event {
        TranscriptEvent::Message { text, origin } => {
            assert!(text.contains("read the symbol table"), "{text}");
            assert!(text.contains("completed"), "the status is kept: {text}");
            assert_eq!(origin, Origin::External);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn content_arriving_as_a_list_of_blocks_is_joined() {
    // ACP's content is a block or a list of them, and an agent may stream
    // either.
    let event = transcript_event(&json!({
        "sessionUpdate": "agent_message_chunk",
        "content": [{"type": "text", "text": "one "}, {"type": "text", "text": "two"}]
    }))
    .expect("a message");
    match event {
        TranscriptEvent::Message { text, .. } => assert_eq!(text, "one two"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn an_update_kind_this_version_does_not_know_is_skipped_rather_than_fatal() {
    // The protocol moves. An unknown update must not end the session.
    assert!(transcript_event(&json!({"sessionUpdate": "something_new_in_v3"})).is_none());
    assert!(transcript_event(&json!({"not": "an update"})).is_none());
    assert!(transcript_event(&json!("nonsense")).is_none());
}
