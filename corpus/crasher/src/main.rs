//! A program that dies in a known way.
//!
//! The ground truth for crash analysis has to be *known*, so every path here
//! is deliberately shaped: a chain of calls deep enough that unwinding has
//! something to do, a generic in the middle so the frame has a mangled name
//! worth demangling, and an `#[inline(always)]` so there is an inlined frame
//! to recover that a naive unwinder would miss entirely.

use std::env;

/// The innermost frame. Inlined, so it does not appear in the frame list
/// unless inline frames are recovered from DWARF — which is `F2.3`'s point.
#[inline(always)]
fn divide(numerator: i64, denominator: i64) -> i64 {
    // A division by zero traps on x86-64 rather than panicking, which makes
    // this a genuine SIGFPE and not a Rust panic.
    numerator / denominator
}

/// A generic frame, so the stack has a mangled symbol worth demangling.
#[inline(never)]
fn compute<T: Into<i64>>(value: T, denominator: i64) -> i64 {
    divide(value.into(), denominator)
}

#[inline(never)]
fn middle(depth: u32, denominator: i64) -> i64 {
    if depth == 0 {
        return compute(42i32, denominator);
    }
    middle(depth - 1, denominator) + 1
}

#[inline(never)]
fn null_write() {
    // A deliberate null dereference, for the SIGSEGV case. Unsafe and
    // intentional: this binary exists to die.
    unsafe {
        let pointer: *mut u64 = std::ptr::null_mut();
        pointer.write_volatile(1);
    }
}

#[inline(never)]
fn explicit_panic(depth: u32) -> ! {
    if depth > 0 {
        explicit_panic(depth - 1)
    } else {
        panic!("the crasher panicked on purpose, at a known line");
    }
}

/// Runs forever, so `gcore` can dump it while it is alive and the stack is at
/// a known depth.
#[inline(never)]
fn wait_to_be_dumped(depth: u32) {
    if depth > 0 {
        wait_to_be_dumped(depth - 1);
        return;
    }
    println!("ready");
    loop {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

fn main() {
    let how = env::args().nth(1).unwrap_or_else(|| "wait".to_string());
    match how.as_str() {
        "divide" => {
            println!("{}", middle(3, 0));
        }
        "segv" => null_write(),
        "panic" => explicit_panic(3),
        // The default: stay alive at a known stack depth so a core can be
        // taken from a live process, which needs no change to core_pattern.
        _ => wait_to_be_dumped(4),
    }
}
