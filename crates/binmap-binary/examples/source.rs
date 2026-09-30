fn main() {
    let path = std::env::args().nth(1).unwrap();
    let p = std::path::Path::new(&path);
    let table = binmap_binary::SymbolTable::read(p).unwrap();
    let sample: Vec<_> = table.symbols.iter().take(3000).cloned().collect();
    let map = binmap_binary::SourceMap::resolve(p, &sample).unwrap();
    println!(
        "debug info: {}  mapped {} of {} ({:.1}%)",
        map.has_debug_info,
        map.len(),
        sample.len(),
        map.coverage(&sample) * 100.0
    );
    let mut shown = 0;
    for s in &sample {
        if let Some(o) = map.of(s) {
            if o.inlined.is_empty() && shown >= 6 {
                continue;
            }
            println!(
                "  {:<48} {}:{}",
                &s.name[..s.name.len().min(48)],
                o.file.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default(),
                o.line
            );
            for f in o.inlined.iter().take(3) {
                println!("      inlined: {}", &f.function[..f.function.len().min(60)]);
            }
            shown += 1;
            if shown >= 12 {
                break;
            }
        }
    }
}
