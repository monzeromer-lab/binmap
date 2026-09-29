//! The binary. `--bench` gives the sweep something to time.

fn main() {
    let mut arguments = std::env::args().skip(1);
    match arguments.next().as_deref() {
        // A workload with a stable cost, sized so that it — rather than
        // process startup — is what gets measured.
        //
        // The first version ran for well under a millisecond, and a sweep over
        // it reported a 22.6% noise floor: entirely honest, and entirely
        // useless, because every comparison then falls inside it. If a
        // benchmark cannot outrun the cost of starting the process, it is
        // measuring the process.
        Some("--bench") => {
            let mut total: u64 = 0;
            for _ in 0..120_000 {
                total = total.wrapping_add(stress::exercise().len() as u64);
                total = total.wrapping_add(u64::from(stress::checksum(stress::table_bytes())));
            }
            // Printed so the work cannot be optimised away.
            println!("{total}");
        }
        _ => println!("{}", stress::exercise()),
    }
}
