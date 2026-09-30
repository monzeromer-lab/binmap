//! A program with a known hot function.
//!
//! Phase 3's acceptance criterion is that an injected regression of 10% or
//! more has its responsible function identified in the top three. That needs a
//! program where "responsible" has an answer, so each worker here does a
//! measurable, separable amount of work and the environment says which one is
//! made slower.
//!
//! The work is deliberately arithmetic rather than allocation: an allocator
//! shows up as `malloc` in every profile and would dominate the ranking with
//! something none of these functions is responsible for.

use std::env;

/// How much work each worker does, scaled by its own multiplier.
///
/// Read from the environment so a regression can be injected without
/// rebuilding — the same binary profiled twice is the only way to know a
/// difference came from the change rather than from the compiler.
fn weight(name: &str) -> u64 {
    env::var(format!("HOTLOOP_{}", name.to_uppercase()))
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(100)
}

#[inline(never)]
fn checksum(rounds: u64) -> u64 {
    let mut state = 0x2545_F491_4F6C_DD1Du64;
    for _ in 0..rounds * 1_000 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
    }
    state
}

#[inline(never)]
fn transform(rounds: u64) -> u64 {
    let mut total = 0u64;
    for index in 0..rounds * 1_000 {
        total = total.wrapping_add(index.wrapping_mul(2_654_435_761));
        total = total.rotate_left(7);
    }
    total
}

#[inline(never)]
fn compress(rounds: u64) -> u64 {
    let mut state = 1u64;
    for index in 0..rounds * 1_000 {
        state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(index);
        state ^= state >> 33;
    }
    state
}

#[inline(never)]
fn validate(rounds: u64) -> u64 {
    let mut count = 0u64;
    for index in 0..rounds * 1_000 {
        if index.count_ones() % 3 == 0 {
            count = count.wrapping_add(index);
        }
    }
    count
}

fn main() {
    let checksum_weight = weight("checksum");
    let transform_weight = weight("transform");
    let compress_weight = weight("compress");
    let validate_weight = weight("validate");

    let mut total = 0u64;
    loop {
        // Each argument goes through `black_box`, or LLVM hoists the whole
        // call out of the loop: these are pure functions of their arguments,
        // so it computed each once and left the loop body containing nothing
        // but `black_box(total)`. The first version of this profiled as 100%
        // `main`, and the profiler was right — there was nothing else running.
        total = total.wrapping_add(checksum(std::hint::black_box(checksum_weight)));
        total = total.wrapping_add(transform(std::hint::black_box(transform_weight)));
        total = total.wrapping_add(compress(std::hint::black_box(compress_weight)));
        total = total.wrapping_add(validate(std::hint::black_box(validate_weight)));
        std::hint::black_box(total);
    }
}
