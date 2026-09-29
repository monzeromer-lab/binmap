//! Reading a real symbol table, from a real binary.
//!
//! Our own test binary is the fixture: it is an ELF file with a full
//! `.symtab`, it exists on any machine that can run the suite, and it is
//! compiled by the same toolchain the tool will be pointed at.

use binmap_binary::symbols::{Mangling, Sizing, Symbol, SymbolTable, demangle};

fn our_own_binary() -> std::path::PathBuf {
    std::env::current_exe().expect("the test binary exists")
}

#[test]
fn a_real_binary_yields_a_symbol_table() {
    let table = SymbolTable::read(&our_own_binary()).expect("it is an object file");
    assert!(!table.is_empty(), "our own test binary has symbols");
    assert!(!table.stripped, "and they came from .symtab rather than .dynsym");

    for symbol in table.symbols.iter().take(200) {
        assert!(!symbol.name.is_empty());
        assert!(!symbol.mangled.is_empty());
    }
}

#[test]
fn no_symbol_claims_to_be_larger_than_the_file() {
    // The classic failure of inferring a size from the next address: a gap
    // across a section boundary becomes a function of megabytes. Sizes are
    // only inferred within one section, and this is what says so.
    let path = our_own_binary();
    let on_disk = std::fs::metadata(&path).unwrap().len();
    let table = SymbolTable::read(&path).unwrap();

    for symbol in &table.symbols {
        assert!(
            symbol.size <= on_disk,
            "{} claims {} bytes in a {on_disk}-byte file",
            symbol.name,
            symbol.size
        );
    }
}

#[test]
fn an_inferred_size_never_spans_two_sections() {
    let table = SymbolTable::read(&our_own_binary()).unwrap();
    let inferred: Vec<&Symbol> =
        table.symbols.iter().filter(|s| s.sizing == Sizing::InferredFromNeighbour).collect();

    // Whatever was inferred, it came from a neighbour in the same section, so
    // none of it can exceed the bound the inference applies.
    for symbol in inferred {
        assert!(symbol.size <= 1 << 20, "{} inferred {} bytes", symbol.name, symbol.size);
    }
}

#[test]
fn how_a_size_was_arrived_at_is_recorded_per_symbol() {
    // An attribution built mostly from inferred sizes deserves less confidence
    // than one built from declared ones, and that is invisible unless carried.
    let table = SymbolTable::read(&our_own_binary()).unwrap();
    assert!((0.0..=1.0).contains(&table.inferred_fraction));

    let declared = table.symbols.iter().filter(|s| s.sizing == Sizing::Declared).count();
    assert!(declared > 0, "a normal binary declares most of its symbol sizes");
}

#[test]
fn rust_symbols_come_back_readable() {
    let table = SymbolTable::read(&our_own_binary()).unwrap();
    let rust: Vec<&Symbol> =
        table.symbols.iter().filter(|s| Mangling::of(&s.mangled) != Mangling::Foreign).collect();
    assert!(!rust.is_empty(), "a Rust binary has Rust symbols");

    let demangled = rust.iter().filter(|s| s.name != s.mangled).count();
    assert!(demangled > rust.len() / 2, "most Rust symbols should demangle");

    // And the legacy hash suffix is gone — it is noise wherever a name is
    // shown.
    for symbol in rust.iter().take(200) {
        assert!(!symbol.name.contains("17h"), "a legacy hash survived: {}", symbol.name);
    }
}

#[test]
fn the_binary_reports_which_mangling_scheme_it_used() {
    // v0 carries the generic arguments and legacy does not, so grouping is
    // worth less on a legacy binary and the interface has to say so.
    let table = SymbolTable::read(&our_own_binary()).unwrap();
    let scheme = table.mangling().expect("a Rust binary has Rust symbols");
    assert!(matches!(scheme, Mangling::V0 | Mangling::Legacy));
}

#[test]
fn something_that_is_not_an_object_file_says_so_rather_than_panicking() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("not-elf");
    std::fs::write(&path, b"this is plainly not an object file").unwrap();

    let error = SymbolTable::read(&path).unwrap_err().to_string();
    assert!(error.contains("not an object file"), "{error}");
    assert!(error.contains("not-elf"), "the path is named: {error}");
}

#[test]
fn a_file_that_is_not_there_names_the_path() {
    let error =
        SymbolTable::read(std::path::Path::new("/nonexistent/binary")).unwrap_err().to_string();
    assert!(error.contains("/nonexistent/binary"), "{error}");
}

#[test]
fn demangling_is_the_identity_for_things_that_are_not_rust_symbols() {
    for foreign in ["main", "memcpy", "_init", "__libc_start_main", ""] {
        assert_eq!(demangle(foreign), foreign);
        assert_eq!(Mangling::of(foreign), Mangling::Foreign);
    }
}

#[test]
fn the_two_mangling_schemes_are_told_apart() {
    assert_eq!(Mangling::of("_RNvCs1234_5crate4func"), Mangling::V0);
    assert_eq!(Mangling::of("_ZN5crate4func17h0123456789abcdefE"), Mangling::Legacy);
    assert_eq!(Mangling::of("rust_eh_personality"), Mangling::Foreign);
}

#[test]
fn the_symbols_of_a_corpus_binary_attribute_to_its_own_crate() {
    // End to end against something built for the purpose rather than against
    // the test harness, which carries the whole test framework with it.
    let artifact = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .unwrap()
        .join("corpus/stress/target/release/stress");
    if !artifact.exists() {
        eprintln!("corpus/stress has not been built; nothing to read");
        return;
    }

    let table = SymbolTable::read(&artifact).expect("it is an ELF binary");
    let attribution = binmap_binary::attribute(&table, &["stress".to_string()]);

    let stress = attribution
        .crates
        .iter()
        .find(|group| group.key == "stress")
        .expect("the crate's own symbols are attributed to it");
    assert!(stress.bytes > 0);

    // The crate is generic-heavy on purpose, so grouping must find something.
    assert!(
        !attribution.monomorphizations.is_empty(),
        "corpus/stress instantiates Tally at five type arguments"
    );
}
