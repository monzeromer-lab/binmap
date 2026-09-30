//! Profile a program, for development.

fn main() {
    let mut args = std::env::args().skip(1);
    let program = args.next().expect("usage: sample <program> [args...]");
    let rest: Vec<String> = args.collect();

    let plan = binmap_perf::sampler::Plan::default();
    let (profile, modules) =
        binmap_perf::sampler::profile(std::path::Path::new(&program), &rest, plan)
            .expect("profiling works");

    println!("{}", profile.describe());
    println!("{} modules loaded", modules.loaded.len());

    let mut by_module: std::collections::BTreeMap<String, Vec<u64>> = Default::default();
    for stack in profile.stacks.keys() {
        for address in &stack.addresses {
            if let Some((module, link_time)) = modules
                .containing(*address)
                .and_then(|module| module.to_link_time(*address).map(|link| (module, link)))
            {
                by_module.entry(module.path.clone()).or_default().push(link_time);
            }
        }
    }
    let symbolizers: std::collections::BTreeMap<String, _> = by_module
        .iter()
        .filter_map(|(module, addresses)| {
            binmap_crash::symbolize::Symbolizer::load(std::path::Path::new(module), addresses)
                .ok()
                .map(|s| (module.clone(), s))
        })
        .collect();

    let attributed = binmap_perf::attribute::attribute(&profile, |address| {
        modules
            .containing(address)
            .and_then(|module| module.to_link_time(address).map(|link| (module.path.clone(), link)))
            .and_then(|(path, link)| symbolizers.get(&path).map(|s| s.resolve(link)))
            .unwrap_or_default()
    });

    println!("\n{}", attributed.describe());
    println!("\nhottest by self time:");
    for hot in attributed.hot.iter().take(12) {
        println!("  {}", hot.describe(attributed.total_samples));
    }
}
