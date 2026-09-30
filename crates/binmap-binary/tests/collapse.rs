//! Collapse strategies (`A1.5`).
//!
//! What is being tested is mostly restraint: that a strategy is offered only
//! when the measurement supports it, and that nothing here ever claims a
//! saving is certain.

use binmap_binary::collapse::{Applicability, Strategy, WORTH_REPORTING, strategies_for};
use binmap_core::attribution::Monomorphization;

fn generic(instantiations: u32, arguments: &[(&str, u64)]) -> Monomorphization {
    Monomorphization {
        generic_path: "crate::thing::work".into(),
        total_bytes: arguments.iter().map(|(_, bytes)| bytes).sum(),
        instantiations,
        arguments: arguments.iter().map(|(a, b)| ((*a).to_string(), *b)).collect(),
    }
}

#[test]
fn a_generic_too_small_to_matter_gets_no_advice() {
    // A list that always has something in it is a list nobody reads.
    let tiny = generic(4, &[("u8", 200), ("u16", 180), ("u32", 160), ("u64", 140)]);
    assert!(tiny.collapsible_bytes() < WORTH_REPORTING);
    assert!(strategies_for(&tiny).is_empty(), "under a kilobyte is not worth anyone's attention");
}

#[test]
fn a_generic_instantiated_once_gets_no_advice() {
    // There is nothing to collapse into.
    let single = generic(1, &[("String", 40_000)]);
    assert!(strategies_for(&single).is_empty());
}

#[test]
fn outlining_is_always_offered_for_a_generic_worth_collapsing() {
    // It is the one strategy that changes no call site, so it is always worth
    // considering.
    let costly = generic(6, &[("String", 9_000), ("&str", 8_000), ("PathBuf", 7_000)]);
    let candidates = strategies_for(&costly);

    assert!(!candidates.is_empty());
    assert_eq!(candidates[0].strategy, Strategy::Outline, "and it is offered first");
    assert_eq!(candidates[0].applicability, Applicability::Likely);
}

#[test]
fn every_saving_is_an_upper_bound_that_leaves_the_largest_instantiation() {
    // The largest has to stay. A proposal claiming the whole total would be
    // claiming a saving that cannot happen.
    let costly = generic(3, &[("String", 9_000), ("&str", 5_000), ("PathBuf", 3_000)]);
    for candidate in strategies_for(&costly) {
        assert_eq!(
            candidate.saves_at_most,
            costly.collapsible_bytes(),
            "{:?} claims a different saving",
            candidate.strategy
        );
        assert!(candidate.saves_at_most < costly.total_bytes, "the largest instantiation stays");
    }
}

#[test]
fn all_reference_arguments_suggest_a_trait_object() {
    let handles = generic(4, &[("&str", 9_000), ("&Path", 8_000), ("Box<dyn Read>", 7_000)]);
    let candidates = strategies_for(&handles);

    let dynamic = candidates.iter().find(|c| c.strategy == Strategy::Dynamic);
    let dynamic = dynamic.expect("all-reference arguments should suggest dyn");
    assert_eq!(
        dynamic.applicability,
        Applicability::NeedsTheSignature,
        "the symbol table cannot see the signature, so this is not asserted"
    );
    assert!(dynamic.because.contains("&str"), "it says what it saw: {}", dynamic.because);
}

#[test]
fn a_concrete_argument_among_them_withdraws_the_trait_object_suggestion() {
    // One owned type means the body may well need the concrete type.
    let mixed = generic(4, &[("&str", 9_000), ("String", 8_000), ("&Path", 7_000)]);
    assert!(
        !strategies_for(&mixed).iter().any(|c| c.strategy == Strategy::Dynamic),
        "a mixed set should not be told to use dyn"
    );
}

#[test]
fn all_numeric_arguments_suggest_collapsing_to_one_width() {
    let numeric = generic(4, &[("u8", 9_000), ("u16", 8_000), ("u32", 7_000), ("u64", 6_000)]);
    let candidates = strategies_for(&numeric);

    let widen = candidates.iter().find(|c| c.strategy == Strategy::WidenNumeric);
    assert!(widen.is_some(), "all-numeric instantiations are the widen case");
    assert_eq!(widen.unwrap().applicability, Applicability::NeedsTheSignature);
}

#[test]
fn identically_sized_instantiations_suggest_the_parameter_is_unused() {
    // The one signal only the symbol table can give: if the parameter changed
    // what the body did, the sizes would differ.
    let identical = generic(4, &[("A", 4_000), ("B", 4_000), ("C", 4_000), ("D", 4_000)]);
    let candidates = strategies_for(&identical);

    let unused = candidates.iter().find(|c| c.strategy == Strategy::UnusedParameter);
    let unused = unused.expect("identical sizes should suggest an unused parameter");
    assert_eq!(unused.applicability, Applicability::Likely);
    assert!(unused.because.contains("same size"), "{}", unused.because);
}

#[test]
fn differing_sizes_do_not_suggest_an_unused_parameter() {
    let differing = generic(4, &[("A", 4_000), ("B", 3_000), ("C", 2_000), ("D", 1_000)]);
    assert!(
        !strategies_for(&differing).iter().any(|c| c.strategy == Strategy::UnusedParameter),
        "sizes that differ are evidence the parameter is used"
    );
}

#[test]
fn zero_sized_symbols_are_not_evidence_of_anything() {
    // Every instantiation "the same size" at zero bytes says nothing, and a
    // confident "the parameter is unused" from it would be a fabrication.
    let empty = generic(4, &[("A", 0), ("B", 0), ("C", 0), ("D", 0)]);
    assert!(strategies_for(&empty).is_empty(), "nothing measurable, so nothing to advise");
}

#[test]
fn what_is_likely_is_offered_before_what_needs_checking() {
    let identical = generic(4, &[("&A", 4_000), ("&B", 4_000), ("&C", 4_000), ("&D", 4_000)]);
    let candidates = strategies_for(&identical);
    assert!(candidates.len() >= 2, "this should offer several");

    let first_uncertain = candidates
        .iter()
        .position(|c| c.applicability == Applicability::NeedsTheSignature)
        .unwrap_or(candidates.len());
    assert!(
        candidates[..first_uncertain].iter().all(|c| c.applicability == Applicability::Likely),
        "the likely ones come first: {candidates:?}"
    );
}

#[test]
fn every_strategy_states_what_it_costs() {
    // All of them cost something, and a proposal that only shows the saving is
    // one that will be regretted.
    for strategy in
        [Strategy::Outline, Strategy::Dynamic, Strategy::UnusedParameter, Strategy::WidenNumeric]
    {
        assert!(!strategy.cost().is_empty(), "{strategy:?} claims to cost nothing");
        assert!(!strategy.sketch().is_empty(), "{strategy:?} has no worked sketch");
        assert!(!strategy.label().is_empty());
        // `Derived` without a rule name is indistinguishable from a guess.
        assert!(strategy.rule().starts_with("collapse-strategy-"), "{}", strategy.rule());
    }
}

#[test]
fn rule_names_are_unique_so_provenance_identifies_the_strategy() {
    let rules = [
        Strategy::Outline.rule(),
        Strategy::Dynamic.rule(),
        Strategy::UnusedParameter.rule(),
        Strategy::WidenNumeric.rule(),
    ];
    let mut sorted = rules;
    sorted.sort_unstable();
    let count = sorted.len();
    let mut deduped = sorted.to_vec();
    deduped.dedup();
    assert_eq!(deduped.len(), count, "two strategies share a rule name: {rules:?}");
}

#[test]
fn every_reason_names_what_was_measured() {
    // A reason that does not cite what it saw is advice the reader cannot
    // check.
    let costly = generic(6, &[("&str", 9_000), ("&Path", 8_000), ("&[u8]", 7_000)]);
    for candidate in strategies_for(&costly) {
        assert!(!candidate.because.is_empty(), "{:?} gives no reason", candidate.strategy);
        assert!(
            candidate.because.chars().any(|c| c.is_ascii_digit())
                || candidate.because.contains('&'),
            "{:?} cites nothing measured: {}",
            candidate.strategy,
            candidate.because
        );
    }
}
