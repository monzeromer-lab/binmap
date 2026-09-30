//! The finding gate (`A1.2`, `R2`).
//!
//! §8's metric table says the grounding rate "must be 1.0 by construction.
//! Lower is a bug in the gate, not a model failure." That sentence is the
//! specification for this module: a model does not *fail* to ground a claim,
//! because a claim that is not grounded never becomes a finding.
//!
//! Two things are checked, and they are different questions:
//!
//! - **Did it cite anything?** A model asked to explain something will
//!   produce an explanation whether or not it looked. An uncited claim is not
//!   a weak finding, it is not a finding.
//! - **Did it cite something real?** This is the airlock. An identifier that
//!   looks plausible and was never issued is the specific failure mode of a
//!   model that has seen the shape of our evidence identifiers, and it is
//!   indistinguishable from a real one without asking the store.
//!
//! The gate lives here rather than inside `Finding::new` because rejections
//! must be *counted*. `Finding::new` returns an error and the caller moves on;
//! the rejection rate is a measurement — high means our tool descriptions are
//! unclear, not that the agent is bad — and something has to keep the tally.

use binmap_core::evidence::{EvidenceId, EvidenceStore};
use binmap_core::finding::{Confidence, Finding, FindingDraft, FindingKind, Provenance};
use serde::{Deserialize, Serialize};

/// What a model proposed, before anything has been checked.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Claim {
    pub id: String,
    pub kind: FindingKind,
    pub title: String,
    pub detail: String,
    pub confidence: Confidence,
    /// What it says it looked at.
    pub cites: Vec<EvidenceId>,
}

/// Why a claim was refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum Rejection {
    /// It cited nothing. Not a weak finding — not a finding.
    Ungrounded { claim: String },
    /// It cited an identifier this session never issued.
    ///
    /// The one that matters. A plausible-looking identifier is
    /// indistinguishable from a real one without asking the store, which is
    /// exactly why the store is asked every time.
    Invented { claim: String, evidence: String },
}

impl Rejection {
    /// The sentence the Agent panel shows beside the rejected claim.
    ///
    /// It names the claim, because "a claim was rejected" teaches nobody
    /// anything and the panel's job is to make the gate legible.
    pub fn describe(&self) -> String {
        match self {
            Rejection::Ungrounded { claim } => {
                format!("`{claim}` cited no evidence, so it was not recorded")
            }
            Rejection::Invented { claim, evidence } => {
                format!("`{claim}` cited `{evidence}`, which no tool in this session produced")
            }
        }
    }
}

/// Runs the gate, and counts.
#[derive(Debug, Default)]
pub struct Gate {
    accepted: usize,
    rejections: Vec<Rejection>,
}

impl Gate {
    pub fn new() -> Self {
        Self::default()
    }

    /// Turn a claim into a finding, or refuse it.
    ///
    /// `provenance` is supplied by the caller rather than the model, because a
    /// model saying its own claim was Measured would be the entire problem.
    /// The caller knows whether this came from its own loop or an external
    /// agent; the model does not get a vote.
    pub fn admit(
        &mut self,
        claim: Claim,
        provenance: Provenance,
        store: &EvidenceStore,
    ) -> std::result::Result<Finding, Rejection> {
        if claim.cites.is_empty() {
            let rejection = Rejection::Ungrounded { claim: claim.id };
            self.rejections.push(rejection.clone());
            return Err(rejection);
        }

        for evidence in &claim.cites {
            if !store.issued(evidence) {
                let rejection =
                    Rejection::Invented { claim: claim.id, evidence: evidence.to_string() };
                self.rejections.push(rejection.clone());
                return Err(rejection);
            }
        }

        let draft = FindingDraft::new(claim.id, claim.kind, claim.title)
            .detail(claim.detail)
            .confidence(claim.confidence)
            .provenance(provenance)
            .citing(claim.cites);

        // Finding::new re-checks both conditions. That is deliberate
        // duplication: this gate counts, the constructor guarantees, and the
        // guarantee must not depend on anyone having run the gate first.
        match Finding::new(draft, store) {
            Ok(finding) => {
                self.accepted += 1;
                Ok(finding)
            }
            Err(_) => {
                let rejection =
                    Rejection::Ungrounded { claim: "a claim the constructor refused".into() };
                self.rejections.push(rejection.clone());
                Err(rejection)
            }
        }
    }

    pub fn accepted(&self) -> usize {
        self.accepted
    }

    pub fn rejections(&self) -> &[Rejection] {
        &self.rejections
    }

    /// The fraction of claims that became findings.
    ///
    /// Note what this is *not*: the grounding rate. Every finding that exists
    /// is grounded, by construction, so the grounding rate is 1.0 and is not
    /// worth measuring. This is the rejection rate's complement, and §8 reads
    /// it as a measurement of our tool descriptions rather than of the model.
    pub fn acceptance_rate(&self) -> f64 {
        let total = self.accepted + self.rejections.len();
        if total == 0 { 1.0 } else { self.accepted as f64 / total as f64 }
    }

    /// The tally the Agent panel shows.
    pub fn tally(&self) -> String {
        match self.rejections.len() {
            0 => format!("{} claims, all grounded", self.accepted),
            rejected => format!("{} claims grounded, {rejected} rejected", self.accepted),
        }
    }
}
