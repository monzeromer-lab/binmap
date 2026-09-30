//! The finding gate, which §8 says must make the grounding rate 1.0 by
//! construction — "lower is a bug in the gate, not a model failure".

use binmap_agent::gate::{Claim, Gate, Rejection};
use binmap_agent::registry::{Registry, Tool, ToolCall};
use binmap_core::config::TrustTier;
use binmap_core::evidence::{EvidenceId, EvidenceStore, ToolInvocation};
use binmap_core::finding::{Confidence, FindingKind, Provenance};
use proptest::prelude::*;

fn store_with(count: usize) -> (EvidenceStore, Vec<EvidenceId>) {
    let store = EvidenceStore::new();
    let ids = (0..count)
        .map(|index| {
            let pending = store.begin(ToolInvocation::new("tool", [index.to_string()]));
            store.complete(pending, format!("output {index}"), 0)
        })
        .collect();
    (store, ids)
}

fn claim(id: &str, cites: Vec<EvidenceId>) -> Claim {
    Claim {
        id: id.to_string(),
        kind: FindingKind::Monomorphization,
        title: "this generic is instantiated too many times".into(),
        detail: "the detail".into(),
        confidence: Confidence::Probable,
        cites,
    }
}

fn inferred() -> Provenance {
    Provenance::InferredNatively { model: "a-local-model".into() }
}

#[test]
fn a_claim_citing_nothing_is_not_a_weak_finding_it_is_not_a_finding() {
    let (store, _) = store_with(2);
    let mut gate = Gate::new();

    let refused = gate.admit(claim("c1", Vec::new()), inferred(), &store).unwrap_err();
    assert!(matches!(refused, Rejection::Ungrounded { .. }));
    assert_eq!(gate.accepted(), 0);
    assert_eq!(gate.rejections().len(), 1);
}

#[test]
fn a_claim_citing_an_invented_identifier_is_refused_and_the_identifier_is_named() {
    // The airlock. A plausible-looking identifier is indistinguishable from a
    // real one without asking the store, which is why the store is asked.
    let (store, _) = store_with(2);
    let invented: EvidenceId = serde_json::from_str("\"ev-000999\"").unwrap();
    let mut gate = Gate::new();

    let refused = gate.admit(claim("c1", vec![invented]), inferred(), &store).unwrap_err();
    match &refused {
        Rejection::Invented { claim, evidence } => {
            assert_eq!(claim, "c1");
            assert_eq!(evidence, "ev-000999");
        }
        other => panic!("expected an invented-evidence rejection, got {other:?}"),
    }
    assert!(refused.describe().contains("no tool in this session produced"));
}

#[test]
fn one_invented_identifier_among_real_ones_still_refuses_the_claim() {
    let (store, mut ids) = store_with(3);
    ids.push(serde_json::from_str("\"ev-999999\"").unwrap());
    let mut gate = Gate::new();

    assert!(gate.admit(claim("c1", ids), inferred(), &store).is_err());
    assert_eq!(gate.accepted(), 0);
}

#[test]
fn a_grounded_claim_becomes_a_finding_at_the_confidence_its_provenance_earns() {
    let (store, ids) = store_with(2);
    let mut gate = Gate::new();

    let mut asking_for_certain = claim("c1", ids);
    asking_for_certain.confidence = Confidence::Certain;
    let finding = gate.admit(asking_for_certain, inferred(), &store).expect("it is grounded");

    // Inferred caps at Probable however confident the model sounded.
    assert_eq!(finding.confidence(), Confidence::Probable);
    assert_eq!(gate.accepted(), 1);
    assert!(gate.rejections().is_empty());
}

#[test]
fn the_model_does_not_get_a_vote_on_its_own_provenance() {
    // A model saying its own claim was Measured would be the entire problem.
    // Provenance is the caller's, and Claim has no field for it.
    let (store, ids) = store_with(1);
    let mut gate = Gate::new();
    let finding = gate.admit(claim("c1", ids), inferred(), &store).unwrap();
    assert!(matches!(finding.provenance(), Provenance::InferredNatively { .. }));
}

#[test]
fn every_finding_that_exists_is_grounded_which_is_why_the_rate_is_not_measured() {
    let (store, ids) = store_with(4);
    let mut gate = Gate::new();

    let accepted = gate.admit(claim("c1", ids.clone()), inferred(), &store).unwrap();
    assert!(!accepted.evidence().is_empty());
    for evidence in accepted.evidence() {
        assert!(store.issued(evidence));
    }

    let _ = gate.admit(claim("c2", Vec::new()), inferred(), &store);
    assert_eq!(gate.acceptance_rate(), 0.5);
    assert_eq!(gate.tally(), "1 claims grounded, 1 rejected");
}

// ---------------------------------------------------------------------------
// The registry
// ---------------------------------------------------------------------------

#[test]
fn every_tool_description_is_a_briefing_not_a_label() {
    // An external agent arrives knowing nothing and has only this string. A
    // high rejection rate in Mode B means these are unclear, not that the
    // agent is bad.
    for tool in Registry::phase_one().tools() {
        assert!(
            tool.description.len() > 80,
            "`{}` is described in {} characters",
            tool.name,
            tool.description.len()
        );
        assert!(
            tool.description.ends_with('.'),
            "`{}` is not a sentence: {}",
            tool.name,
            tool.description
        );
        assert!(tool.parameters.is_object(), "`{}` has no schema", tool.name);
    }
}

#[test]
fn nothing_phase_one_exposes_changes_anything() {
    // One problem at a time: in Phase 1 the trust question is entirely about
    // what a model may conclude, not what it may do.
    for tool in Registry::phase_one().tools() {
        assert!(!tool.side_effects, "`{}` has side effects", tool.name);
        assert_eq!(tool.required_tier, TrustTier::Observe);
    }
}

#[test]
fn registering_one_name_twice_is_an_error_not_a_replacement() {
    let mut registry = Registry::new();
    let tool = Tool::reading("read", "Read a thing.", serde_json::json!({}));
    registry.register(tool.clone()).unwrap();
    assert!(registry.register(tool).is_err(), "the abstraction silently broke");
}

#[test]
fn asking_for_a_tool_that_does_not_exist_names_the_ones_that_do() {
    // A model that asked for the wrong tool can correct itself given the list
    // and cannot given "no".
    let registry = Registry::phase_one();
    let call = ToolCall { tool: "read_the_source".into(), arguments: serde_json::json!({}) };
    let error = registry.authorize(&call, TrustTier::Observe).unwrap_err().to_string();

    assert!(error.contains("read_the_source"), "{error}");
    assert!(error.contains("attribute_size"), "the available tools are not listed: {error}");
}

#[test]
fn a_tool_above_the_session_tier_is_refused_before_it_runs() {
    let mut registry = Registry::new();
    registry
        .register(Tool::writing(
            "apply_patch",
            "Write a verified patch into the working tree.",
            serde_json::json!({}),
            TrustTier::Autonomous,
        ))
        .unwrap();

    let call = ToolCall { tool: "apply_patch".into(), arguments: serde_json::json!({}) };
    assert!(registry.authorize(&call, TrustTier::Tune).is_err());
    assert!(registry.authorize(&call, TrustTier::Autonomous).is_ok());
}

#[test]
fn the_mcp_description_is_generated_from_the_same_registry() {
    // A tool written twice means the abstraction has broken, so the MCP view
    // is derived rather than maintained.
    let registry = Registry::phase_one();
    let mcp = registry.as_mcp();
    let tools = mcp["tools"].as_array().expect("an array of tools");

    assert_eq!(tools.len(), registry.len());
    for tool in tools {
        let name = tool["name"].as_str().expect("a name");
        let native = registry.get(name).expect("it came from the registry");
        assert_eq!(tool["description"].as_str(), Some(native.description.as_str()));
        assert_eq!(&tool["inputSchema"], &native.parameters);
    }
}

proptest! {
    /// However a model's claim is shaped, it never becomes a finding without
    /// citing evidence this session issued.
    #[test]
    fn no_shape_of_claim_gets_past_the_gate_ungrounded(
        real in 0usize..5,
        forged in prop::collection::vec("ev-[0-9]{1,8}", 0..4),
        confidence in 0usize..4,
    ) {
        let (store, mut cites) = store_with(real);
        let mut any_forged = false;
        for spelling in &forged {
            let id: EvidenceId = serde_json::from_str(&format!("\"{spelling}\"")).unwrap();
            if !store.issued(&id) {
                any_forged = true;
            }
            cites.push(id);
        }

        let mut claim = claim("c", cites.clone());
        claim.confidence = [
            Confidence::Speculative,
            Confidence::Probable,
            Confidence::High,
            Confidence::Certain,
        ][confidence];

        let mut gate = Gate::new();
        let outcome = gate.admit(claim, inferred(), &store);

        if cites.is_empty() || any_forged {
            prop_assert!(outcome.is_err(), "an ungrounded claim became a finding");
        } else {
            let finding = outcome.expect("every citation was real");
            // And whatever confidence was asked for, it is capped.
            prop_assert!(finding.confidence() <= Confidence::Probable);
            for evidence in finding.evidence() {
                prop_assert!(store.issued(evidence));
            }
        }
    }
}
