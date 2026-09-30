//! What a reasoning session did, as a sequence of events (`U1.3`, `AI.9`).
//!
//! This lives in `binmap-core` rather than beside the loop that produces it,
//! for the same reason `Attribution` does: `§2.4` allows the interface to
//! depend on `binmap-core` and `binmap-session` and nothing else, so the Agent
//! panel can only render a transcript if its *shape* is here. The loop, the
//! providers and the backends stay in `binmap-agent`, which the interface may
//! not reach — rendering a session costs no dependency on the crate that ran
//! one.
//!
//! The enum deliberately mirrors ACP's `session/update` notifications —
//! message chunks, thought chunks, tool calls, tool-call updates, plans —
//! because `DESIGN-AI §9.2` asks for one transcript view that renders both
//! modes. Mode A is then the easy case rather than the special case, and
//! Phase 2's ACP agents map onto a view that already exists.
//!
//! Two things here are not ACP's and are load-bearing:
//!
//! - `Hypothesis` has no ACP equivalent, because ACP agents are not required
//!   to state one. It is Mode A's central control (`§8`), so the transcript
//!   has to be able to show it — and its absence in Mode B is exactly the
//!   "weaker grounding guarantee" the UI is required to admit to (`§1`).
//! - `origin` distinguishes what we drove from what an external agent told
//!   us. `§1` requires the UI to say so, and it cannot if the transcript has
//!   thrown the distinction away.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Who produced an entry.
///
/// Not cosmetic. A claim from an external agent was reached by a loop we did
/// not control, with tools we did not necessarily observe, so it carries a
/// weaker guarantee than one from our own loop and the interface must be able
/// to badge it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Origin {
    /// Our own loop, whose every step we chose.
    Native,
    /// An external agent over ACP. Shown with a badge.
    External,
}

impl Origin {
    pub fn label(self) -> &'static str {
        match self {
            Origin::Native => "native",
            Origin::External => "external",
        }
    }

    /// Whether the interface must badge this as externally reached.
    pub fn needs_badge(self) -> bool {
        matches!(self, Origin::External)
    }
}

/// How a tool call is going.
///
/// ACP sends a tool call and then updates it, so a transcript entry is not
/// immutable and the view has to be able to find the entry again — hence the
/// call id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CallStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
}

impl CallStatus {
    pub fn label(self) -> &'static str {
        match self {
            CallStatus::Pending => "pending",
            CallStatus::Running => "running",
            CallStatus::Succeeded => "succeeded",
            CallStatus::Failed => "failed",
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, CallStatus::Succeeded | CallStatus::Failed)
    }
}

/// Why a session stopped.
///
/// A session that ran out of budget is not a session that failed, and the
/// difference decides what the interface may present as an answer. `§8` is
/// explicit that budget exhaustion yields a best current hypothesis marked
/// `Speculative`, which is only possible if the reason survives.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StopReason {
    /// The model said it was finished.
    Concluded,
    /// Out of steps, tokens, time or money. Carries which one, because
    /// "it stopped" is not an answer a user can act on.
    BudgetExhausted { limit: String },
    /// The user stopped it.
    Cancelled,
    /// Something broke.
    Failed { error: String },
}

impl StopReason {
    /// Whether anything concluded here may be presented as more than a guess.
    ///
    /// Only a session that actually finished may. A session cut short mid-way
    /// has, by definition, not finished checking.
    pub fn permits_confident_conclusion(&self) -> bool {
        matches!(self, StopReason::Concluded)
    }

    pub fn describe(&self) -> String {
        match self {
            StopReason::Concluded => "concluded".into(),
            StopReason::BudgetExhausted { limit } => format!("out of {limit}"),
            StopReason::Cancelled => "cancelled".into(),
            StopReason::Failed { error } => format!("failed: {error}"),
        }
    }
}

/// One entry in a reasoning transcript.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TranscriptEvent {
    /// The session began, against a named reasoner.
    Started { reasoner: String, origin: Origin },

    /// What the model believes, and what would refute it (`§8` step 1).
    ///
    /// Mode A only. `refuted_by` is not decoration: a hypothesis nothing
    /// could refute is not a hypothesis, and holding the model to naming a
    /// refutation is what keeps the loop from wandering.
    Hypothesis { step: u32, belief: String, refuted_by: String },

    /// Prose from the model. ACP's `agent_message_chunk`.
    ///
    /// Never becomes a `Finding`. `§5` is explicit that commentary is
    /// commentary, and the gate is what enforces it.
    Message { text: String, origin: Origin },

    /// Reasoning content, where a provider exposes it. ACP's
    /// `agent_thought_chunk`. Shown collapsed: it is not a claim.
    Thought { text: String, origin: Origin },

    /// A tool the model asked for. ACP's `tool_call`.
    ToolCall { id: String, tool: String, arguments: serde_json::Value, origin: Origin },

    /// How that call went. ACP's `tool_call_update`.
    ///
    /// The output is a summary, not the whole thing: the verbatim output is in
    /// the evidence store, which is where a citation points anyway.
    ToolResult {
        id: String,
        status: CallStatus,
        summary: String,
        /// Present when the call minted evidence. A successful call always
        /// should have.
        evidence: Option<String>,
    },

    /// A claim was refused by the gate, and why.
    ///
    /// In the transcript rather than swallowed, because `§5` makes the
    /// rejection rate a measurement of our own tool descriptions. A rejection
    /// nobody sees cannot be that.
    ClaimRejected { claim: String, reason: String },

    /// A claim survived the gate and became a finding.
    ClaimAccepted { claim: String, finding: String },

    /// The session ended.
    Finished { reason: StopReason, steps: u32, elapsed: Duration },
}

impl TranscriptEvent {
    /// Whether this ends the session.
    pub fn is_terminal(&self) -> bool {
        matches!(self, TranscriptEvent::Finished { .. })
    }

    /// Where this entry came from, for the badge.
    ///
    /// Events we mint ourselves — a gate rejection, the start and end of a
    /// session — are native whatever the reasoner was, because *we* are the
    /// ones saying them.
    pub fn origin(&self) -> Origin {
        match self {
            TranscriptEvent::Started { origin, .. }
            | TranscriptEvent::Message { origin, .. }
            | TranscriptEvent::Thought { origin, .. }
            | TranscriptEvent::ToolCall { origin, .. } => *origin,
            _ => Origin::Native,
        }
    }

    /// A one-line label for the collapsed view.
    ///
    /// Clamped centrally rather than per arm. Every arm used to truncate its
    /// own text except `Hypothesis`, which interpolated a belief of any length
    /// straight into the row and broke the layout for a wordy model. Doing it
    /// once here means a variant added later cannot reintroduce that.
    pub fn headline(&self) -> String {
        let headline = one_line(&self.headline_text());
        if headline.is_empty() {
            // A blank row is a thing the user can see and cannot click. The
            // loop never pushes an empty message, but `Transcript` is public
            // and an ACP agent may send an empty chunk.
            return self.kind_label().to_string();
        }
        headline
    }

    /// What kind of entry this is, for a headline with nothing else to say.
    pub fn kind_label(&self) -> &'static str {
        match self {
            TranscriptEvent::Started { .. } => "Session started",
            TranscriptEvent::Hypothesis { .. } => "Hypothesis",
            TranscriptEvent::Message { .. } => "(no text)",
            TranscriptEvent::Thought { .. } => "Thinking",
            TranscriptEvent::ToolCall { .. } => "Tool call",
            TranscriptEvent::ToolResult { .. } => "Tool result",
            TranscriptEvent::ClaimRejected { .. } => "Claim refused",
            TranscriptEvent::ClaimAccepted { .. } => "Claim accepted",
            TranscriptEvent::Finished { .. } => "Session finished",
        }
    }

    fn headline_text(&self) -> String {
        match self {
            TranscriptEvent::Started { reasoner, .. } => format!("Session started · {reasoner}"),
            TranscriptEvent::Hypothesis { step, belief, .. } => format!("Step {step}: {belief}"),
            TranscriptEvent::Message { text, .. } => first_line(text),
            TranscriptEvent::Thought { text, .. } => format!("Thinking: {}", first_line(text)),
            TranscriptEvent::ToolCall { tool, .. } => format!("Called {tool}"),
            TranscriptEvent::ToolResult { status, summary, .. } => {
                format!("{}: {}", status.label(), first_line(summary))
            }
            TranscriptEvent::ClaimRejected { claim, reason } => {
                format!("Refused “{}” — {reason}", first_line(claim))
            }
            TranscriptEvent::ClaimAccepted { claim, .. } => {
                format!("Accepted “{}”", first_line(claim))
            }
            TranscriptEvent::Finished { reason, steps, .. } => {
                format!("Finished after {steps} steps — {}", reason.describe())
            }
        }
    }
}

/// A whole session, as the panel renders it.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Transcript {
    events: Vec<TranscriptEvent>,
}

impl Transcript {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, event: TranscriptEvent) {
        self.events.push(event);
    }

    pub fn events(&self) -> &[TranscriptEvent] {
        &self.events
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Why the session stopped, once it has.
    pub fn stop_reason(&self) -> Option<&StopReason> {
        self.events.iter().rev().find_map(|event| match event {
            TranscriptEvent::Finished { reason, .. } => Some(reason),
            _ => None,
        })
    }

    /// Apply a tool-call update to the call it refers to.
    ///
    /// ACP sends the call and its outcome as separate notifications, so the
    /// transcript has to be able to find the earlier entry. Returns whether
    /// it did: an update for a call we never saw is a protocol error worth
    /// noticing rather than a silent append.
    pub fn update_call(
        &mut self,
        id: &str,
        status: CallStatus,
        summary: impl Into<String>,
    ) -> bool {
        let known = self.events.iter().any(|event| match event {
            TranscriptEvent::ToolCall { id: existing, .. } => existing == id,
            _ => false,
        });
        if !known {
            return false;
        }
        self.events.push(TranscriptEvent::ToolResult {
            id: id.to_string(),
            status,
            summary: summary.into(),
            evidence: None,
        });
        true
    }

    /// How many claims the gate refused.
    ///
    /// `§5`: a high rate means our tool descriptions are unclear, so this is
    /// shown as a tally rather than buried.
    pub fn rejected(&self) -> usize {
        self.events
            .iter()
            .filter(|event| matches!(event, TranscriptEvent::ClaimRejected { .. }))
            .count()
    }

    pub fn accepted(&self) -> usize {
        self.events
            .iter()
            .filter(|event| matches!(event, TranscriptEvent::ClaimAccepted { .. }))
            .count()
    }

    /// Whether anything here was reached by an external agent.
    pub fn has_external_claims(&self) -> bool {
        self.events.iter().any(|event| event.origin().needs_badge())
    }
}

fn first_line(text: &str) -> String {
    clamp(text.lines().next().unwrap_or("").trim(), 80)
}

/// One line, bounded, whatever went in.
///
/// Collapses embedded newlines rather than taking only the first line: a
/// headline built from several fields must not lose the later ones just
/// because an earlier one contained a line break.
fn one_line(text: &str) -> String {
    let joined = text.split_whitespace().collect::<Vec<_>>().join(" ");
    clamp(&joined, 160)
}

/// Truncate on a character boundary, never a byte one.
fn clamp(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let kept: String = text.chars().take(limit.saturating_sub(1)).collect();
    format!("{kept}…")
}

/// What a session spent, for the meter (`§9.3`).
///
/// Display vocabulary only: the budget *logic* — which limit was passed, and
/// what to do about it — stays with the loop in `binmap-agent`, because the
/// interface does not decide when to stop. This is what the panel needs to draw
/// a meter, and nothing more.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct SessionCost {
    pub steps: u32,
    pub max_steps: u32,
    pub input_tokens: u64,
    pub output_tokens: u64,
    /// `None` means free, which is not zero. A local model costs nothing to
    /// run; a priced model nobody has called yet has spent nothing. The meter
    /// says different things for the two.
    pub cost: Option<f64>,
    pub elapsed: Duration,
}

impl SessionCost {
    pub fn total_tokens(&self) -> u64 {
        self.input_tokens + self.output_tokens
    }

    /// How far through the step budget, in 0.0..=1.0, for the meter's bar.
    pub fn step_fraction(&self) -> f32 {
        if self.max_steps == 0 {
            return 0.0;
        }
        (self.steps as f32 / self.max_steps as f32).clamp(0.0, 1.0)
    }

    /// What the meter reads.
    pub fn label(&self) -> String {
        let money = match self.cost {
            // Below a cent, "$0.00" reads as free when it is not. Saying
            // "under $0.01" is the honest version of the same number.
            Some(spent) if spent > 0.0 && spent < 0.01 => "under $0.01".to_string(),
            Some(spent) => format!("${spent:.2}"),
            None => "free".to_string(),
        };
        format!(
            "{} of {} steps · {} tokens · {money}",
            self.steps,
            self.max_steps,
            self.total_tokens()
        )
    }
}
