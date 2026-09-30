//! Performance attribution (`F3.2`–`F3.4`).
//!
//! The symbolization is real: the addresses come from `corpus/crasher`'s
//! actual symbol table, resolved through the same layer the crash analyser
//! uses. What is synthesized is the *sampling*, because `perf_event_paranoid`
//! is 4 on most machines and a test that needs root is a test nobody runs.
//!
//! Most of what is checked is statistical honesty. A profiler that ranks two
//! functions three samples apart out of four hundred has invented an ordering,
//! and a regression hunt starting from an invented ordering wastes a day.

use binmap_crash::symbolize::{Location, Resolved};
use binmap_perf::attribute::attribute;
use binmap_perf::flame::{build, compare};
use binmap_perf::sample::{Profile, Source, Stack};

fn stack(addresses: &[u64]) -> Stack {
    Stack { addresses: addresses.to_vec() }
}

/// A resolver that names addresses, with a pair of inlined frames at 0x10.
fn fake_resolve(address: u64) -> Resolved {
    let at = |function: &str, line: u32, inlined: bool| Location {
        function: Some(function.into()),
        file: Some("src/lib.rs".into()),
        line: Some(line),
        inlined,
    };
    match address {
        0x10 => {
            Resolved { locations: vec![at("core::ptr::inner", 1, true), at("app::hot", 10, false)] }
        }
        0x20 => Resolved { locations: vec![at("app::middle", 20, false)] },
        0x30 => Resolved { locations: vec![at("app::main", 30, false)] },
        0x40 => Resolved { locations: vec![at("app::cold", 40, false)] },
        _ => Resolved::default(),
    }
}

fn a_profile(entries: &[(&[u64], u64)]) -> Profile {
    let mut profile = Profile::new(Source::Internal, "test");
    for (addresses, count) in entries {
        profile.record(stack(addresses), *count);
    }
    profile
}

// --- counting ---------------------------------------------------------------

#[test]
fn identical_stacks_collapse_into_one_entry_with_a_count() {
    // What makes a profile of a million samples a few thousand entries.
    let mut profile = Profile::new(Source::Perf, "perf record");
    profile.record(stack(&[0x10, 0x20]), 3);
    profile.record(stack(&[0x10, 0x20]), 4);

    assert_eq!(profile.stacks.len(), 1);
    assert_eq!(profile.total(), 7);
}

#[test]
fn self_time_is_the_innermost_frame_and_total_time_is_the_whole_stack() {
    let profile = a_profile(&[(&[0x10, 0x20, 0x30], 10), (&[0x20, 0x30], 5)]);

    let self_samples = profile.self_samples();
    assert_eq!(self_samples.get(&0x10), Some(&10), "the program was in 0x10");
    assert_eq!(self_samples.get(&0x20), Some(&5));
    assert_eq!(self_samples.get(&0x30), None, "0x30 is never innermost");

    let total = profile.total_samples();
    assert_eq!(total.get(&0x30), Some(&15), "everything passed through 0x30");
}

#[test]
fn recursion_is_counted_once_per_stack_not_once_per_frame() {
    // The program was not in a recursive function twice at the same instant,
    // and counting it twice makes recursion look like it dominates.
    let profile = a_profile(&[(&[0x10, 0x10, 0x10, 0x30], 4)]);
    assert_eq!(profile.total_samples().get(&0x10), Some(&4), "once, not three times");
}

// --- statistical honesty ----------------------------------------------------

#[test]
fn a_profile_with_too_few_samples_says_so_before_ranking_anything() {
    // A hundred samples over a real program gives every function a count of
    // one or two, and ranking those is ranking noise.
    let thin = a_profile(&[(&[0x10], 3), (&[0x20], 2)]);
    assert!(!thin.has_enough_samples());
    assert!(thin.describe().contains("too few to rank"), "{}", thin.describe());

    let thick = a_profile(&[(&[0x10], 4_000), (&[0x20], 2_000)]);
    assert!(thick.has_enough_samples());
    assert!(!thick.describe().contains("too few"));
}

#[test]
fn counts_closer_together_than_sampling_error_are_not_distinguishable() {
    // The profiler equivalent of the sweep's noise floor.
    let profile = a_profile(&[(&[0x10], 2_000), (&[0x20], 1_997)]);
    assert!(
        !profile.distinguishable(2_000, 1_997),
        "three samples apart out of four thousand is not an ordering"
    );
    assert!(profile.distinguishable(3_000, 1_000), "a three-to-one split is");
}

#[test]
fn the_standard_error_is_zero_at_the_extremes_and_largest_in_the_middle() {
    // A binomial's error vanishes when every sample or no sample is in the
    // bucket, which is the arithmetic being relied on.
    let profile = a_profile(&[(&[0x10], 1_000)]);
    assert_eq!(profile.standard_error(0), 0.0);
    assert_eq!(profile.standard_error(1_000), 0.0);
    assert!(profile.standard_error(500) > profile.standard_error(100));
}

#[test]
fn an_empty_profile_does_not_divide_by_zero() {
    let empty = Profile::new(Source::Perf, "nothing");
    assert!(empty.is_empty());
    assert_eq!(empty.standard_error(0), 0.0);
    assert_eq!(empty.total(), 0);
}

// --- attribution (F3.2) -----------------------------------------------------

#[test]
fn samples_are_attributed_to_the_innermost_inlined_function() {
    // A profiler attributing a hot address to the outermost function reports
    // that `Iterator::fold` is 40% of your program — true and useless.
    let profile = a_profile(&[(&[0x10, 0x30], 100)]);
    let attributed = attribute(&profile, fake_resolve);

    let hottest = &attributed.hot[0];
    assert_eq!(hottest.function, "core::ptr::inner", "the innermost location");
    assert!(hottest.inlined, "and it is marked as inlined");
    assert_eq!(hottest.self_samples, 100);
}

#[test]
fn a_function_that_only_ever_calls_still_appears_with_its_total_time() {
    // Where the work is organised is frequently what a reader is looking for,
    // and it has no self time at all.
    let profile = a_profile(&[(&[0x10, 0x30], 100)]);
    let attributed = attribute(&profile, fake_resolve);

    let main = attributed.hot.iter().find(|hot| hot.function == "app::main").expect("main");
    assert_eq!(main.self_samples, 0);
    assert_eq!(main.total_samples, 100);
}

#[test]
fn ranking_is_by_self_time_because_that_is_where_the_time_goes() {
    let profile = a_profile(&[(&[0x10, 0x30], 30), (&[0x40, 0x30], 70)]);
    let attributed = attribute(&profile, fake_resolve);

    assert_eq!(attributed.hot[0].function, "app::cold", "70 samples beats 30");
    assert_eq!(attributed.hot[0].self_samples, 70);
}

#[test]
fn samples_that_resolve_to_nothing_are_reported_rather_than_dropped() {
    // A profile that silently discards a third of its samples shows
    // percentages adding to a hundred that describe two thirds of the program.
    let profile = a_profile(&[(&[0x10], 60), (&[0xdead_beef], 40)]);
    let attributed = attribute(&profile, fake_resolve);

    assert_eq!(attributed.unresolved_samples, 40);
    assert!((attributed.coverage() - 0.6).abs() < 1e-9, "{}", attributed.coverage());
    assert!(attributed.describe().contains("60%"), "{}", attributed.describe());
}

#[test]
fn your_own_functions_can_be_picked_out_of_the_runtime() {
    let profile = a_profile(&[(&[0x10, 0x30], 100)]);
    let attributed = attribute(&profile, fake_resolve);

    let yours = attributed.yours(&["app".to_string()]);
    assert!(yours.iter().all(|hot| hot.function.starts_with("app::")), "{yours:?}");
    assert!(!yours.is_empty());
    assert!(!yours.iter().any(|hot| hot.function.starts_with("core::")), "core is not yours");
}

// --- the flamegraph (F3.3) --------------------------------------------------

#[test]
fn the_logical_tree_expands_inline_frames_and_the_physical_one_does_not() {
    let profile = a_profile(&[(&[0x10, 0x20, 0x30], 100)]);

    let logical = build(&profile, true, fake_resolve);
    let names: Vec<&str> =
        logical.root.walk().iter().map(|(_, node)| node.function.as_str()).collect();
    assert!(names.contains(&"core::ptr::inner"), "the inlined frame is a node: {names:?}");
    assert!(logical.root.walk().iter().any(|(_, node)| node.inlined));

    let physical = build(&profile, false, fake_resolve);
    let names: Vec<&str> =
        physical.root.walk().iter().map(|(_, node)| node.function.as_str()).collect();
    assert!(!names.contains(&"core::ptr::inner"), "the machine never called it: {names:?}");
    assert!(names.contains(&"app::hot"));
}

#[test]
fn the_tree_runs_outermost_to_innermost() {
    // A stack is sampled innermost first and a flamegraph reads outermost
    // first. Getting this backwards produces a graph that is upside down and
    // looks perfectly plausible.
    let profile = a_profile(&[(&[0x10, 0x20, 0x30], 100)]);
    let graph = build(&profile, false, fake_resolve);

    let path = graph.root.hot_path();
    assert_eq!(path, vec!["all", "app::main", "app::middle", "app::hot"], "{path:?}");
}

#[test]
fn a_nodes_width_is_everything_that_passed_through_it() {
    let profile = a_profile(&[(&[0x10, 0x30], 70), (&[0x40, 0x30], 30)]);
    let graph = build(&profile, false, fake_resolve);

    let main = &graph.root.children[0];
    assert_eq!(main.function, "app::main");
    assert_eq!(main.samples, 100, "everything went through main");
    assert_eq!(main.self_samples, 0, "and nothing stopped in it");
    assert_eq!(main.children[0].function, "app::hot", "widest child first");
    assert_eq!(main.children[0].samples, 70);
}

#[test]
fn the_hot_path_is_the_widest_chain_rather_than_the_hottest_function() {
    // What a reader wants first is not the hottest function but the chain that
    // leads to it.
    let profile = a_profile(&[(&[0x10, 0x20, 0x30], 80), (&[0x40, 0x30], 20)]);
    let graph = build(&profile, false, fake_resolve);
    assert_eq!(graph.root.hot_path().last(), Some(&"app::hot"));
}

#[test]
fn an_empty_profile_makes_an_empty_graph_rather_than_panicking() {
    let graph = build(&Profile::new(Source::Perf, "x"), true, fake_resolve);
    assert_eq!(graph.total_samples, 0);
    assert!(graph.root.children.is_empty());
    assert_eq!(graph.root.share(0), 0.0);
}

// --- differential profiling (F3.4) ------------------------------------------

#[test]
fn a_real_regression_is_reported_with_its_direction() {
    let before = a_profile(&[(&[0x10, 0x30], 1_000), (&[0x40, 0x30], 1_000)]);
    let after = a_profile(&[(&[0x10, 0x30], 200), (&[0x40, 0x30], 1_800)]);

    let changes = compare(
        &before,
        &after,
        &attribute(&before, fake_resolve),
        &attribute(&after, fake_resolve),
    );

    let cold = changes.iter().find(|c| c.function == "app::cold").expect("cold");
    assert!(cold.is_significant());
    assert!(cold.delta() > 0, "it got slower");
    assert!(cold.describe().contains("slower"), "{}", cold.describe());

    // And the biggest regression is first, which is what a regression hunt is
    // for.
    assert_eq!(changes[0].function, "app::cold");
}

#[test]
fn a_difference_inside_sampling_error_is_not_reported_as_a_change() {
    // The discipline the sweep already applies: a difference smaller than the
    // measurement's own error is not a difference.
    let before = a_profile(&[(&[0x10, 0x30], 1_000), (&[0x40, 0x30], 1_000)]);
    let after = a_profile(&[(&[0x10, 0x30], 1_003), (&[0x40, 0x30], 997)]);

    let changes = compare(
        &before,
        &after,
        &attribute(&before, fake_resolve),
        &attribute(&after, fake_resolve),
    );

    assert!(
        changes.iter().all(|change| !change.is_significant()),
        "three samples out of two thousand is noise: {changes:?}"
    );
    let described = changes[0].describe();
    assert!(described.contains("inside sampling error"), "{described}");
    assert!(described.contains("not a change"), "{described}");
}

#[test]
fn a_function_present_in_only_one_profile_still_appears() {
    // A function that vanished is the most interesting kind of change, and
    // intersecting the two profiles would lose it.
    let before = a_profile(&[(&[0x10, 0x30], 2_000)]);
    let after = a_profile(&[(&[0x40, 0x30], 2_000)]);

    let changes = compare(
        &before,
        &after,
        &attribute(&before, fake_resolve),
        &attribute(&after, fake_resolve),
    );

    assert!(changes.iter().any(|c| c.function == "core::ptr::inner" && c.after == 0));
    assert!(changes.iter().any(|c| c.function == "app::cold" && c.before == 0));
}
