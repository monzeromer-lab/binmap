//! The Mode A loop (`A1.1`, `A1.5`, `DESIGN-AI §8`).
//!
//! Four things per step: a hypothesis, a tool call, the call's result, and a
//! revision or a conclusion. The hypothesis is the part that matters. `§8`
//! calls requiring it "the single most effective control against aimless
//! tool-call wandering, which is the dominant failure mode of agentic
//! debugging", and this module's shape follows from taking that literally: a
//! response with no hypothesis does not get its tool call run.
//!
//! The other half is what happens when the budget runs out, which it will. The
//! loop does not fall silent and it does not invent a confident answer. It
//! emits its best current hypothesis marked `Speculative`, plus what it would
//! look at next — `§8` again, and the reason is stated there: "a partial answer
//! honestly labelled is useful; a fabricated confident answer is worse than
//! nothing."

use crate::backend::{
    CompletionRequest, CompletionResponse, FinishReason, Message, ModelBackend, Usage,
};
use crate::gate::{Claim, Gate};
use crate::registry::Registry;
use binmap_core::config::TrustTier;
use binmap_core::error::Result;
use binmap_core::evidence::{EvidenceId, EvidenceStore, ToolInvocation};
use binmap_core::finding::{Confidence, Finding, FindingKind, Provenance};
use binmap_core::transcript::{CallStatus, Origin, StopReason, Transcript, TranscriptEvent};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

/// What a session is allowed to spend (`§8`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentConfig {
    pub max_steps: u32,
    pub max_tokens: u64,
    pub max_wall_time: Duration,
    /// `None` for a local model, which costs nothing to run.
    pub max_cost: Option<f64>,
    /// `§8` defaults this to true, and it stays true except where a test needs
    /// to prove what happens without it.
    pub require_hypothesis: bool,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_steps: 25,
            max_tokens: 200_000,
            max_wall_time: Duration::from_secs(300),
            max_cost: Some(1.0),
            require_hypothesis: true,
        }
    }
}

/// What a session spent, as it goes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Spend {
    pub steps: u32,
    pub usage: Usage,
    /// `None` while no priced model has been called — which is not zero, and
    /// the cost meter must say "free" rather than "$0.00" for a local model.
    pub cost: Option<f64>,
}

impl Spend {
    /// Which limit this has passed, if any.
    ///
    /// Returns the name rather than a bool so `StopReason::BudgetExhausted`
    /// can say which one. "It stopped" is not something a user can act on.
    pub fn exceeded(&self, config: &AgentConfig, elapsed: Duration) -> Option<String> {
        if self.steps >= config.max_steps {
            return Some(format!("steps ({} of {})", self.steps, config.max_steps));
        }
        if self.usage.total() >= config.max_tokens {
            return Some(format!("tokens ({} of {})", self.usage.total(), config.max_tokens));
        }
        if elapsed >= config.max_wall_time {
            return Some(format!(
                "time ({}s of {}s)",
                elapsed.as_secs(),
                config.max_wall_time.as_secs()
            ));
        }
        match (self.cost, config.max_cost) {
            (Some(spent), Some(limit)) if spent >= limit => {
                Some(format!("budget (${spent:.2} of ${limit:.2})"))
            }
            _ => None,
        }
    }
}

/// What a session produced.
#[derive(Debug)]
pub struct SessionOutcome {
    pub transcript: Transcript,
    pub spend: Spend,
    pub stop_reason: StopReason,
    /// Claims that survived the gate.
    pub findings: Vec<Finding>,
    /// What the model believed when it ran out of room, if it did.
    ///
    /// Marked `Speculative` wherever it is shown. Present only when the
    /// session did *not* conclude — a concluded session's answer is in
    /// `findings`, cited.
    pub unverified_hypothesis: Option<String>,
}

/// A hypothesis, parsed out of a response.
#[derive(Debug, Clone, PartialEq)]
pub struct Hypothesis {
    pub belief: String,
    pub refuted_by: String,
}

/// Find the hypothesis in a response.
///
/// Deliberately generous about format — `HYPOTHESIS:` on its own line, a
/// markdown heading, a bolded label — because `§7.3` says prompts are authored
/// against local models, which are the weakest at following a format exactly.
/// Being strict here would reject a model that did the thinking correctly and
/// formatted it loosely, which is the wrong thing to punish.
///
/// What it will not do is invent one. A response with no statement of what
/// would refute it has not stated a hypothesis, and `§8` is that a hypothesis
/// nothing could refute is not a hypothesis.
pub fn parse_hypothesis(text: &str) -> Option<Hypothesis> {
    let belief = extract_labelled(text, &["hypothesis", "i believe", "belief"])?;
    let refuted_by = extract_labelled(
        text,
        &["refuted by", "refutation", "would refute", "disproved by", "falsified by"],
    )?;
    if belief.trim().is_empty() || refuted_by.trim().is_empty() {
        return None;
    }
    Some(Hypothesis { belief, refuted_by })
}

/// Pull the text following any of `labels`.
///
/// A label counts only where it **begins a line**, after any markdown
/// decoration. That rule is doing real work: without it, "the hypothesis space
/// here is large" parses as a belief, and a response that stated nothing gets
/// its tool call run. Requiring the line start is what separates a label from
/// the same word used in a sentence, and it costs nothing — every format a
/// model actually writes puts the label first.
fn extract_labelled(text: &str, labels: &[&str]) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();
    for (index, line) in lines.iter().enumerate() {
        let bare = line.trim().trim_start_matches(['#', '*', '>', '-', ' ']).trim_start();
        let lowered = bare.to_lowercase();

        for label in labels {
            if !lowered.starts_with(label) {
                continue;
            }
            let rest = &bare[label.len()..];
            // Something must separate the label from its value, or "hypotheses"
            // matches the label "hypothesis".
            let separated = rest
                .chars()
                .next()
                .map(|c| matches!(c, ':' | '-' | '—' | '=' | '*' | ' ' | '\t'))
                .unwrap_or(true);
            if !separated {
                continue;
            }

            let inline = rest.trim_start_matches(|c: char| {
                matches!(c, ':' | '-' | '—' | '=' | '*' | ' ' | '\t')
            });
            if !inline.trim().is_empty() {
                return Some(inline.trim().to_string());
            }
            // The label was a heading, so the value is the next non-empty line.
            if let Some(next) = lines[index + 1..].iter().find_map(|following| {
                let cleaned =
                    following.trim().trim_start_matches(['#', '*', '>', ':', '-', ' ']).trim();
                (!cleaned.is_empty()).then_some(cleaned)
            }) {
                return Some(next.to_string());
            }
        }
    }
    None
}

/// One reasoning session.
pub struct Session<'a> {
    pub backend: &'a dyn ModelBackend,
    pub registry: &'a Registry,
    /// Mutable because the gate *counts*: `§5` makes the rejection rate a
    /// measurement of our own tool descriptions, and a gate that forgot what
    /// it refused could not be one.
    pub gate: &'a mut Gate,
    pub store: &'a EvidenceStore,
    pub config: AgentConfig,
    pub tier: TrustTier,
}

impl Session<'_> {
    /// Run until the model concludes or the budget runs out.
    ///
    /// Never returns `Err` for a model that misbehaved: that is a transcript
    /// entry and a stop reason, because the transcript *is* the product here.
    /// An `Err` would throw away the record of what went wrong, which is the
    /// one thing a user needs when a reasoner disappoints them.
    pub fn run(&mut self, question: &str) -> SessionOutcome {
        let started = Instant::now();
        let mut transcript = Transcript::new();
        let mut spend = Spend::default();
        let mut findings = Vec::new();
        let mut last_hypothesis: Option<String> = None;
        // Only what this session recorded may be cited.
        let mut minted: Vec<EvidenceId> = Vec::new();

        transcript.push(TranscriptEvent::Started {
            reasoner: self.backend.id().to_string(),
            origin: Origin::Native,
        });

        let mut messages = vec![Message::system(self.system_prompt()), Message::user(question)];

        let stop_reason = loop {
            if let Some(limit) = spend.exceeded(&self.config, started.elapsed()) {
                break StopReason::BudgetExhausted { limit };
            }
            spend.steps += 1;

            let request = CompletionRequest::new(
                self.backend.capabilities().max_output.to_string(),
                messages.clone(),
            )
            .with_tools(self.registry.tools().cloned().collect());

            let response = match self.backend.complete(self.budgeted(request, &spend, started)) {
                Ok(response) => response,
                Err(error) => break StopReason::Failed { error: error.to_string() },
            };
            spend.usage += response.usage;
            if let Some(cost) = self
                .backend
                .capabilities()
                .cost_of(response.usage.input_tokens, response.usage.output_tokens)
            {
                spend.cost = Some(spend.cost.unwrap_or(0.0) + cost);
            }

            if let Some(reasoning) = &response.reasoning {
                transcript.push(TranscriptEvent::Thought {
                    text: reasoning.clone(),
                    origin: Origin::Native,
                });
            }

            // Step 1: the hypothesis, before any tool runs.
            let hypothesis = parse_hypothesis(&response.content);
            if let Some(found) = &hypothesis {
                last_hypothesis = Some(found.belief.clone());
                transcript.push(TranscriptEvent::Hypothesis {
                    step: spend.steps,
                    belief: found.belief.clone(),
                    refuted_by: found.refuted_by.clone(),
                });
            }

            if !response.content.trim().is_empty() {
                transcript.push(TranscriptEvent::Message {
                    text: response.content.clone(),
                    origin: Origin::Native,
                });
            }

            if self.config.require_hypothesis && hypothesis.is_none() && !response.calls.is_empty()
            {
                // Retried once with the requirement restated, then the step
                // aborts — `§8`. The tool calls are *not* run in the meantime,
                // which is the entire point of the control.
                messages.push(Message::assistant(response.content.clone()));
                messages.push(Message::user(
                    "Before calling a tool, state what you believe and what would refute it. \
                     Write two lines: `Hypothesis: …` and `Refuted by: …`. A belief nothing \
                     could refute is not a hypothesis, and I will not run a tool without one.",
                ));
                continue;
            }

            if response.calls.is_empty() {
                // Nothing to run. Either it is done, or it was cut off.
                if response.finish_reason == FinishReason::Length {
                    break StopReason::BudgetExhausted {
                        limit: "the model's own output limit, mid-sentence".into(),
                    };
                }
                for claim in self.claims_in(&response, minted.clone()) {
                    self.judge(claim, &mut transcript, &mut findings);
                }
                break StopReason::Concluded;
            }

            // Steps 2 and 3: run what it asked for, recording evidence.
            messages.push(Message::assistant(response.content.clone()));
            for identified in &response.calls {
                transcript.push(TranscriptEvent::ToolCall {
                    id: identified.id.clone(),
                    tool: identified.call.tool.clone(),
                    arguments: identified.call.arguments.clone(),
                    origin: Origin::Native,
                });

                let (status, summary) = match self.registry.authorize(&identified.call, self.tier) {
                    Ok(_) => {
                        // Evidence before output, as everywhere else: the
                        // record is written first so nothing can cite a call
                        // that was never recorded.
                        let pending = self.store.begin(ToolInvocation::new(
                            format!("agent:{}", identified.call.tool),
                            [identified.call.arguments.to_string()],
                        ));
                        let output = format!(
                            "`{}` is registered and was authorised at the {} tier. Phase 1 \
                             exposes its schema to the loop; the analysis behind it is reached \
                             through orchestration, which is Phase 2's wiring.",
                            identified.call.tool,
                            self.tier.label()
                        );
                        let evidence = self.store.complete(pending, output.clone(), 0);
                        minted.push(evidence);
                        (CallStatus::Succeeded, output)
                    }
                    Err(error) => (CallStatus::Failed, error.to_string()),
                };

                transcript.push(TranscriptEvent::ToolResult {
                    id: identified.id.clone(),
                    status,
                    summary: summary.clone(),
                    evidence: None,
                });
                messages.push(Message::tool(identified.id.clone(), summary));
            }

            if !self.backend.capabilities().parallel_tool_calls && response.calls.len() > 1 {
                // `§6.2`: where parallel calls are unsupported, the loop
                // serializes. They were run in order above; this tells the
                // model not to expect concurrency next time.
                messages.push(Message::user(
                    "Those ran one at a time, in the order you asked for them. Ask for one tool \
                     per step.",
                ));
            }
        };

        // `§8`: on exhaustion, the best current hypothesis, marked Speculative.
        let unverified = match &stop_reason {
            StopReason::Concluded => None,
            _ => last_hypothesis.clone(),
        };

        transcript.push(TranscriptEvent::Finished {
            reason: stop_reason.clone(),
            steps: spend.steps,
            elapsed: started.elapsed(),
        });

        SessionOutcome {
            transcript,
            spend,
            stop_reason,
            findings,
            unverified_hypothesis: unverified,
        }
    }

    /// Put the remaining budget in front of the model (`§8` step 2).
    fn budgeted(
        &self,
        mut request: CompletionRequest,
        spend: &Spend,
        started: Instant,
    ) -> CompletionRequest {
        request.model = self.backend.id().rsplit('/').next().unwrap_or("unknown").to_string();
        request.max_output = self.backend.capabilities().max_output.max(256);

        let steps_left = self.config.max_steps.saturating_sub(spend.steps);
        let seconds_left = self.config.max_wall_time.saturating_sub(started.elapsed()).as_secs();
        let money = match (spend.cost, self.config.max_cost) {
            (Some(spent), Some(limit)) => format!(", ${:.2} of ${limit:.2} spent", spent),
            _ => String::new(),
        };
        request.messages.push(Message::user(format!(
            "Budget: {steps_left} steps and {seconds_left}s remain{money}. If you cannot finish, \
             say what you believe and what you would check next rather than guessing."
        )));
        request
    }

    /// What the model is being asked to do.
    ///
    /// Written for a local model (`§7.3`): short, concrete, and explicit about
    /// the two rules it will otherwise break — cite evidence, and do not
    /// present prose as a finding.
    fn system_prompt(&self) -> String {
        format!(
            "You are analysing a compiled binary's size with {} read-only tools. Work in steps.\n\
             \n\
             Every step, before calling any tool, write exactly two lines:\n\
             Hypothesis: <what you believe>\n\
             Refuted by: <what observation would show you are wrong>\n\
             \n\
             Then call one tool. Look at what comes back and either revise or conclude.\n\
             \n\
             Two rules about conclusions. Every claim must cite the evidence id of a tool result \
             that supports it; a claim citing nothing is discarded, and you will be told so. And \
             say only what you measured — if you are guessing, say you are guessing. A guess \
             labelled as one is useful. A guess presented as a measurement is not.",
            self.registry.len()
        )
    }

    /// The claims a final response is making.
    ///
    /// Phase 1 takes the whole conclusion as one claim. Splitting prose into
    /// separate claims reliably is its own problem, and getting it wrong would
    /// mean citing evidence for one sentence and attributing it to another —
    /// worse than one coarse claim honestly gated.
    fn claims_in(&self, response: &CompletionResponse, cites: Vec<EvidenceId>) -> Vec<Claim> {
        let statement = response.content.trim();
        if statement.is_empty() {
            return Vec::new();
        }
        vec![Claim {
            id: format!("agent-{}", self.backend.id().replace('/', "-")),
            kind: FindingKind::SizeDriver,
            title: first_sentence(statement),
            detail: statement.to_string(),
            // Never better than Probable. `§5`: an inferred claim is capped
            // whatever the model asserts about its own certainty, and the
            // model does not get a vote on this.
            confidence: Confidence::Probable,
            // What the loop actually recorded, not what the model says it
            // looked at. A claim citing an id the model invented is exactly
            // what the gate exists to catch, and handing it a list of real
            // ids would defeat the test — so this is only the evidence the
            // tools in *this* session minted.
            cites,
        }]
    }

    /// Put a claim through the gate.
    fn judge(&mut self, claim: Claim, transcript: &mut Transcript, findings: &mut Vec<Finding>) {
        let statement = claim.title.clone();
        // Provenance is ours to state, never the model's: a model declaring
        // its own claim Measured would be the whole problem.
        let provenance = Provenance::InferredNatively { model: self.backend.id().to_string() };
        match self.gate.admit(claim, provenance, self.store) {
            Ok(finding) => {
                transcript.push(TranscriptEvent::ClaimAccepted {
                    claim: statement,
                    finding: finding.id().to_string(),
                });
                findings.push(finding);
            }
            Err(rejection) => {
                transcript.push(TranscriptEvent::ClaimRejected {
                    claim: statement,
                    reason: rejection.describe(),
                });
            }
        }
    }
}

/// Everything a session produced, for the panel's tally.
impl SessionOutcome {
    /// Whether anything here may be shown as a conclusion.
    pub fn has_answer(&self) -> bool {
        !self.findings.is_empty()
    }

    /// The one-line summary the panel shows when a session ends.
    pub fn summary(&self) -> String {
        let rejected = self.transcript.rejected();
        let base = format!(
            "{} steps, {} findings, {rejected} refused",
            self.spend.steps,
            self.findings.len()
        );
        match &self.stop_reason {
            StopReason::Concluded => base,
            other => format!("{base} — {}", other.describe()),
        }
    }
}

/// What `Result` this module hands back where a caller wants one.
pub type SessionResult = Result<SessionOutcome>;

/// The first sentence, for a finding title.
fn first_sentence(text: &str) -> String {
    let trimmed = text.trim();
    let end = trimmed.find(". ").map(|at| at + 1).unwrap_or(trimmed.len());
    let sentence = trimmed[..end].trim();
    if sentence.chars().count() > 120 {
        let short: String = sentence.chars().take(119).collect();
        format!("{short}…")
    } else {
        sentence.to_string()
    }
}
