//! The Mode A loop's controls (`DESIGN-AI §8`).
//!
//! Three things are being tested here, and they are the three that decide
//! whether a reasoner can be trusted at all:
//!
//! 1. A tool does not run without a hypothesis. `§8` calls this the single
//!    most effective control against aimless tool-call wandering, so a test
//!    that only checked the hypothesis was *recorded* would miss the point —
//!    what matters is that the call did not happen.
//! 2. A budget that runs out produces an honestly labelled partial answer
//!    rather than silence or a confident fabrication.
//! 3. A claim citing evidence nobody issued does not become a finding.

use binmap_agent::backend::{
    CompletionRequest, CompletionResponse, FinishReason, IdentifiedCall, ModelBackend, Usage,
};
use binmap_agent::gate::Gate;
use binmap_agent::native::{AgentConfig, Session, parse_hypothesis};
use binmap_agent::provider::{Capabilities, ToolCallingSupport};
use binmap_agent::registry::{Registry, ToolCall};
use binmap_agent::transcript::{StopReason, TranscriptEvent};
use binmap_core::config::TrustTier;
use binmap_core::evidence::EvidenceStore;
use serde_json::json;
use std::sync::Mutex;

/// A backend that replies from a script and records what it was asked.
struct Scripted {
    replies: Mutex<std::collections::VecDeque<CompletionResponse>>,
    asked: Mutex<usize>,
    capabilities: Capabilities,
}

impl Scripted {
    fn new(replies: Vec<CompletionResponse>) -> Self {
        Self {
            replies: Mutex::new(replies.into()),
            asked: Mutex::new(0),
            capabilities: Capabilities {
                context_window: 32_768,
                max_output: 4096,
                tool_calling: ToolCallingSupport::Native,
                parallel_tool_calls: false,
                reasoning_effort: false,
                vision: false,
                cost_per_mtok: None,
            },
        }
    }

    fn times_asked(&self) -> usize {
        *self.asked.lock().unwrap()
    }
}

impl ModelBackend for Scripted {
    fn id(&self) -> &str {
        "test/scripted"
    }

    fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    fn complete(&self, _request: CompletionRequest) -> binmap_core::Result<CompletionResponse> {
        *self.asked.lock().unwrap() += 1;
        self.replies
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| binmap_core::Error::Other("the script ran out".into()))
    }
}

fn says(content: &str) -> CompletionResponse {
    CompletionResponse {
        content: content.into(),
        reasoning: None,
        calls: Vec::new(),
        usage: Usage { input_tokens: 10, output_tokens: 5 },
        finish_reason: FinishReason::Stop,
    }
}

fn says_and_calls(content: &str, tool: &str) -> CompletionResponse {
    CompletionResponse {
        content: content.into(),
        reasoning: None,
        calls: vec![IdentifiedCall {
            id: "call_1".into(),
            call: ToolCall { tool: tool.into(), arguments: json!({}) },
        }],
        usage: Usage { input_tokens: 10, output_tokens: 5 },
        finish_reason: FinishReason::ToolCalls,
    }
}

/// The name of a tool that really is registered, so authorisation is not what
/// a test is accidentally measuring.
fn a_real_tool(registry: &Registry) -> String {
    registry.tools().next().expect("Phase 1 registers tools").name.clone()
}

#[test]
fn a_tool_call_without_a_hypothesis_does_not_run() {
    // The control that matters. The model asks for a tool and states no
    // belief; the loop must refuse to run it and say why.
    let registry = Registry::phase_one();
    let tool = a_real_tool(&registry);
    let store = EvidenceStore::new();
    let mut gate = Gate::new();

    let backend = Scripted::new(vec![
        says_and_calls("Let me just check something.", &tool),
        // After being told, it complies and concludes.
        says("Hypothesis: it is the formatting machinery\nRefuted by: no fmt symbols present"),
    ]);

    let mut session = Session {
        backend: &backend,
        registry: &registry,
        gate: &mut gate,
        store: &store,
        config: AgentConfig::default(),
        tier: TrustTier::Observe,
    };
    let outcome = session.run("why is this binary large?");

    let ran_a_tool = outcome
        .transcript
        .events()
        .iter()
        .any(|event| matches!(event, TranscriptEvent::ToolCall { .. }));
    assert!(
        !ran_a_tool,
        "a tool ran without a hypothesis, which is the failure §8 exists to prevent"
    );
    assert_eq!(backend.times_asked(), 2, "the model was asked again rather than the step aborting");
    assert!(store.records().is_empty(), "and nothing was recorded, because nothing ran");
}

#[test]
fn a_tool_call_with_a_hypothesis_does_run() {
    // The complement. A control that blocked everything would pass the test
    // above while making the loop useless.
    let registry = Registry::phase_one();
    let tool = a_real_tool(&registry);
    let store = EvidenceStore::new();
    let mut gate = Gate::new();

    let backend = Scripted::new(vec![
        says_and_calls(
            "Hypothesis: formatting dominates\nRefuted by: no core::fmt symbols in the table",
            &tool,
        ),
        says("Hypothesis: confirmed\nRefuted by: nothing further"),
    ]);

    let mut session = Session {
        backend: &backend,
        registry: &registry,
        gate: &mut gate,
        store: &store,
        config: AgentConfig::default(),
        tier: TrustTier::Observe,
    };
    let outcome = session.run("why is this binary large?");

    let calls: Vec<&TranscriptEvent> = outcome
        .transcript
        .events()
        .iter()
        .filter(|event| matches!(event, TranscriptEvent::ToolCall { .. }))
        .collect();
    assert_eq!(calls.len(), 1, "the call should have run");
    assert!(!store.records().is_empty(), "and it should have recorded evidence");

    let hypotheses = outcome
        .transcript
        .events()
        .iter()
        .filter(|event| matches!(event, TranscriptEvent::Hypothesis { .. }))
        .count();
    assert!(hypotheses >= 1, "the hypothesis is in the transcript for the user to read");
}

#[test]
fn a_hypothesis_is_not_required_when_it_is_turned_off() {
    let registry = Registry::phase_one();
    let tool = a_real_tool(&registry);
    let store = EvidenceStore::new();
    let mut gate = Gate::new();

    let backend = Scripted::new(vec![says_and_calls("just looking", &tool), says("done")]);
    let mut session = Session {
        backend: &backend,
        registry: &registry,
        gate: &mut gate,
        store: &store,
        config: AgentConfig { require_hypothesis: false, ..AgentConfig::default() },
        tier: TrustTier::Observe,
    };
    let outcome = session.run("go");

    assert!(
        outcome
            .transcript
            .events()
            .iter()
            .any(|event| matches!(event, TranscriptEvent::ToolCall { .. })),
        "with the requirement off, the call runs"
    );
}

#[test]
fn running_out_of_steps_yields_the_best_hypothesis_marked_unverified() {
    // §8: "a partial answer honestly labelled is useful; a fabricated
    // confident answer is worse than nothing."
    let registry = Registry::phase_one();
    let tool = a_real_tool(&registry);
    let store = EvidenceStore::new();
    let mut gate = Gate::new();

    // Always asks for another tool, never concludes.
    let backend = Scripted::new(
        (0..10)
            .map(|step| {
                says_and_calls(
                    &format!(
                        "Hypothesis: driver number {step} is the cause\nRefuted by: its bytes \
                         being small"
                    ),
                    &tool,
                )
            })
            .collect(),
    );

    let mut session = Session {
        backend: &backend,
        registry: &registry,
        gate: &mut gate,
        store: &store,
        config: AgentConfig { max_steps: 3, ..AgentConfig::default() },
        tier: TrustTier::Observe,
    };
    let outcome = session.run("why?");

    match &outcome.stop_reason {
        StopReason::BudgetExhausted { limit } => {
            assert!(limit.contains("steps"), "it should say which budget: {limit}");
            assert!(limit.contains('3'), "and what the limit was: {limit}");
        }
        other => panic!("expected the step budget to stop it, got {other:?}"),
    }
    assert_eq!(outcome.spend.steps, 3, "it stopped at the limit, not past it");
    assert!(
        outcome.unverified_hypothesis.is_some(),
        "an exhausted session must still say what it believed"
    );
    assert!(
        outcome.findings.is_empty(),
        "and must not present it as a finding, which would be the confident fabrication"
    );
}

#[test]
fn a_concluded_session_has_no_unverified_hypothesis_left_over() {
    // The complement: a session that finished has its answer in `findings`,
    // cited. Reporting a leftover speculation too would double-count it.
    let registry = Registry::phase_one();
    let store = EvidenceStore::new();
    let mut gate = Gate::new();

    let backend = Scripted::new(vec![says(
        "Hypothesis: nothing is wrong\nRefuted by: a large driver appearing",
    )]);
    let mut session = Session {
        backend: &backend,
        registry: &registry,
        gate: &mut gate,
        store: &store,
        config: AgentConfig::default(),
        tier: TrustTier::Observe,
    };
    let outcome = session.run("go");

    assert_eq!(outcome.stop_reason, StopReason::Concluded);
    assert!(outcome.unverified_hypothesis.is_none());
}

#[test]
fn a_conclusion_citing_nothing_is_refused() {
    // The gate. A session that ran no tools has no evidence, so its conclusion
    // is ungrounded by construction — and must not become a finding.
    let registry = Registry::phase_one();
    let store = EvidenceStore::new();
    let mut gate = Gate::new();

    let backend = Scripted::new(vec![says(
        "Hypothesis: the formatting machinery is 40% of this binary\nRefuted by: fmt symbols \
         being small",
    )]);
    let mut session = Session {
        backend: &backend,
        registry: &registry,
        gate: &mut gate,
        store: &store,
        config: AgentConfig::default(),
        tier: TrustTier::Observe,
    };
    let outcome = session.run("go");

    assert!(
        outcome.findings.is_empty(),
        "a claim with no evidence behind it is not a weak finding, it is not a finding"
    );
    assert_eq!(outcome.transcript.rejected(), 1, "and the refusal is in the transcript");

    let named = outcome.transcript.events().iter().any(|event| {
        matches!(event, TranscriptEvent::ClaimRejected { reason, .. } if !reason.is_empty())
    });
    assert!(named, "§5 makes the rejection rate a measurement of us, so the reason is shown");
}

#[test]
fn a_conclusion_citing_real_evidence_becomes_a_finding() {
    let registry = Registry::phase_one();
    let tool = a_real_tool(&registry);
    let store = EvidenceStore::new();
    let mut gate = Gate::new();

    let backend = Scripted::new(vec![
        says_and_calls("Hypothesis: formatting dominates\nRefuted by: no fmt symbols", &tool),
        says("Hypothesis: formatting accounts for most of it\nRefuted by: nothing remaining"),
    ]);
    let mut session = Session {
        backend: &backend,
        registry: &registry,
        gate: &mut gate,
        store: &store,
        config: AgentConfig::default(),
        tier: TrustTier::Observe,
    };
    let outcome = session.run("go");

    assert_eq!(outcome.findings.len(), 1, "a grounded claim survives: {outcome:?}");
    assert_eq!(outcome.transcript.accepted(), 1);

    // And it is labelled as inferred by a model, never as measured.
    let finding = &outcome.findings[0];
    assert_eq!(finding.provenance().glyph(), '◆', "inferred, not measured");
}

#[test]
fn a_backend_that_fails_leaves_a_transcript_rather_than_an_error() {
    // The transcript *is* the product here. An `Err` would throw away the
    // record of what went wrong, which is the one thing a user needs.
    let registry = Registry::phase_one();
    let store = EvidenceStore::new();
    let mut gate = Gate::new();

    let backend = Scripted::new(Vec::new()); // the script is empty: the first ask fails
    let mut session = Session {
        backend: &backend,
        registry: &registry,
        gate: &mut gate,
        store: &store,
        config: AgentConfig::default(),
        tier: TrustTier::Observe,
    };
    let outcome = session.run("go");

    match &outcome.stop_reason {
        StopReason::Failed { error } => assert!(error.contains("script"), "{error}"),
        other => panic!("expected a failure, got {other:?}"),
    }
    assert!(!outcome.transcript.is_empty(), "the transcript survives the failure");
    assert!(
        outcome.transcript.events().last().expect("events").is_terminal(),
        "and it ends with a terminal event, so a waiting view is released"
    );
}

#[test]
fn a_session_against_the_null_backend_says_there_is_no_reasoner() {
    // The configuration the whole suite runs under. It must fail with a
    // sentence a user can act on, not an empty panel.
    let registry = Registry::phase_one();
    let store = EvidenceStore::new();
    let mut gate = Gate::new();
    let backend = binmap_agent::NullBackend::new();

    let mut session = Session {
        backend: &backend,
        registry: &registry,
        gate: &mut gate,
        store: &store,
        config: AgentConfig::default(),
        tier: TrustTier::Observe,
    };
    let outcome = session.run("go");

    match &outcome.stop_reason {
        StopReason::Failed { error } => {
            assert!(error.contains("no reasoner"), "{error}");
            assert!(
                error.contains("without a model"),
                "and it should say the deterministic path still works: {error}"
            );
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
}

// --- the hypothesis parser, which the control above depends on entirely ---

#[test]
fn a_plainly_formatted_hypothesis_is_found() {
    let parsed = parse_hypothesis(
        "Hypothesis: core::fmt dominates the binary\nRefuted by: fmt symbols being under 1KB",
    )
    .expect("the format the prompt asks for");
    assert_eq!(parsed.belief, "core::fmt dominates the binary");
    assert_eq!(parsed.refuted_by, "fmt symbols being under 1KB");
}

#[test]
fn a_loosely_formatted_hypothesis_is_still_found() {
    // §7.3: prompts are authored against local models, which are the weakest
    // at following a format exactly. Rejecting a model that did the thinking
    // and formatted it loosely punishes the wrong thing.
    let variants = [
        "## Hypothesis\ncore::fmt dominates\n## Refuted by\nsmall fmt symbols",
        "**Hypothesis:** core::fmt dominates\n**Refuted by:** small fmt symbols",
        "I believe - core::fmt dominates\nWould refute: small fmt symbols",
        "hypothesis = core::fmt dominates\nrefutation = small fmt symbols",
    ];
    for variant in variants {
        let parsed =
            parse_hypothesis(variant).unwrap_or_else(|| panic!("should parse:\n{variant}"));
        assert!(
            parsed.belief.to_lowercase().contains("fmt"),
            "belief from {variant:?} was {:?}",
            parsed.belief
        );
        assert!(!parsed.refuted_by.is_empty());
    }
}

#[test]
fn a_belief_with_no_refutation_is_not_a_hypothesis() {
    // §8: a hypothesis nothing could refute is not a hypothesis. Accepting a
    // bare assertion would turn the control into a formatting check.
    assert!(parse_hypothesis("Hypothesis: it is just big").is_none());
    assert!(parse_hypothesis("I believe the formatting machinery is at fault").is_none());
}

#[test]
fn prose_that_merely_mentions_the_word_is_not_a_hypothesis() {
    // The failure this guards: "the hypothesis space is large" read as a
    // hypothesis, letting a tool run on a response that stated nothing.
    assert!(parse_hypothesis("The hypothesis space here is large and hard to search").is_none());
    assert!(parse_hypothesis("").is_none());
    assert!(parse_hypothesis("Let me look at the symbol table first.").is_none());
}

#[test]
fn a_label_only_counts_where_it_begins_a_line() {
    // The adversarial case for a generous parser: prose that mentions the word
    // and, separately, a real refutation line. Without the line-start rule the
    // belief is "space here is large" — garbage — and the tool call runs on a
    // response that stated nothing.
    let parsed = parse_hypothesis(
        "The hypothesis space here is large and hard to search.\nRefuted by: nothing at all",
    );
    assert!(parsed.is_none(), "a mention mid-sentence is not a label, got {parsed:?}");
}

#[test]
fn a_longer_word_starting_with_a_label_is_not_the_label() {
    // "Hypotheses" is not "Hypothesis:". Without a separator check the plural
    // matches and the belief becomes the tail of the word.
    assert!(
        parse_hypothesis("Hypotheses abound here\nRefuted by: nothing").is_none(),
        "the plural is not the label"
    );
}

#[test]
fn a_quoted_or_bulleted_hypothesis_is_still_found() {
    // Markdown decoration before the label is decoration, not absence.
    for variant in [
        "- Hypothesis: fmt dominates\n- Refuted by: small fmt symbols",
        "> Hypothesis: fmt dominates\n> Refuted by: small fmt symbols",
    ] {
        let parsed =
            parse_hypothesis(variant).unwrap_or_else(|| panic!("should parse:\n{variant}"));
        assert_eq!(parsed.belief, "fmt dominates", "from {variant:?}");
        assert_eq!(parsed.refuted_by, "small fmt symbols");
    }
}

#[test]
fn the_loops_default_budget_is_the_one_the_meter_draws() {
    // Two copies of this number would drift, and the meter would quietly
    // describe a budget that was not the one being enforced.
    assert_eq!(
        AgentConfig::default().max_steps,
        binmap_core::transcript::DEFAULT_MAX_STEPS,
        "the loop and the meter must agree on the step budget"
    );
}
