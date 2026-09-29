fn main() {
    let path = std::env::args().nth(1).unwrap();
    let own: Vec<String> = std::env::args().skip(2).collect();
    let table = binmap_binary::SymbolTable::read(std::path::Path::new(&path)).unwrap();
    let a = binmap_binary::attribute(&table, &own);
    println!(
        "{} symbols, {} bytes attributed, {:.1}% sizes inferred, v0={}",
        table.symbols.len(),
        a.attributed_bytes,
        a.inferred_fraction * 100.0,
        a.generic_arguments_available
    );
    println!("\nby driver:");
    for (d, b) in a.drivers.iter().take(8) {
        println!("  {:<28} {:>10}", d.label(), b);
    }
    println!("\ntop crates:");
    for g in a.crates.iter().take(6) {
        println!("  {:<28} {:>10}  ({} symbols)", g.key, g.bytes, g.symbols);
    }
    println!("\ntop monomorphizations:");
    for m in a.monomorphizations.iter().take(5) {
        println!(
            "  {:<44} {:>8} over {} instantiations (collapsible {})",
            &m.generic_path[..m.generic_path.len().min(44)],
            m.total_bytes,
            m.instantiations,
            m.collapsible_bytes()
        );
    }
}
