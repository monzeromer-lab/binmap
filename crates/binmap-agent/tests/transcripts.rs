//! The transcript, and the properties the panel depends on (`U1.3`, `§9.2`).

use binmap_agent::native::parse_hypothesis;
use binmap_agent::transcript::{CallStatus, Origin, StopReason, Transcript, TranscriptEvent};
use proptest::prelude::*;
use serde_json::json;
use std::time::Duration;

fn a_call(id: &str) -> TranscriptEvent {
    TranscriptEvent::ToolCall {
        id: id.into(),
        tool: "read_symbols".into(),
        arguments: json!({}),
        origin: Origin::Native,
    }
}

#[test]
fn an_update_for_a_known_call_is_applied() {
    let mut transcript = Transcript::new();
    transcript.push(a_call("call_1"));
    assert!(transcript.update_call("call_1", CallStatus::Succeeded, "12 symbols"));
    assert_eq!(transcript.len(), 2);
}

#[test]
fn an_update_for_a_call_nobody_made_is_refused() {
    // ACP sends calls and updates separately. An update for a call we never
    // saw is a protocol error worth noticing, not a silent append that leaves
    // a result floating with nothing above it.
    let mut transcript = Transcript::new();
    transcript.push(a_call("call_1"));
    assert!(!transcript.update_call("call_99", CallStatus::Succeeded, "?"));
    assert_eq!(transcript.len(), 1, "nothing was appended");
}

#[test]
fn only_a_concluded_session_permits_a_confident_conclusion() {
    // §8: a session cut short has, by definition, not finished checking.
    assert!(StopReason::Concluded.permits_confident_conclusion());
    for cut_short in [
        StopReason::BudgetExhausted { limit: "steps".into() },
        StopReason::Cancelled,
        StopReason::Failed { error: "network".into() },
    ] {
        assert!(
            !cut_short.permits_confident_conclusion(),
            "{cut_short:?} must not permit a confident conclusion"
        );
    }
}

#[test]
fn a_stop_reason_always_says_something_a_user_can_read() {
    for reason in [
        StopReason::Concluded,
        StopReason::BudgetExhausted { limit: "steps (25 of 25)".into() },
        StopReason::Cancelled,
        StopReason::Failed { error: "connection refused".into() },
    ] {
        let described = reason.describe();
        assert!(!described.is_empty(), "{reason:?} describes itself as nothing");
        assert!(!described.contains("Object"), "{described}");
    }
    assert!(
        StopReason::BudgetExhausted { limit: "steps (25 of 25)".into() }.describe().contains("25"),
        "the limit that was hit is named"
    );
}

#[test]
fn an_external_claim_is_badged_and_our_own_is_not() {
    // §1 requires the UI to say when grounding is weaker. It cannot if the
    // transcript threw the distinction away.
    assert!(Origin::External.needs_badge());
    assert!(!Origin::Native.needs_badge());

    let mut transcript = Transcript::new();
    transcript.push(TranscriptEvent::Message { text: "hello".into(), origin: Origin::Native });
    assert!(!transcript.has_external_claims());

    transcript.push(TranscriptEvent::Message { text: "hi".into(), origin: Origin::External });
    assert!(transcript.has_external_claims());
}

#[test]
fn what_we_say_ourselves_is_native_whatever_the_reasoner_was() {
    // A gate rejection is *our* statement about an external agent's claim, so
    // badging it as external would attribute our own refusal to them.
    let rejection = TranscriptEvent::ClaimRejected {
        claim: "it is the allocator".into(),
        reason: "cited nothing".into(),
    };
    assert_eq!(rejection.origin(), Origin::Native);

    let finished = TranscriptEvent::Finished {
        reason: StopReason::Concluded,
        steps: 3,
        elapsed: Duration::from_secs(1),
    };
    assert_eq!(finished.origin(), Origin::Native);
}

#[test]
fn the_tally_counts_what_the_gate_did() {
    // §5 makes the rejection rate a measurement of our tool descriptions, so
    // the panel shows it rather than burying it.
    let mut transcript = Transcript::new();
    transcript.push(TranscriptEvent::ClaimAccepted { claim: "a".into(), finding: "f1".into() });
    transcript
        .push(TranscriptEvent::ClaimRejected { claim: "b".into(), reason: "no evidence".into() });
    transcript
        .push(TranscriptEvent::ClaimRejected { claim: "c".into(), reason: "invented".into() });

    assert_eq!(transcript.accepted(), 1);
    assert_eq!(transcript.rejected(), 2);
}

#[test]
fn only_the_finish_event_is_terminal() {
    // A view waiting for the end must not be released by a mid-session event,
    // and must be released by the real one.
    assert!(!a_call("c").is_terminal());
    assert!(
        TranscriptEvent::Finished {
            reason: StopReason::Cancelled,
            steps: 1,
            elapsed: Duration::ZERO
        }
        .is_terminal()
    );
}

#[test]
fn the_stop_reason_is_recoverable_from_a_finished_transcript() {
    let mut transcript = Transcript::new();
    assert!(transcript.stop_reason().is_none(), "an unfinished session has not stopped");

    transcript.push(TranscriptEvent::Finished {
        reason: StopReason::BudgetExhausted { limit: "tokens".into() },
        steps: 9,
        elapsed: Duration::from_secs(2),
    });
    match transcript.stop_reason() {
        Some(StopReason::BudgetExhausted { limit }) => assert_eq!(limit, "tokens"),
        other => panic!("expected the budget reason, got {other:?}"),
    }
}

#[test]
fn a_headline_is_one_line_and_never_empty() {
    // The collapsed view puts these in a list. A multi-line headline breaks
    // the row; an empty one leaves a blank entry the user cannot click.
    let long = "x".repeat(500);
    let events = vec![
        TranscriptEvent::Started { reasoner: "local/qwen3".into(), origin: Origin::Native },
        TranscriptEvent::Hypothesis { step: 1, belief: long.clone(), refuted_by: "y".into() },
        TranscriptEvent::Message {
            text: format!("first line\nsecond line\n{long}"),
            origin: Origin::Native,
        },
        TranscriptEvent::Thought { text: "mulling".into(), origin: Origin::Native },
        a_call("c"),
        TranscriptEvent::ToolResult {
            id: "c".into(),
            status: CallStatus::Failed,
            summary: format!("bad\n{long}"),
            evidence: None,
        },
        TranscriptEvent::ClaimRejected { claim: long.clone(), reason: "no evidence".into() },
        TranscriptEvent::ClaimAccepted { claim: long.clone(), finding: "f".into() },
        TranscriptEvent::Finished {
            reason: StopReason::Concluded,
            steps: 4,
            elapsed: Duration::from_secs(3),
        },
    ];

    for event in events {
        let headline = event.headline();
        assert!(!headline.trim().is_empty(), "{event:?} has an empty headline");
        assert_eq!(headline.lines().count(), 1, "{event:?} spans lines: {headline:?}");
        assert!(
            headline.chars().count() <= 200,
            "{event:?} headline is {} chars",
            headline.chars().count()
        );
    }
}

#[test]
fn a_transcript_survives_a_round_trip_through_json() {
    // The session artifact carries this, so it has to serialise.
    let mut transcript = Transcript::new();
    transcript.push(a_call("c"));
    transcript.push(TranscriptEvent::Finished {
        reason: StopReason::Concluded,
        steps: 1,
        elapsed: Duration::from_millis(1500),
    });

    let json = serde_json::to_string(&transcript).expect("serialises");
    let back: Transcript = serde_json::from_str(&json).expect("deserialises");
    assert_eq!(back.len(), transcript.len());
    assert!(matches!(back.stop_reason(), Some(StopReason::Concluded)));
}

proptest! {
    /// The parser never panics, whatever a model emits.
    ///
    /// It runs on untrusted model output on every step, so a panic here is a
    /// crash triggered by a remote string.
    #[test]
    fn the_hypothesis_parser_never_panics(text in ".{0,400}") {
        let _ = parse_hypothesis(&text);
    }

    /// And it never reports a hypothesis without both halves.
    ///
    /// This is the property the whole control rests on: if a belief can be
    /// found with no refutation, `§8`'s requirement is decorative.
    #[test]
    fn a_hypothesis_always_has_both_a_belief_and_a_refutation(text in ".{0,400}") {
        if let Some(parsed) = parse_hypothesis(&text) {
            prop_assert!(!parsed.belief.trim().is_empty());
            prop_assert!(!parsed.refuted_by.trim().is_empty());
            prop_assert_eq!(parsed.belief.lines().count(), 1);
            prop_assert_eq!(parsed.refuted_by.lines().count(), 1);
        }
    }

    /// Arbitrary unicode, including the multi-byte slicing that a naive
    /// index-based parser gets wrong.
    #[test]
    fn the_parser_handles_multibyte_text(text in "\\PC{0,200}") {
        let _ = parse_hypothesis(&text);
        let decorated = format!("Hypothesis: {text}\nRefuted by: {text}");
        let _ = parse_hypothesis(&decorated);
    }
}

#[test]
fn an_event_with_no_text_still_has_a_headline() {
    // An ACP agent may send an empty chunk. A blank row is something the user
    // can see and cannot click, so the kind stands in for the text.
    for event in [
        TranscriptEvent::Message { text: String::new(), origin: Origin::External },
        TranscriptEvent::Thought { text: "   ".into(), origin: Origin::External },
    ] {
        let headline = event.headline();
        assert!(!headline.trim().is_empty(), "{event:?} produced a blank row");
    }
}
