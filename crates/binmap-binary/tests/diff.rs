//! Diffing two builds, and the rename problem that makes the naive version
//! useless.

use binmap_binary::diff::diff;
use binmap_binary::symbols::{Sizing, Symbol, SymbolTable};

fn symbol(name: &str, size: u64) -> Symbol {
    Symbol {
        mangled: format!("_R{name}"),
        name: name.to_string(),
        address: 0x1000,
        size,
        section: ".text".into(),
        sizing: Sizing::Declared,
    }
}

fn table(symbols: Vec<Symbol>) -> SymbolTable {
    SymbolTable { symbols, inferred_fraction: 0.0, stripped: false }
}

#[test]
fn a_function_that_grew_is_reported_as_growth_not_as_two_events() {
    let before = table(vec![symbol("app::route", 1000)]);
    let after = table(vec![symbol("app::route", 1500)]);
    let changed = diff(&before, &after, &["app".into()]);

    assert_eq!(changed.grew.len(), 1);
    assert_eq!(changed.grew[0].delta, 500);
    assert!(changed.added.is_empty() && changed.removed.is_empty());
    assert_eq!(changed.total_delta(), 500);
}

#[test]
fn a_generic_whose_arguments_changed_is_one_change_not_a_removal_and_an_addition() {
    // The whole reason F1.5 says "including generic-argument renames". Type
    // arguments move whenever the calling code changes type, an inference
    // decision moves, or a closure is numbered differently — and reporting
    // `Tally<u32>` removed and `Tally<u64>` added is true and useless.
    let before = table(vec![symbol("app::Tally<u32>::describe", 1000)]);
    let after = table(vec![symbol("app::Tally<u64>::describe", 1200)]);
    let changed = diff(&before, &after, &["app".into()]);

    assert!(changed.added.is_empty(), "a rename was reported as an addition");
    assert!(changed.removed.is_empty(), "a rename was reported as a removal");
    assert_eq!(changed.grew.len(), 1);
    assert_eq!(changed.grew[0].name, "app::Tally");
    assert_eq!(changed.grew[0].delta, 200);
}

#[test]
fn instantiations_are_compared_in_aggregate_across_the_two_builds() {
    // Three instantiations on one side, four on the other, is one generic
    // that grew — not four additions and three removals.
    let before = table(vec![
        symbol("app::Tally<u8>::f", 100),
        symbol("app::Tally<u16>::f", 100),
        symbol("app::Tally<u32>::f", 100),
    ]);
    let after = table(vec![
        symbol("app::Tally<u8>::f", 100),
        symbol("app::Tally<u16>::f", 100),
        symbol("app::Tally<u32>::f", 100),
        symbol("app::Tally<u64>::f", 100),
    ]);
    let changed = diff(&before, &after, &["app".into()]);

    assert!(changed.added.is_empty());
    assert_eq!(changed.grew.len(), 1);
    assert_eq!(changed.grew[0].delta, 100);
    assert_eq!(changed.grew[0].symbols, 4);
    // And the diff says how much of itself rests on that grouping.
    assert_eq!(changed.matched_by_generic, 1);
}

#[test]
fn a_genuinely_new_function_is_an_addition() {
    let before = table(vec![symbol("app::route", 1000)]);
    let after = table(vec![symbol("app::route", 1000), symbol("app::retry", 400)]);
    let changed = diff(&before, &after, &["app".into()]);

    assert_eq!(changed.added.len(), 1);
    assert_eq!(changed.added[0].name, "app::retry");
    assert!(changed.grew.is_empty(), "an unchanged function was reported as growth");
}

#[test]
fn a_function_that_went_away_is_a_removal() {
    let before = table(vec![symbol("app::route", 1000), symbol("app::legacy", 800)]);
    let after = table(vec![symbol("app::route", 1000)]);
    let changed = diff(&before, &after, &["app".into()]);

    assert_eq!(changed.removed.len(), 1);
    assert_eq!(changed.removed[0].name, "app::legacy");
    assert_eq!(changed.removed[0].delta, -800);
    assert_eq!(changed.total_delta(), -800);
}

#[test]
fn identical_builds_diff_to_nothing() {
    let one = table(vec![symbol("app::route", 1000), symbol("app::Tally<u8>::f", 200)]);
    let changed = diff(&one, &one, &["app".into()]);
    assert!(changed.is_empty(), "a build differed from itself");
    assert_eq!(changed.total_delta(), 0);
}

#[test]
fn growth_is_ranked_by_how_much_so_the_worst_is_first() {
    let before = table(vec![symbol("app::a", 100), symbol("app::b", 100)]);
    let after = table(vec![symbol("app::a", 200), symbol("app::b", 5000)]);
    let changed = diff(&before, &after, &["app".into()]);

    assert_eq!(changed.grew[0].name, "app::b");
    assert_eq!(changed.grew[0].delta, 4900);
}

#[test]
fn savings_are_ranked_by_how_much_too() {
    let before = table(vec![symbol("app::a", 5000), symbol("app::b", 200)]);
    let after = table(vec![symbol("app::a", 100), symbol("app::b", 100)]);
    let changed = diff(&before, &after, &["app".into()]);

    assert_eq!(changed.shrank[0].name, "app::a", "the largest saving should be first");
    assert_eq!(changed.shrank[0].delta, -4900);
}

#[test]
fn the_change_is_also_reported_by_category() {
    // "What kind of thing grew" should be answerable without reading a list
    // of four hundred symbols.
    use binmap_core::attribution::Driver;
    let before = table(vec![symbol("app::route", 1000)]);
    let mut after_symbols = vec![symbol("app::route", 1000)];
    after_symbols.push(symbol("core::fmt::Formatter::pad", 3000));
    let changed = diff(&before, &table(after_symbols), &["app".into()]);

    let formatting = changed
        .by_driver
        .iter()
        .find(|(driver, _)| *driver == Driver::Formatting)
        .expect("formatting machinery appeared");
    assert_eq!(formatting.1, 3000);
}

#[test]
fn a_zero_size_symbol_does_not_appear_on_either_side() {
    let before = table(vec![symbol("app::marker", 0)]);
    let after = table(vec![symbol("app::marker", 0)]);
    assert!(diff(&before, &after, &["app".into()]).is_empty());
}
