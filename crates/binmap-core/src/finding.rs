//! `Finding` is the whole product.
//!
//! Every analysis produces findings, every view renders them, and the exported
//! artifact is a list of them. A finding carries a kind, a location, an impact,
//! a confidence, a provenance and a non-empty evidence list — and the
//! constructor is the thing that makes "non-empty" and "actually issued" true
//! rather than merely intended (`R2`).
//!
//! The words in [`Provenance`] and [`Confidence`] are fixed vocabulary. They
//! are never paraphrased — not in code, not in the interface, not in the docs.

use crate::error::{Error, Result};
use crate::evidence::{EvidenceId, EvidenceStore};
use crate::location::Location;
use serde::{Deserialize, Serialize};

/// How we came to believe a finding, and therefore how far it can be trusted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Provenance {
    /// A tool ran. Command, arguments and output digest are recorded.
    Measured,

    /// A named rule of ours concluded it from measured inputs. The rule is
    /// named because "derived" without a rule name is indistinguishable from a
    /// guess.
    Derived { rule: String },

    /// Our own loop, which forces a hypothesis before every tool call and
    /// enforces the grounding gate in this constructor.
    InferredNatively { model: String },

    /// An external agent's claim, validated at the airlock. A weaker
    /// guarantee, and the interface labels it as one.
    InferredExternally { agent: String },
}

impl Provenance {
    /// The badge glyph, matching the design: ● measured, ◈ derived, ◆ inferred.
    pub fn glyph(&self) -> char {
        match self {
            Provenance::Measured => '●',
            Provenance::Derived { .. } => '◈',
            Provenance::InferredNatively { .. } | Provenance::InferredExternally { .. } => '◆',
        }
    }

    /// The label, in the fixed vocabulary.
    pub fn label(&self) -> &'static str {
        match self {
            Provenance::Measured => "Measured",
            Provenance::Derived { .. } => "Derived",
            Provenance::InferredNatively { .. } => "Inferred, natively",
            Provenance::InferredExternally { .. } => "Inferred, externally",
        }
    }

    /// The highest confidence this provenance may carry. A finding claiming
    /// more than its provenance allows is clamped by [`Finding::new`] rather
    /// than rejected — the claim is still worth showing, at its real ceiling.
    pub fn ceiling(&self) -> Confidence {
        match self {
            Provenance::Measured => Confidence::Certain,
            Provenance::Derived { .. } => Confidence::High,
            Provenance::InferredNatively { .. } | Provenance::InferredExternally { .. } => {
                Confidence::Probable
            }
        }
    }

    /// Whether the claim entered from outside our own loop. The Agent panel
    /// puts an external-origin badge on these (`AI.10`).
    pub fn is_external(&self) -> bool {
        matches!(self, Provenance::InferredExternally { .. })
    }
}

/// How sure we are. Four words, ordered, never paraphrased.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Speculative,
    Probable,
    High,
    Certain,
}

impl Confidence {
    pub fn label(self) -> &'static str {
        match self {
            Confidence::Certain => "Certain",
            Confidence::High => "High",
            Confidence::Probable => "Probable",
            Confidence::Speculative => "Speculative",
        }
    }
}

/// What the finding is about. Open-ended by design: each phase adds kinds and
/// no view switches exhaustively on this.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum FindingKind {
    /// A build configuration measured during a sweep.
    Configuration,
    /// A configuration on the Pareto frontier over size, runtime and build time.
    FrontierPoint,
    /// A configuration that was measured and lost — kept visible, because a
    /// near miss is informative.
    RejectedCandidate,
    /// A probe of the environment: a tool present, missing, or the wrong version.
    EnvironmentProbe,
    /// Bytes attributed to a unit of the artifact.
    SizeAttribution,
    /// Generic instantiations that could be collapsed.
    Monomorphization,
    /// A recurring size driver: formatting machinery, panic strings, vtables.
    SizeDriver,
    /// A difference between two artifacts.
    Diff,
    /// Anything a later phase adds without a schema break.
    Other(String),
}

impl FindingKind {
    pub fn label(&self) -> &str {
        match self {
            FindingKind::Configuration => "Configuration",
            FindingKind::FrontierPoint => "Frontier point",
            FindingKind::RejectedCandidate => "Rejected candidate",
            FindingKind::EnvironmentProbe => "Environment probe",
            FindingKind::SizeAttribution => "Size attribution",
            FindingKind::Monomorphization => "Monomorphization",
            FindingKind::SizeDriver => "Size driver",
            FindingKind::Diff => "Diff",
            FindingKind::Other(name) => name,
        }
    }
}

/// What acting on the finding is worth, in the dimensions we actually measure.
///
/// Every field is a signed delta against the baseline, and negative is an
/// improvement. All of them are optional because a finding that did not
/// measure a dimension must not imply it measured zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Impact {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_nanos: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_time_nanos: Option<i64>,
    /// True when the measured difference fell inside the machine's noise floor.
    /// Such a result is reported inconclusive and never coloured as a win.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub within_noise_floor: bool,
}

impl Impact {
    pub fn none() -> Self {
        Self::default()
    }

    pub fn size(bytes: i64) -> Self {
        Self { size_bytes: Some(bytes), ..Self::default() }
    }

    pub fn with_runtime(mut self, nanos: i64) -> Self {
        self.runtime_nanos = Some(nanos);
        self
    }

    pub fn with_build_time(mut self, nanos: i64) -> Self {
        self.build_time_nanos = Some(nanos);
        self
    }

    pub fn inconclusive(mut self) -> Self {
        self.within_noise_floor = true;
        self
    }

    /// Whether this is an improvement worth colouring as one. A result inside
    /// the noise floor is not, whatever its sign.
    pub fn is_a_win(&self) -> bool {
        if self.within_noise_floor {
            return false;
        }
        self.size_bytes.is_some_and(|b| b < 0) || self.runtime_nanos.is_some_and(|n| n < 0)
    }
}

/// A single claim about the artifact, and the grounds for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    pub id: String,
    pub kind: FindingKind,
    /// One line, as it appears in the Inspector header.
    pub title: String,
    /// The hypothesis, in full. The Inspector's first tab.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub detail: String,
    pub location: Location,
    pub impact: Impact,
    pub confidence: Confidence,
    pub provenance: Provenance,
    /// Never empty. Enforced by [`Finding::new`].
    pub evidence: Vec<EvidenceId>,
}

/// The fields of a finding, before it has been checked.
///
/// Constructing one of these is free; turning it into a [`Finding`] is where
/// the grounding gate runs.
#[derive(Debug, Clone)]
pub struct FindingDraft {
    pub id: String,
    pub kind: FindingKind,
    pub title: String,
    pub detail: String,
    pub location: Location,
    pub impact: Impact,
    pub confidence: Confidence,
    pub provenance: Provenance,
    pub evidence: Vec<EvidenceId>,
}

impl FindingDraft {
    pub fn new(id: impl Into<String>, kind: FindingKind, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            kind,
            title: title.into(),
            detail: String::new(),
            location: Location::nowhere(),
            impact: Impact::none(),
            confidence: Confidence::Probable,
            provenance: Provenance::Measured,
            evidence: Vec::new(),
        }
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = detail.into();
        self
    }

    pub fn at(mut self, location: Location) -> Self {
        self.location = location;
        self
    }

    pub fn impact(mut self, impact: Impact) -> Self {
        self.impact = impact;
        self
    }

    pub fn confidence(mut self, confidence: Confidence) -> Self {
        self.confidence = confidence;
        self
    }

    pub fn provenance(mut self, provenance: Provenance) -> Self {
        self.provenance = provenance;
        self
    }

    pub fn citing(mut self, evidence: impl IntoIterator<Item = EvidenceId>) -> Self {
        self.evidence.extend(evidence);
        self
    }

    pub fn cite(mut self, evidence: EvidenceId) -> Self {
        self.evidence.push(evidence);
        self
    }
}

impl Finding {
    /// The only way to make a finding.
    ///
    /// Two things are checked, and both are the difference between a tool that
    /// reports facts and one that confabulates:
    ///
    /// 1. The evidence list is not empty.
    /// 2. Every identifier in it was issued by this store — the airlock.
    ///
    /// A confidence above what the provenance allows is clamped to the ceiling
    /// rather than rejected. The claim is still worth showing; it is not worth
    /// showing at a confidence it has not earned.
    pub fn new(draft: FindingDraft, store: &EvidenceStore) -> Result<Self> {
        if draft.evidence.is_empty() {
            return Err(Error::Ungrounded(draft.id));
        }
        for id in &draft.evidence {
            if !store.issued(id) {
                return Err(Error::UnknownEvidence {
                    finding: draft.id.clone(),
                    evidence: id.to_string(),
                });
            }
        }

        let ceiling = draft.provenance.ceiling();
        Ok(Finding {
            id: draft.id,
            kind: draft.kind,
            title: draft.title,
            detail: draft.detail,
            location: draft.location,
            impact: draft.impact,
            confidence: draft.confidence.min(ceiling),
            provenance: draft.provenance,
            evidence: draft.evidence,
        })
    }

    /// Re-check an imported finding against the store that adopted its
    /// evidence. Import is not construction, so it needs its own gate.
    pub fn revalidate(&self, store: &EvidenceStore) -> Result<()> {
        if self.evidence.is_empty() {
            return Err(Error::Ungrounded(self.id.clone()));
        }
        for id in &self.evidence {
            if !store.issued(id) {
                return Err(Error::UnknownEvidence {
                    finding: self.id.clone(),
                    evidence: id.to_string(),
                });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::ToolInvocation;

    fn store_with_one_record() -> (EvidenceStore, EvidenceId) {
        let store = EvidenceStore::new();
        let pending = store.begin(ToolInvocation::new("size", ["app"]));
        let id = store.complete(pending, "text 1024", 0);
        (store, id)
    }

    #[test]
    fn a_finding_citing_nothing_cannot_be_built() {
        let (store, _) = store_with_one_record();
        let draft = FindingDraft::new("f1", FindingKind::SizeAttribution, "core::fmt is large");
        assert!(matches!(Finding::new(draft, &store), Err(Error::Ungrounded(_))));
    }

    #[test]
    fn a_finding_citing_evidence_we_never_issued_is_rejected() {
        let (ours, id) = store_with_one_record();
        let theirs = EvidenceStore::new();
        let draft = FindingDraft::new("f1", FindingKind::SizeAttribution, "claim").cite(id);
        // The identifier is real, but not ours. This is the airlock.
        assert!(matches!(Finding::new(draft, &theirs), Err(Error::UnknownEvidence { .. })));
        let draft = FindingDraft::new("f1", FindingKind::SizeAttribution, "claim")
            .cite(ours.records()[0].id.clone());
        assert!(Finding::new(draft, &ours).is_ok());
    }

    #[test]
    fn confidence_is_clamped_to_what_the_provenance_earns() {
        let (store, id) = store_with_one_record();
        let draft = FindingDraft::new("f1", FindingKind::Monomorphization, "collapse this generic")
            .cite(id)
            .confidence(Confidence::Certain)
            .provenance(Provenance::InferredExternally { agent: "claude-code".into() });
        let finding = Finding::new(draft, &store).unwrap();
        assert_eq!(finding.confidence, Confidence::Probable);
        assert!(finding.provenance.is_external());
    }

    #[test]
    fn a_result_inside_the_noise_floor_is_never_a_win() {
        let impact = Impact::size(-12).with_runtime(-3).inconclusive();
        assert!(!impact.is_a_win());
    }
}
