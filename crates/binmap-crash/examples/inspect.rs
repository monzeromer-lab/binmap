//! Inspect a core dump against its binary, for development.

fn main() {
    let mut args = std::env::args().skip(1);
    let core_path = args.next().expect("usage: inspect <core> <binary>");
    let binary_path = args.next().expect("usage: inspect <core> <binary>");

    let core_data = std::fs::read(&core_path).unwrap();
    let binary = std::fs::read(&binary_path).unwrap();
    let dump = binmap_crash::CoreDump::parse(&core_data).unwrap();
    let thread = dump.crashing_thread();

    println!("signal {} in pid {}", thread.signal, thread.pid);
    let verdict = binmap_crash::correspondence::verify(&dump, &core_data, &binary, &binary_path)
        .expect("verifiable");
    println!("{}", verdict.describe());
    let bias = binmap_crash::bias::derive(&dump, &binary).expect("a bias");
    println!("{}", bias.derivation.describe());

    let name = std::path::Path::new(&binary_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let modules = binmap_crash::modules::Modules::load(&dump, Some((&name, &binary)));
    println!(
        "{} modules loaded, {} unavailable\n",
        modules.loaded.len(),
        modules.unavailable.len()
    );

    let stack =
        binmap_crash::unwind::walk(&dump, &core_data, &modules, &thread.registers).expect("stack");
    println!("{}\n", stack.describe());

    // One symbolizer per module, built from the frames that landed in it.
    let mut resolved_frames = Vec::new();
    for frame in &stack.frames {
        let Some((module, address)) = frame.module.as_deref().zip(frame.link_time_address) else {
            resolved_frames.push(binmap_crash::symbolize::Resolved::default());
            continue;
        };
        let symbolizer =
            binmap_crash::symbolize::Symbolizer::load(std::path::Path::new(module), &[address]);
        resolved_frames.push(match symbolizer {
            Ok(symbolizer) => symbolizer.resolve(address),
            Err(_) => binmap_crash::symbolize::Resolved::default(),
        });
    }

    let crash = binmap_crash::classify::classify(&dump, thread, &resolved_frames);
    println!("classified: {}", crash.title());
    println!("look at: {}\n", crash.what_to_look_at());

    for (index, (frame, resolved)) in stack.frames.iter().zip(&resolved_frames).enumerate() {
        let short = frame.module.as_deref().and_then(|m| m.rsplit('/').next()).unwrap_or("?");
        if resolved.is_empty() {
            println!(
                "#{index:<2} {:#018x}  <{short}>  [{}]",
                frame.runtime_address,
                frame.method.label()
            );
            continue;
        }
        for (depth, location) in resolved.locations.iter().enumerate() {
            let marker = if location.inlined { "  (inlined)" } else { "" };
            if depth == 0 {
                println!(
                    "#{index:<2} {:#018x}  {}{marker}  [{}]",
                    frame.runtime_address,
                    location.describe(),
                    frame.method.label()
                );
            } else {
                println!("{:22}{}{marker}", "", location.describe());
            }
        }
    }
}
