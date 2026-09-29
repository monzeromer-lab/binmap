//! Property tests.
//!
//! §8 asks for these and none existed. They matter most here because
//! `binmap-measure` is where the arithmetic lives, and arithmetic is where a
//! hand-written test happens to pick the inputs that work.
//!
//! Each property below is a rule the product states somewhere in prose. A
//! property test is how prose becomes enforceable.

use binmap_measure::pareto::{Point, frontier, frontier_within};
use binmap_measure::timing::{NoiseFloor, Samples};
use binmap_verify::BenchmarkVerdict;
use proptest::prelude::*;
use std::time::Duration;

/// Sizes and times in ranges a real sweep produces, so a shrunk failure
/// reports something recognisable.
fn a_point() -> impl Strategy<Value = Point> {
    (
        1u64..10_000_000,
        prop::option::of(1u64..1_000_000_000),
        prop::option::of(1u64..600_000_000_000),
        any::<bool>(),
        0usize..1000,
    )
        .prop_map(|(size, runtime, build, eligible, id)| {
            let mut point = Point::new(format!("cfg-{id}"), size);
            if let Some(runtime) = runtime {
                point = point.with_runtime(runtime);
            }
            if let Some(build) = build {
                point = point.with_build_time(build);
            }
            if !eligible {
                point = point.ineligible();
            }
            point
        })
}

fn some_points() -> impl Strategy<Value = Vec<Point>> {
    prop::collection::vec(a_point(), 0..12)
}

proptest! {
    /// Dominance must not depend on which point is asked.
    ///
    /// The relation was asymmetric once — a point missing an axis could not
    /// dominate one that had it — and the frontier changed with the order the
    /// points arrived in. This is that bug, generalised.
    #[test]
    fn dominance_is_antisymmetric(points in some_points()) {
        for a in &points {
            for b in &points {
                prop_assert!(
                    !(a.dominates(b) && b.dominates(a)),
                    "{} and {} dominate each other", a.configuration, b.configuration
                );
            }
        }
    }

    /// Nothing dominates itself, whatever it measured.
    #[test]
    fn dominance_is_irreflexive(points in some_points()) {
        for point in &points {
            prop_assert!(!point.dominates(point), "{} dominates itself", point.configuration);
        }
    }

    /// The frontier is exactly the eligible points nothing dominates — no
    /// more, and no fewer.
    #[test]
    fn the_frontier_is_what_survives_domination(points in some_points()) {
        let on = frontier(&points);
        for (index, point) in points.iter().enumerate() {
            let dominated = points
                .iter()
                .enumerate()
                .any(|(other, candidate)| other != index && candidate.dominates(point));
            prop_assert_eq!(on.contains(&index), point.eligible && !dominated);
        }
    }

    /// A candidate its gates rejected never reaches the frontier, however good
    /// its numbers are.
    #[test]
    fn an_ineligible_point_is_never_on_the_frontier(points in some_points()) {
        for index in frontier(&points) {
            prop_assert!(points[index].eligible);
        }
    }

    /// Noise tolerance moves the frontier in **both** directions, and that is
    /// correct.
    ///
    /// This property was originally written the other way round — "a larger
    /// noise floor never shrinks the frontier" — and proptest refuted it in
    /// under a second with three points. The reasoning behind the guess was
    /// that merging runtimes can only make fewer points dominate. It can also
    /// make *more*:
    ///
    /// ```text
    /// A: 50 bytes, 100 ns        B: 100 bytes, 90 ns
    /// ```
    ///
    /// Strictly, A wins on size and B wins on runtime, so both survive. Tell
    /// the comparison that 10 ns is inside the machine's noise and the
    /// runtimes are the same number — at which point A is smaller and no
    /// slower, so it dominates B outright and the frontier halves.
    ///
    /// That is the whole point of the tolerance: a binary twice the size
    /// should not survive on a runtime advantage the machine invented. So the
    /// invariant is not monotonicity. It is that the frontier stays *valid*
    /// at whatever tolerance it was computed with.
    #[test]
    fn the_frontier_is_valid_at_whatever_tolerance_it_was_computed_with(
        points in some_points(),
        noise in 0.0f64..0.5,
    ) {
        let on = frontier_within(&points, noise);
        for (index, point) in points.iter().enumerate() {
            let dominated = points.iter().enumerate().any(|(other, candidate)| {
                other != index && candidate.dominates_within(point, noise)
            });
            prop_assert_eq!(on.contains(&index), point.eligible && !dominated);
        }
    }

    /// There is always something on the frontier when there is anything to put
    /// there.
    ///
    /// An empty frontier over eligible points would mean every one of them was
    /// dominated, which cannot happen if dominance is a strict partial order —
    /// so this is the order property, observed from outside.
    #[test]
    fn a_sweep_with_any_usable_result_always_has_a_frontier(
        points in some_points(),
        noise in 0.0f64..0.5,
    ) {
        let eligible = points.iter().filter(|point| point.eligible).count();
        let on = frontier_within(&points, noise).len();
        prop_assert!(
            eligible == 0 || on > 0,
            "{} eligible configurations produced an empty frontier", eligible
        );
    }

    /// The single smallest eligible configuration is always shown.
    ///
    /// Dominance requires being no larger, so nothing can dominate the unique
    /// smallest. If this ever failed, a sweep could measure the smallest
    /// binary and not report it — which is the one result the product exists
    /// to deliver.
    #[test]
    fn the_uniquely_smallest_configuration_is_always_on_the_frontier(
        points in some_points(),
        noise in 0.0f64..0.5,
    ) {
        let eligible: Vec<(usize, &Point)> =
            points.iter().enumerate().filter(|(_, point)| point.eligible).collect();
        let Some(&(smallest, best)) = eligible
            .iter()
            .min_by_key(|(_, point)| point.size_bytes)
        else {
            return Ok(());
        };
        // Only when it is the unique minimum; ties are a different question.
        let unique =
            eligible.iter().filter(|(_, point)| point.size_bytes == best.size_bytes).count() == 1;
        if unique {
            prop_assert!(
                frontier_within(&points, noise).contains(&smallest),
                "the smallest eligible configuration was not reported"
            );
        }
    }

    /// The median is within the samples' range — never outside it.
    #[test]
    fn the_median_lies_within_the_samples(
        values in prop::collection::vec(1u64..1_000_000, 1..40)
    ) {
        let samples = Samples::new(values.iter().map(|&v| Duration::from_nanos(v)));
        let median = samples.median().expect("non-empty");
        let low = Duration::from_nanos(*values.iter().min().unwrap());
        let high = Duration::from_nanos(*values.iter().max().unwrap());
        prop_assert!(median >= low && median <= high);
    }

    /// Comparing a sample set against itself is never a win or a loss.
    ///
    /// §6's rule at its limit: if identical measurements could produce a
    /// verdict, every verdict would be suspect.
    #[test]
    fn identical_samples_never_produce_a_verdict(
        values in prop::collection::vec(1_000u64..10_000_000, 3..30),
        floor in 0.0f64..0.2
    ) {
        let samples = Samples::new(values.iter().map(|&v| Duration::from_nanos(v)));
        let noise = NoiseFloor { relative: floor, samples: samples.clone() };
        let verdict = binmap_measure::timing::compare(&samples, &samples, &noise, 0.05);
        prop_assert!(
            matches!(
                verdict,
                BenchmarkVerdict::Inconclusive { .. }
                    | BenchmarkVerdict::NoSignificantChange { .. }
            ),
            "identical samples produced {:?}", verdict
        );
    }

    /// Swapping the two sides swaps the verdict. An asymmetric comparison
    /// would mean the answer depended on argument order.
    #[test]
    fn the_comparison_is_symmetric_under_swapping(
        left in prop::collection::vec(1_000_000u64..2_000_000, 5..15),
        right in prop::collection::vec(1_000_000u64..2_000_000, 5..15),
    ) {
        let a = Samples::new(left.iter().map(|&v| Duration::from_nanos(v)));
        let b = Samples::new(right.iter().map(|&v| Duration::from_nanos(v)));
        let noise = NoiseFloor { relative: 0.01, samples: a.clone() };

        let forward = binmap_measure::timing::compare(&a, &b, &noise, 0.05);
        let backward = binmap_measure::timing::compare(&b, &a, &noise, 0.05);

        let flipped = matches!(
            (&forward, &backward),
            (BenchmarkVerdict::Improved { .. }, BenchmarkVerdict::Regressed { .. })
                | (BenchmarkVerdict::Regressed { .. }, BenchmarkVerdict::Improved { .. })
                | (BenchmarkVerdict::Inconclusive { .. }, BenchmarkVerdict::Inconclusive { .. })
                | (
                    BenchmarkVerdict::NoSignificantChange { .. },
                    BenchmarkVerdict::NoSignificantChange { .. }
                )
        );
        prop_assert!(flipped, "forward {:?} but backward {:?}", forward, backward);
    }

    /// A difference inside the floor is inconclusive whatever its size or
    /// direction. §6's rule over the whole input space rather than one case.
    #[test]
    fn a_difference_inside_the_floor_is_always_inconclusive(
        base in 10_000_000u64..100_000_000,
        drift in -50i64..50,
    ) {
        let shifted = (base as i64 + base as i64 * drift / 1000).max(1) as u64;
        let a = Samples::new((0..9).map(|i| Duration::from_nanos(base + i)));
        let b = Samples::new((0..9).map(|i| Duration::from_nanos(shifted + i)));
        // A floor comfortably wider than the drift.
        let noise = NoiseFloor { relative: 0.10, samples: a.clone() };

        let verdict = binmap_measure::timing::compare(&a, &b, &noise, 0.05);
        prop_assert!(
            matches!(verdict, BenchmarkVerdict::Inconclusive { .. }),
            "a {} per mille difference on a 10% machine produced {:?}", drift, verdict
        );
    }
}
