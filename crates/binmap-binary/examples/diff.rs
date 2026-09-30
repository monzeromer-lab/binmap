fn main() {
    let mut args = std::env::args().skip(1);
    let (before, after) = (args.next().unwrap(), args.next().unwrap());
    let own: Vec<String> = args.collect();
    let b = binmap_binary::SymbolTable::read(std::path::Path::new(&before)).unwrap();
    let a = binmap_binary::SymbolTable::read(std::path::Path::new(&after)).unwrap();
    let d = binmap_binary::diff(&b, &a, &own);
    println!("{} -> {} bytes ({:+})", d.before_bytes, d.after_bytes, d.total_delta());
    println!("{} matched only by ignoring generic arguments", d.matched_by_generic);
    println!("\nby category:");
    for (driver, delta) in d.by_driver.iter().take(8) {
        println!("  {:<30} {:>+10}", driver.label(), delta);
    }
    println!("\ngrew most:");
    for c in d.grew.iter().take(6) {
        println!(
            "  {:<50} {:>+9}  ({} -> {})",
            &c.name[..c.name.len().min(50)],
            c.delta,
            c.before,
            c.after
        );
    }
    println!("\nshrank most:");
    for c in d.shrank.iter().take(6) {
        println!("  {:<50} {:>+9}", &c.name[..c.name.len().min(50)], c.delta);
    }
    println!("\nadded: {}   removed: {}", d.added.len(), d.removed.len());
}
