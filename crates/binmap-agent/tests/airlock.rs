//! Phase 2's exit criterion (`DESIGN-AI §5`).
//!
//! > The airlock rejects an external claim citing evidence never issued, and
//! > the rejection rate is measured — a high rate means the tool descriptions
//! > are unclear, not that the agent is bad.
//!
//! The whole arrangement rests on one asymmetry: an external agent can say
//! anything, and it cannot *issue* an evidence identifier. Identifiers come
//! from the store, minted before a tool's output is returned, and the airlock's
//! single question is whether this store issued the one being cited. A
//! plausible-looking identifier is indistinguishable from a real one without
//! asking, which is exactly why it is asked every time.

use binmap_agent::gate::{Claim, Gate, Rejection};
use binmap_agent::mcp::{Request, Server, ToolOutput, Tools};
use binmap_agent::registry::{Registry, ToolCall};
use binmap_core::config::TrustTier;
use binmap_core::evidence::{EvidenceId, EvidenceStore, ToolInvocation};
use binmap_core::finding::{Confidence, FindingKind, Provenance};
use serde_json::json;

/// Tools backed by a real evidence store, as the MCP subcommand wires them.
struct StoreBacked<'a> {
    store: &'a EvidenceStore,
}

impl Tools for StoreBacked<'_> {
    fn call(&self, call: &ToolCall) -> ToolOutput {
        let pending =
            self.store.begin(ToolInvocation::new(format!("mcp:{}", call.tool), ["--".to_string()]));
        let text = format!("ran {}", call.tool);
        let evidence = self.store.complete(pending, text.clone(), 0);
        ToolOutput { text, evidence: Some(evidence.to_string()), failed: false }
    }
}

fn request(method: &str, params: serde_json::Value, id: Option<i64>) -> Request {
    serde_json::from_value(json!({"jsonrpc": "2.0", "method": method, "params": params, "id": id}))
        .expect("well-formed")
}

/// Everything an external agent would do: connect, call a tool, keep the
/// identifier it was given.
fn an_external_session(store: &EvidenceStore, calls: usize) -> Vec<EvidenceId> {
    let registry = Registry::phase_one();
    let tools = StoreBacked { store };
    let mut server = Server::new(&registry, &tools, TrustTier::Observe);
    server.handle(&request("initialize", json!({}), Some(1)));
    server.handle(&request("notifications/initialized", json!({}), None));

    let name = registry.tools().next().expect("a tool").name.clone();
    (0..calls)
        .map(|index| {
            let response = server
                .handle(&request(
                    "tools/call",
                    json!({"name": name, "arguments": {}}),
                    Some(100 + index as i64),
                ))
                .expect("a response");
            let identifier = response["result"]["_meta"]["binmap/evidence"]
                .as_str()
                .expect("every result carries one")
                .to_string();
            // Deserialization is the only route from a string to an
            // `EvidenceId`, and that is deliberate: there is no public
            // constructor, so nothing outside a store can mint one. This is
            // the same path an external agent's citation really takes — JSON
            // in, identifier out, worthless until the store confirms it.
            serde_json::from_value(json!(identifier)).expect("an identifier is a string")
        })
        .collect()
}

fn claim(id: &str, cites: Vec<EvidenceId>) -> Claim {
    Claim {
        id: id.into(),
        kind: FindingKind::SizeDriver,
        title: "the formatting machinery dominates".into(),
        detail: "measured across the symbol table".into(),
        confidence: Confidence::Probable,
        cites,
    }
}

#[test]
fn a_claim_citing_evidence_this_session_issued_is_admitted() {
    // The complement of the criterion. An airlock that rejected everything
    // would satisfy the letter of it and make Mode B useless.
    let store = EvidenceStore::new();
    let issued = an_external_session(&store, 2);
    let mut gate = Gate::new();

    let finding = gate
        .admit(
            claim("external-1", issued),
            Provenance::InferredExternally { agent: "claude-code".into() },
            &store,
        )
        .expect("a grounded external claim is admitted");

    // Admitted, and labelled as externally reached — §1's weaker guarantee has
    // to survive into the finding.
    assert_eq!(finding.provenance().glyph(), '◆');
    assert_eq!(gate.accepted(), 1);
    assert!(gate.rejections().is_empty());
}

#[test]
fn a_claim_citing_evidence_never_issued_is_rejected() {
    // The exit criterion itself. The identifier below is well-formed and
    // plausible, which is the point: it is indistinguishable from a real one
    // without asking the store.
    let store = EvidenceStore::new();
    let _real = an_external_session(&store, 1);
    let mut gate = Gate::new();

    let invented: EvidenceId = serde_json::from_value(json!("ev-000999")).unwrap();
    let rejection = gate
        .admit(
            claim("external-2", vec![invented.clone()]),
            Provenance::InferredExternally { agent: "claude-code".into() },
            &store,
        )
        .expect_err("an invented identifier must not pass");

    match &rejection {
        Rejection::Invented { evidence, .. } => {
            assert_eq!(evidence, &invented.to_string(), "the rejection names it");
        }
        other => panic!("expected Invented, got {other:?}"),
    }
    assert_eq!(gate.accepted(), 0);
    assert_eq!(gate.rejections().len(), 1);
}

#[test]
fn one_invented_identifier_among_real_ones_still_rejects_the_claim() {
    // The interesting case: an agent that cites three real results and one it
    // made up. Admitting it because most of the citations check out would make
    // the airlock a majority vote.
    let store = EvidenceStore::new();
    let mut cites = an_external_session(&store, 3);
    cites.push(serde_json::from_value(json!("ev-000999")).unwrap());

    let mut gate = Gate::new();
    let rejection = gate
        .admit(
            claim("external-3", cites),
            Provenance::InferredExternally { agent: "codex".into() },
            &store,
        )
        .expect_err("one invented citation is enough");
    assert!(matches!(rejection, Rejection::Invented { .. }));
}

#[test]
fn a_claim_citing_nothing_is_rejected_as_ungrounded_rather_than_invented() {
    // Different failure, different message. "You cited nothing" and "you cited
    // something that does not exist" are different mistakes, and an agent told
    // the wrong one will fix the wrong thing.
    let store = EvidenceStore::new();
    let _ = an_external_session(&store, 1);
    let mut gate = Gate::new();

    let rejection = gate
        .admit(
            claim("external-4", Vec::new()),
            Provenance::InferredExternally { agent: "codex".into() },
            &store,
        )
        .expect_err("nothing cited");
    assert!(matches!(rejection, Rejection::Ungrounded { .. }), "{rejection:?}");
}

#[test]
fn identifiers_from_another_session_do_not_transfer() {
    // Each store issues its own. An identifier from a previous run looks
    // exactly like one from this run, and accepting it would let an agent
    // ground today's claim in yesterday's measurement.
    let yesterday = EvidenceStore::new();
    let stale = an_external_session(&yesterday, 1);

    let today = EvidenceStore::new();
    let _ = an_external_session(&today, 1);
    let mut gate = Gate::new();

    let rejection = gate
        .admit(
            claim("external-5", stale),
            Provenance::InferredExternally { agent: "claude-code".into() },
            &today,
        )
        .expect_err("another session's identifier is not this session's");
    assert!(matches!(rejection, Rejection::Invented { .. }));
}

#[test]
fn the_rejection_rate_is_measured_and_is_about_us() {
    // §5: "a high rate means the tool descriptions are unclear, not that the
    // agent is bad". A rate nobody records cannot be that measurement.
    let store = EvidenceStore::new();
    let issued = an_external_session(&store, 4);
    let mut gate = Gate::new();

    // Three good claims, one invented.
    for (index, evidence) in issued.iter().take(3).enumerate() {
        gate.admit(
            claim(&format!("good-{index}"), vec![evidence.clone()]),
            Provenance::InferredExternally { agent: "claude-code".into() },
            &store,
        )
        .expect("grounded");
    }
    let _ = gate.admit(
        claim("bad", vec![serde_json::from_value(json!("ev-999999")).unwrap()]),
        Provenance::InferredExternally { agent: "claude-code".into() },
        &store,
    );

    assert_eq!(gate.accepted(), 3);
    assert_eq!(gate.rejections().len(), 1);
    assert!((gate.acceptance_rate() - 0.75).abs() < 1e-9, "{}", gate.acceptance_rate());

    // And it is reportable as a sentence, because a number nobody surfaces is
    // not a measurement anyone acts on.
    let tally = gate.tally();
    assert!(!tally.is_empty());
    assert!(tally.contains('1') || tally.contains("refus"), "{tally}");
}

#[test]
fn every_rejection_names_the_claim_so_the_description_can_be_fixed() {
    // The rejection rate is only useful if each rejection says which tool's
    // briefing failed. "A claim was rejected" teaches nobody anything.
    let store = EvidenceStore::new();
    let _ = an_external_session(&store, 1);
    let mut gate = Gate::new();

    let _ = gate.admit(
        claim("the-claim-that-failed", vec![serde_json::from_value(json!("ev-000999")).unwrap()]),
        Provenance::InferredExternally { agent: "claude-code".into() },
        &store,
    );

    let rejection = &gate.rejections()[0];
    let described = rejection.describe();
    assert!(described.contains("the-claim-that-failed"), "{described}");
    assert!(!described.is_empty());
}

#[test]
fn an_external_claim_can_never_be_measured_however_it_is_presented() {
    // Provenance is the caller's to state, never the agent's. An agent
    // declaring its own claim Measured would be the entire problem.
    let store = EvidenceStore::new();
    let issued = an_external_session(&store, 1);
    let mut gate = Gate::new();

    let finding = gate
        .admit(
            claim("external-6", issued),
            Provenance::InferredExternally { agent: "claude-code".into() },
            &store,
        )
        .expect("grounded");

    assert_ne!(finding.provenance(), &Provenance::Measured);
    assert!(
        finding.confidence() <= Confidence::Probable,
        "an inferred claim is capped whatever it asserts: {:?}",
        finding.confidence()
    );
}
