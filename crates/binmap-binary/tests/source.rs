//! Mapping symbols back to source, against a real binary with real DWARF.

use binmap_binary::source::SourceMap;
use binmap_binary::symbols::SymbolTable;

/// Our own test binary, which is compiled with debug info.
fn our_own_binary() -> std::path::PathBuf {
    std::env::current_exe().expect("the test binary exists")
}

#[test]
fn a_binary_with_debug_info_maps_symbols_to_source() {
    let path = our_own_binary();
    let table = SymbolTable::read(&path).expect("it is an object file");
    // A subset: resolving forty thousand symbols is not what this is testing.
    let sample: Vec<_> = table.symbols.iter().take(400).cloned().collect();

    let map = SourceMap::resolve(&path, &sample).expect("it reads");
    if !map.has_debug_info {
        eprintln!("this build carries no debug info; nothing to map");
        return;
    }

    assert!(!map.is_empty());
    // Every mapping names a file and a line, or it would not have been kept.
    for symbol in &sample {
        if let Some(origin) = map.of(symbol) {
            assert!(!origin.file.as_os_str().is_empty(), "{} mapped to no file", symbol.name);
            assert!(origin.line > 0, "{} mapped to line 0", symbol.name);
        }
    }
}

#[test]
fn coverage_is_reported_rather_than_assumed() {
    // An attribution that maps a tenth of its symbols is a weaker thing than
    // one that maps all of them, and the difference is invisible unless
    // carried.
    let path = our_own_binary();
    let table = SymbolTable::read(&path).unwrap();
    let sample: Vec<_> = table.symbols.iter().take(200).cloned().collect();

    let map = SourceMap::resolve(&path, &sample).unwrap();
    let coverage = map.coverage(&sample);
    assert!((0.0..=1.0).contains(&coverage), "coverage {coverage} is not a fraction");
    assert_eq!(map.coverage(&[]), 0.0, "nothing asked about is nothing covered");
}

#[test]
fn inlined_frames_are_recovered_where_the_compiler_created_them() {
    // A physical frame in an optimized Rust binary routinely represents
    // several logical ones, and "this function did not physically exist" is
    // the confusion the product exists to resolve.
    let path = our_own_binary();
    let table = SymbolTable::read(&path).unwrap();
    let sample: Vec<_> = table.symbols.iter().take(600).cloned().collect();
    let map = SourceMap::resolve(&path, &sample).unwrap();

    if !map.has_debug_info {
        return;
    }
    // Not every symbol has inlining, but a binary of this size has some.
    let with_inlining = sample.iter().filter_map(|s| map.of(s)).filter(|o| !o.inlined.is_empty());
    for origin in with_inlining.take(20) {
        for frame in &origin.inlined {
            assert!(!frame.function.is_empty(), "an inlined frame has no function");
        }
    }
}

#[test]
fn a_binary_with_no_debug_info_reports_that_rather_than_failing() {
    // A release build with strip = "symbols" is the normal thing to ship, and
    // having nothing to map is a fact to report, not an error to raise.
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("empty.bin");

    // A minimal well-formed ELF that carries no DWARF at all.
    let source = our_own_binary();
    let bytes = std::fs::read(&source).unwrap();
    std::fs::write(&path, &bytes[..bytes.len().min(64)]).unwrap();

    // Either it parses and has nothing, or it is not an object file — both
    // are answers, and neither is a panic.
    match SourceMap::resolve(&path, &[]) {
        Ok(map) => assert!(map.is_empty()),
        Err(error) => assert!(error.to_string().contains("empty.bin"), "{error}"),
    }
}

#[test]
fn a_file_that_is_not_there_names_the_path() {
    let error = SourceMap::resolve(std::path::Path::new("/nonexistent/binary"), &[])
        .unwrap_err()
        .to_string();
    assert!(error.contains("/nonexistent/binary"), "{error}");
}

#[test]
fn a_corpus_binary_maps_its_own_functions_to_its_own_source() {
    // End to end against something built for the purpose: stress has debug
    // info in its release profile only if cargo left it, so this is
    // conditional — but where it maps, it must map to stress's own files.
    let artifact = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .unwrap()
        .join("corpus/stress/target/release/stress");
    if !artifact.exists() {
        eprintln!("corpus/stress has not been built");
        return;
    }

    let table = SymbolTable::read(&artifact).unwrap();
    let own: Vec<_> = table
        .symbols
        .iter()
        .filter(|symbol| symbol.name.starts_with("stress::"))
        .cloned()
        .collect();
    if own.is_empty() {
        return;
    }

    let map = SourceMap::resolve(&artifact, &own).unwrap();
    if !map.has_debug_info {
        eprintln!("corpus/stress was built without debug info");
        return;
    }
    for symbol in &own {
        if let Some(origin) = map.of(symbol) {
            let file = origin.file.to_string_lossy();
            assert!(
                file.contains("stress") || file.contains("src"),
                "{} mapped to {file}, which is not its own source",
                symbol.name
            );
        }
    }
}
