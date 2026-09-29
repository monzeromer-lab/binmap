//! Property tests for the invariants the whole product rests on.
//!
//! The unit tests in `binmap-core` check these against inputs I chose. These
//! check them against inputs I did not, which is the only kind of check worth
//! having for a rule described as "enforced by construction".

use binmap_core::config::TrustTier;
use binmap_core::configuration::BuildConfiguration;
use binmap_core::evidence::{Evidence, EvidenceId, EvidenceStore, ToolInvocation};
use binmap_core::finding::{Confidence, Finding, FindingDraft, FindingKind, Provenance};
use proptest::prelude::*;

fn a_provenance() -> impl Strategy<Value = Provenance> {
    prop_oneof![
        Just(Provenance::Measured),
        "[a-z-]{1,12}".prop_map(|rule| Provenance::Derived { rule }),
        "[a-z0-9.:-]{1,12}".prop_map(|model| Provenance::InferredNatively { model }),
        "[a-z-]{1,12}".prop_map(|agent| Provenance::InferredExternally { agent }),
    ]
}

fn a_confidence() -> impl Strategy<Value = Confidence> {
    prop_oneof![
        Just(Confidence::Speculative),
        Just(Confidence::Probable),
        Just(Confidence::High),
        Just(Confidence::Certain),
    ]
}

/// A store holding `count` real records, and their identifiers.
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

proptest! {
    /// A finding's confidence never exceeds what its provenance earns —
    /// whatever confidence was asked for.
    ///
    /// The table is in the plan: Measured→Certain, Derived→High,
    /// Inferred→Probable. This is the ceiling holding over every combination
    /// rather than the four I happened to write by hand.
    #[test]
    fn confidence_never_exceeds_the_provenance_ceiling(
        provenance in a_provenance(),
        asked in a_confidence(),
    ) {
        let (store, ids) = store_with(1);
        let finding = Finding::new(
            FindingDraft::new("f", FindingKind::Configuration, "a claim")
                .cite(ids[0].clone())
                .confidence(asked)
                .provenance(provenance.clone()),
            &store,
        )
        .expect("it cites real evidence");

        prop_assert!(
            finding.confidence() <= provenance.ceiling(),
            "{:?} claimed {:?} with a ceiling of {:?}",
            provenance, finding.confidence(), provenance.ceiling()
        );
        // And it is never raised above what was asked for either.
        prop_assert!(finding.confidence() <= asked);
    }

    /// A finding can never be built citing evidence the store did not issue,
    /// however many real identifiers accompany the forged one.
    #[test]
    fn one_unknown_identifier_is_enough_to_refuse_a_finding(
        real in 0usize..6,
        forged in "[a-z0-9-]{1,16}",
    ) {
        let (store, mut ids) = store_with(real);
        // An identifier from outside. Deserialization is the only way to make
        // one, which is exactly how they arrive in practice.
        let outsider: EvidenceId =
            serde_json::from_str(&format!("\"{forged}\"")).expect("a string is an id");
        prop_assume!(!store.issued(&outsider));
        ids.push(outsider);

        let built = Finding::new(
            FindingDraft::new("f", FindingKind::Configuration, "a claim").citing(ids),
            &store,
        );
        prop_assert!(built.is_err(), "a forged identifier was accepted");
    }

    /// Evidence round-trips through JSON, and a record that was altered on the
    /// way never verifies.
    #[test]
    fn altering_any_part_of_a_record_breaks_its_digest(
        output in "[ -~\\n]{0,200}",
        tool in "[a-z][a-z0-9-]{0,10}",
        exit in -8i32..8,
        which in 0usize..4,
    ) {
        let store = EvidenceStore::new();
        let pending = store.begin(ToolInvocation::new(tool.clone(), ["--flag"]));
        let id = store.complete(pending, output.clone(), exit);
        let record = store.get(&id).expect("just completed");
        prop_assert!(record.digest_matches());

        // Survives serialization untouched.
        let json = serde_json::to_string(&record).expect("serializable");
        let restored: Evidence = serde_json::from_str(&json).expect("deserializable");
        prop_assert!(restored.digest_matches(), "a round trip broke the digest");

        // But not alteration, whichever part is altered.
        let mut altered = restored;
        match which {
            0 => altered.output.push('x'),
            1 => altered.invocation.tool.push('x'),
            2 => altered.invocation.arguments.push("--extra".into()),
            _ => altered.exit_code = altered.exit_code.wrapping_add(1),
        }
        prop_assert!(!altered.digest_matches(), "part {} of the record was not covered", which);
    }

    /// A store only ever adopts records that hash to their own digest.
    #[test]
    fn a_store_adopts_exactly_the_records_that_verify(
        outputs in prop::collection::vec("[ -~]{0,40}", 0..6),
        tamper in prop::collection::vec(any::<bool>(), 0..6),
    ) {
        let source = EvidenceStore::new();
        let mut records: Vec<Evidence> = outputs
            .iter()
            .map(|output| {
                let pending = source.begin(ToolInvocation::new("tool", ["-x"]));
                let id = source.complete(pending, output.clone(), 0);
                source.get(&id).expect("just completed")
            })
            .collect();

        let mut expected_refusals = 0;
        for (record, tampered) in records.iter_mut().zip(tamper.iter().chain(std::iter::repeat(&false))) {
            if *tampered {
                record.output.push_str(" (edited)");
                expected_refusals += 1;
            }
        }

        let destination = EvidenceStore::new();
        let refused = destination.adopt(records.clone());
        prop_assert_eq!(refused.len(), expected_refusals);
        prop_assert_eq!(destination.len(), records.len() - expected_refusals);
    }

    /// A tier permits an action exactly when it is at least the required tier.
    #[test]
    fn the_tier_ladder_is_an_order(at in 0usize..4, needs in 0usize..4) {
        let at = TrustTier::ALL[at];
        let needs = TrustTier::ALL[needs];
        let permitted = at.require(needs, "do the thing").is_ok();
        prop_assert_eq!(permitted, at >= needs);
        prop_assert_eq!(permitted, at.rank() >= needs.rank());
    }

    /// A configuration survives the round trip through the pairs the interface
    /// renders, so what is applied is what was measured.
    #[test]
    fn a_configuration_round_trips_through_its_settings(
        opt in 0usize..6, lto in 0usize..3, cgu in 1u32..64,
        panic in 0usize..2, strip in 0usize..3, overflow in any::<bool>(),
    ) {
        use binmap_core::config::{Lto, OptLevel, PanicStrategy, Strip};
        let original = BuildConfiguration {
            opt_level: Some([OptLevel::Zero, OptLevel::One, OptLevel::Two, OptLevel::Three,
                             OptLevel::Size, OptLevel::SizeNoLoopVec][opt]),
            lto: Some([Lto::Off, Lto::Thin, Lto::Fat][lto]),
            codegen_units: Some(cgu),
            panic: Some([PanicStrategy::Unwind, PanicStrategy::Abort][panic]),
            strip: Some([Strip::None, Strip::Debuginfo, Strip::Symbols][strip]),
            debug: None,
            overflow_checks: Some(overflow),
            build_std: None,
            target_cpu: None,
        };

        let mut restored = BuildConfiguration::default();
        restored.apply_settings(&original.settings());
        prop_assert_eq!(&restored, &original);

        // And its name is stable and unique to its content.
        prop_assert_eq!(original.name(), restored.name());
    }

    /// Two different configurations never share a name — which is the target
    /// directory, the resume key and the frontier point id at once.
    #[test]
    fn distinct_configurations_have_distinct_names(
        a_opt in 0usize..6, a_cgu in 1u32..64, a_cpu in "[a-z0-9-]{0,12}",
        b_opt in 0usize..6, b_cgu in 1u32..64, b_cpu in "[a-z0-9-]{0,12}",
    ) {
        use binmap_core::config::OptLevel;
        let levels = [OptLevel::Zero, OptLevel::One, OptLevel::Two, OptLevel::Three,
                      OptLevel::Size, OptLevel::SizeNoLoopVec];
        let make = |opt: usize, cgu: u32, cpu: &str| BuildConfiguration {
            opt_level: Some(levels[opt]),
            codegen_units: Some(cgu),
            target_cpu: if cpu.is_empty() { None } else { Some(cpu.to_string()) },
            ..Default::default()
        };
        let a = make(a_opt, a_cgu, &a_cpu);
        let b = make(b_opt, b_cgu, &b_cpu);

        if a == b {
            prop_assert_eq!(a.name(), b.name());
        } else {
            prop_assert_ne!(a.name(), b.name(), "{:?} and {:?} collide", a, b);
        }
    }
}
