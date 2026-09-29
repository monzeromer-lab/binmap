use binmap_binary::attribution::*;
use binmap_binary::symbols::{Sizing, Symbol, SymbolTable};

fn symbol(name: &str, mangled: &str, size: u64, section: &str) -> Symbol {
    Symbol {
        mangled: mangled.to_string(),
        name: name.to_string(),
        address: 0x1000,
        size,
        section: section.to_string(),
        sizing: Sizing::Declared,
    }
}

fn table(symbols: Vec<Symbol>) -> SymbolTable {
    SymbolTable { symbols, inferred_fraction: 0.0, stripped: false }
}

// ---------------------------------------------------------------------------
// Splitting a path (the grouping key F1.3 rests on)
// ---------------------------------------------------------------------------

#[test]
fn a_generic_and_its_arguments_are_separated() {
    let origin = split("app::Router<hyper::Body>::dispatch");
    assert_eq!(origin.crate_name, "app");
    assert_eq!(origin.generic_path, "app::Router");
    assert_eq!(origin.arguments.as_deref(), Some("<hyper::Body>"));
}

#[test]
fn nested_generic_arguments_are_not_cut_in_the_middle() {
    // The naive `find('<')..find('>')` splits this at the inner bracket and
    // produces a grouping key that groups nothing.
    let origin = split("app::Cache<BTreeMap<String, Vec<u8>>>::get");
    assert_eq!(origin.generic_path, "app::Cache");
    assert_eq!(origin.arguments.as_deref(), Some("<BTreeMap<String, Vec<u8>>>"));
}

#[test]
fn two_instantiations_of_one_generic_share_a_grouping_key() {
    // This is the entire mechanism behind monomorphization grouping.
    let a = split("app::Tally<u32>::describe");
    let b = split("app::Tally<alloc::string::String>::describe");
    assert_eq!(a.generic_path, b.generic_path);
    assert_ne!(a.arguments, b.arguments);
}

#[test]
fn a_qualified_self_type_at_the_front_is_not_a_generic_argument_list() {
    // `<T as Trait>::method` opens with `<` and is not a generic instantiation
    // of anything. Treating it as one makes every trait impl look like a
    // monomorphization.
    let origin = split("<app::Router as core::fmt::Debug>::fmt");
    assert_eq!(origin.arguments, None, "a qualified self type was read as arguments");
}

#[test]
fn a_qualified_method_belongs_to_the_crate_of_its_self_type() {
    // Reading `<std::path::PathBuf as Debug>::fmt` literally put a crate
    // called `<std` in the list — visible the first time attribution ran
    // against a real binary, where `<std`, `<core` and `<alloc` were four of
    // the top six "crates".
    let origin = split("<std::path::PathBuf as core::fmt::Debug>::fmt");
    assert_eq!(origin.crate_name, "std", "the self type decides, not the trait");

    // And a nested generic in the self type is not cut early.
    let nested = split("<alloc::vec::Vec<u8> as core::fmt::Debug>::fmt");
    assert_eq!(nested.crate_name, "alloc");

    // A bare `<Type>` with no trait keeps the whole type.
    let bare = split("<app::Router>::dispatch");
    assert_eq!(bare.crate_name, "app");
}

#[test]
fn a_turbofish_does_not_leave_a_separator_in_the_path() {
    // `drop_in_place::<T>` splits at the `<` and the path keeps a trailing
    // `::`, which showed up in the interface as `core::ptr::drop_in_place::`.
    let origin = split("core::ptr::drop_in_place::<alloc::string::String>");
    assert_eq!(origin.generic_path, "core::ptr::drop_in_place");
    assert_eq!(origin.crate_name, "core");
    assert_eq!(origin.arguments.as_deref(), Some("<alloc::string::String>"));
}

#[test]
fn a_plain_function_has_no_arguments_and_groups_with_nothing() {
    let origin = split("app::router::dispatch");
    assert_eq!(origin.crate_name, "app");
    assert_eq!(origin.module, "router");
    assert_eq!(origin.arguments, None);
}

// ---------------------------------------------------------------------------
// Classification (F1.4)
// ---------------------------------------------------------------------------

#[test]
fn the_recurring_size_drivers_are_told_apart() {
    let own = vec!["app".to_string()];
    let cases = [
        ("core::fmt::Formatter::pad", ".text", Driver::Formatting),
        ("core::panicking::panic_fmt", ".text", Driver::Panic),
        ("core::ptr::drop_in_place<app::Session>", ".text", Driver::DropGlue),
        ("app::Router::{{vtable}}", ".data.rel.ro", Driver::Vtable),
        ("app::router::dispatch", ".text", Driver::Yours),
        ("serde_json::de::from_str", ".text", Driver::Dependency),
        ("core::slice::sort", ".text", Driver::StandardLibrary),
    ];
    for (name, section, expected) in cases {
        let got = classify(&symbol(name, "_R0", 100, section), &own);
        assert_eq!(got, expected, "{name} classified as {got:?}");
    }
}

#[test]
fn unwinding_tables_are_recognised_by_their_section() {
    // `.eh_frame` is unwinding whatever the symbol is called, and it is the
    // category `panic = "abort"` removes.
    let own = vec!["app".to_string()];
    assert_eq!(classify(&symbol("anything", "_R0", 100, ".eh_frame"), &own), Driver::Unwinding);
}

#[test]
fn a_foreign_symbol_in_a_data_section_is_static_data_not_code() {
    let own = vec!["app".to_string()];
    assert_eq!(
        classify(&symbol("crc_table", "crc_table", 4096, ".rodata"), &own),
        Driver::StaticData
    );
    assert_eq!(classify(&symbol("memcpy", "memcpy", 200, ".text"), &own), Driver::Runtime);
}

#[test]
fn your_code_is_whatever_you_said_it_was() {
    // There is no marker in a symbol name for "mine", and guessing from the
    // crate name would be wrong for anyone whose crate is called serde.
    let theirs = classify(&symbol("serde::de::run", "_R0", 100, ".text"), &["app".to_string()]);
    assert_eq!(theirs, Driver::Dependency);

    let mine = classify(&symbol("serde::de::run", "_R0", 100, ".text"), &["serde".to_string()]);
    assert_eq!(mine, Driver::Yours);
}

#[test]
fn a_driver_offers_advice_only_where_there_is_some() {
    // Saying nothing beats inventing advice.
    assert!(Driver::Formatting.remedy().is_some());
    assert!(Driver::Unwinding.remedy().is_some());
    assert!(Driver::Yours.remedy().is_none(), "we cannot advise on the user's own code");
    assert!(Driver::StaticData.remedy().is_none());
}

// ---------------------------------------------------------------------------
// Attribution end to end
// ---------------------------------------------------------------------------

fn a_realistic_table() -> SymbolTable {
    table(vec![
        symbol("app::Tally<u32>::describe", "_R1", 1200, ".text"),
        symbol("app::Tally<alloc::string::String>::describe", "_R2", 1800, ".text"),
        symbol("app::Tally<(u8, char)>::describe", "_R3", 900, ".text"),
        symbol("app::main", "_R4", 400, ".text"),
        symbol("core::fmt::Formatter::pad", "_R5", 3000, ".text"),
        symbol("core::panicking::panic_fmt", "_R6", 500, ".text"),
        symbol("serde_json::de::from_str", "_R7", 2500, ".text"),
        symbol("CRC_TABLE", "CRC_TABLE", 4096, ".rodata"),
    ])
}

#[test]
fn bytes_are_attributed_to_the_crate_they_came_from() {
    let attribution = attribute(&a_realistic_table(), &["app".to_string()]);
    let app = attribution.crates.iter().find(|group| group.key == "app").expect("app is present");
    assert_eq!(app.bytes, 1200 + 1800 + 900 + 400);
    assert_eq!(app.symbols, 4);
    assert!(!app.examples.is_empty());

    // And the list is largest first, because that is the order a reader wants.
    let sizes: Vec<u64> = attribution.crates.iter().map(|group| group.bytes).collect();
    let mut sorted = sizes.clone();
    sorted.sort_by(|a, b| b.cmp(a));
    assert_eq!(sizes, sorted);
}

#[test]
fn every_attributed_byte_is_counted_exactly_once_across_drivers() {
    let table = a_realistic_table();
    let attribution = attribute(&table, &["app".to_string()]);
    let across_drivers: u64 = attribution.drivers.iter().map(|(_, bytes)| bytes).sum();
    assert_eq!(
        across_drivers,
        table.total_bytes(),
        "a byte was double-counted or dropped between categories"
    );
}

#[test]
fn instantiations_of_one_generic_are_grouped_with_their_aggregate_cost() {
    // F1.3, and the top finding Phase 1's acceptance criterion asks for.
    let attribution = attribute(&a_realistic_table(), &["app".to_string()]);
    let tally = attribution
        .monomorphizations
        .iter()
        .find(|m| m.generic_path == "app::Tally")
        .expect("three instantiations were grouped");

    assert_eq!(tally.instantiations, 3);
    assert_eq!(tally.total_bytes, 1200 + 1800 + 900);
    // Largest instantiation first, so the reader sees the worst case.
    assert_eq!(tally.arguments[0].1, 1800);
    // Collapsing keeps the largest, so that is what cannot be saved.
    assert_eq!(tally.collapsible_bytes(), 1200 + 900);
}

#[test]
fn something_instantiated_once_is_not_a_monomorphization_problem() {
    let single = table(vec![symbol("app::Once<u8>::run", "_R1", 500, ".text")]);
    let attribution = attribute(&single, &["app".to_string()]);
    assert!(attribution.monomorphizations.is_empty(), "one instantiation is not a group");
}

#[test]
fn monomorphizations_are_ranked_by_what_they_cost_in_total() {
    // Not by instantiation count: twelve copies of a tiny function matter less
    // than three copies of a large one, and the ranking has to say so.
    let mixed = table(vec![
        symbol("app::Small<u8>::f", "_R1", 10, ".text"),
        symbol("app::Small<u16>::f", "_R2", 10, ".text"),
        symbol("app::Small<u32>::f", "_R3", 10, ".text"),
        symbol("app::Small<u64>::f", "_R4", 10, ".text"),
        symbol("app::Large<u8>::f", "_R5", 5000, ".text"),
        symbol("app::Large<u16>::f", "_R6", 5000, ".text"),
    ]);
    let attribution = attribute(&mixed, &["app".to_string()]);
    assert_eq!(attribution.monomorphizations[0].generic_path, "app::Large");
    assert!(attribution.monomorphizations[0].instantiations < 4);
}

#[test]
fn an_attribution_says_how_far_it_can_be_trusted() {
    let mut table = a_realistic_table();
    table.inferred_fraction = 0.42;
    let attribution = attribute(&table, &["app".to_string()]);

    // Both caveats travel with the result rather than being left implicit.
    assert_eq!(attribution.inferred_fraction, 0.42);
    assert!(
        attribution.generic_arguments_available,
        "these names carry their arguments, so grouping is real"
    );
}

#[test]
fn a_symbol_with_no_size_is_not_attributed_anywhere() {
    let with_zero = table(vec![
        symbol("app::real", "_R1", 100, ".text"),
        symbol("app::marker", "_R2", 0, ".text"),
    ]);
    let attribution = attribute(&with_zero, &["app".to_string()]);
    let app = attribution.crates.iter().find(|g| g.key == "app").unwrap();
    assert_eq!(app.symbols, 1, "a zero-size marker was counted as a symbol");
    assert_eq!(app.bytes, 100);
}

#[test]
fn an_empty_table_attributes_nothing_and_does_not_panic() {
    let attribution = attribute(&table(Vec::new()), &["app".to_string()]);
    assert_eq!(attribution.attributed_bytes, 0);
    assert!(attribution.crates.is_empty());
    assert!(attribution.monomorphizations.is_empty());
}
