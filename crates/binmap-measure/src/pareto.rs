//! The Pareto frontier over size, runtime and build time (`F0.6`).
//!
//! Derived, never flagged: no configuration is marked as interesting by the
//! thing that produced it. A point is on the frontier because nothing else
//! measured beat it on every objective at once, and that is a property of the
//! measurements rather than an opinion about them.

use serde::{Deserialize, Serialize};

/// One measured configuration, as a point in the objective space.
///
/// Every objective is minimized, and every optional one is `None` when it was
/// not measured rather than zero. `runtime_nanos` is absent when no benchmark
/// was declared; `build_time_nanos` is absent when the sweep ran builds
/// concurrently, which makes each build's wall-clock time a measurement of the
/// machine's load rather than of the configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Point {
    /// The configuration's stable name.
    pub configuration: String,
    pub size_bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_nanos: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_time_nanos: Option<u64>,
    /// A candidate that did not build or did not pass its tests is not
    /// eligible for the frontier, but it stays in the table — a near miss is
    /// informative.
    pub eligible: bool,
}

impl Point {
    pub fn new(configuration: impl Into<String>, size_bytes: u64) -> Self {
        Self {
            configuration: configuration.into(),
            size_bytes,
            runtime_nanos: None,
            build_time_nanos: None,
            eligible: true,
        }
    }

    pub fn with_runtime(mut self, runtime_nanos: u64) -> Self {
        self.runtime_nanos = Some(runtime_nanos);
        self
    }

    pub fn with_build_time(mut self, build_time_nanos: u64) -> Self {
        self.build_time_nanos = Some(build_time_nanos);
        self
    }

    pub fn ineligible(mut self) -> Self {
        self.eligible = false;
        self
    }

    /// Whether this point is at least as good as `other` everywhere and better
    /// somewhere — the definition of dominance, and the only judgement this
    /// module makes.
    pub fn dominates(&self, other: &Point) -> bool {
        self.dominates_within(other, 0.0)
    }

    /// Dominance, treating runtime differences within `noise` — a fraction, so
    /// `0.03` is a machine that varies by three per cent — as no difference.
    ///
    /// Two rules here, both of which were wrong before an audit.
    ///
    /// **An axis only one point measured is not part of the comparison, for
    /// either of them.** The rule used to be that a point missing an axis
    /// could not dominate one that had it. That sounds conservative and is
    /// not: it made dominance depend on which point was asked, so A could
    /// dominate B while B could never dominate A on the same data. If B did
    /// not measure runtime, neither point's standing on runtime is known and
    /// neither may claim it.
    ///
    /// **A runtime difference smaller than the noise floor is not a
    /// difference.** Comparing medians with a strict `<` let a one-nanosecond
    /// gap decide frontier membership on a machine that varies by three per
    /// cent — §6's rule broken in the one place it changes what is shown.
    pub fn dominates_within(&self, other: &Point, noise: f64) -> bool {
        if !self.eligible || !other.eligible {
            return false;
        }

        let mut strictly_better_somewhere = false;

        // Bytes are bytes, and a build time recorded at all was recorded
        // serially; both are exact.
        let mut exact = |mine: u64, theirs: u64| -> bool {
            if mine > theirs {
                return false;
            }
            if mine < theirs {
                strictly_better_somewhere = true;
            }
            true
        };

        if !exact(self.size_bytes, other.size_bytes) {
            return false;
        }
        if let (Some(mine), Some(theirs)) = (self.build_time_nanos, other.build_time_nanos)
            && !exact(mine, theirs)
        {
            return false;
        }

        // Runtime is measured, so it carries the machine's noise with it.
        if let (Some(mine), Some(theirs)) = (self.runtime_nanos, other.runtime_nanos) {
            let larger = mine.max(theirs) as f64;
            let apart =
                if larger == 0.0 { 0.0 } else { (mine as f64 - theirs as f64).abs() / larger };
            if apart > noise {
                if mine > theirs {
                    return false;
                }
                strictly_better_somewhere = true;
            }
        }

        strictly_better_somewhere
    }
}

/// The indices of the points nothing else dominates.
///
/// Returned as indices rather than clones so the caller keeps whatever it
/// attached to each point — the configuration, its findings, its evidence.
pub fn frontier(points: &[Point]) -> Vec<usize> {
    frontier_within(points, 0.0)
}

/// The frontier, treating runtime differences within the machine's measured
/// noise floor as no difference.
///
/// `noise` is a fraction — pass a
/// [`NoiseFloor`](crate::NoiseFloor)'s `relative`. Without it a difference the
/// machine invented decides which configuration the user is shown, which is
/// §6's rule broken in the one place it changes the answer.
pub fn frontier_within(points: &[Point], noise: f64) -> Vec<usize> {
    (0..points.len())
        .filter(|&index| {
            points[index].eligible
                && !points.iter().enumerate().any(|(other, candidate)| {
                    other != index && candidate.dominates_within(&points[index], noise)
                })
        })
        .collect()
}

/// The frontier, ordered smallest-first — the order the Profile Lab's table
/// reads in, since size is the objective Phase 0 exists to serve.
pub fn frontier_by_size(points: &[Point]) -> Vec<usize> {
    frontier_by_size_within(points, 0.0)
}

/// The frontier, noise-aware and smallest-first.
pub fn frontier_by_size_within(points: &[Point], noise: f64) -> Vec<usize> {
    let mut indices = frontier_within(points, noise);
    indices.sort_by_key(|&index| (points[index].size_bytes, points[index].build_time_nanos));
    indices
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(name: &str, size: u64, build: u64) -> Point {
        Point::new(name, size).with_build_time(build)
    }

    #[test]
    fn a_point_beaten_on_every_axis_is_off_the_frontier() {
        let points = vec![point("small-and-fast", 100, 100), point("big-and-slow", 200, 200)];
        assert_eq!(frontier(&points), vec![0]);
    }

    #[test]
    fn a_trade_off_keeps_both_points() {
        // Smaller but slower to build, against bigger but quicker to build.
        let points = vec![point("small", 100, 300), point("quick-build", 200, 100)];
        assert_eq!(frontier(&points), vec![0, 1]);
    }

    #[test]
    fn identical_points_do_not_dominate_each_other() {
        let points = vec![point("a", 100, 100), point("b", 100, 100)];
        // Neither is strictly better anywhere, so both stand.
        assert_eq!(frontier(&points), vec![0, 1]);
    }

    #[test]
    fn an_unmeasured_axis_is_never_a_free_win() {
        let measured = point("measured", 100, 100).with_runtime(500);
        let unmeasured = point("unmeasured", 100, 100);
        // Tied on size and build time. If the missing runtime counted as zero
        // it would dominate; it does not.
        assert!(!unmeasured.dominates(&measured));
        assert!(!measured.dominates(&unmeasured));
    }

    #[test]
    fn dominance_does_not_depend_on_which_point_is_asked() {
        // The old rule — "a point missing an axis cannot dominate one that has
        // it" — sounded conservative and made the relation asymmetric: A could
        // dominate B while B could never dominate A, on the same data.
        let with = Point::new("with", 100).with_runtime(500);
        let without = Point::new("without", 50);
        assert!(!with.dominates(&without), "the smaller point wins on the only shared axis");
        assert!(without.dominates(&with), "50 bytes beats 100 on the axis both measured");
        // And the frontier is the same set whichever order they arrive in.
        assert_eq!(frontier(&[with.clone(), without.clone()]), vec![1]);
        assert_eq!(frontier(&[without, with]), vec![0]);
    }

    #[test]
    fn a_runtime_difference_inside_the_noise_floor_does_not_decide_the_frontier() {
        // §6's rule, applied where it changes what the user is shown. On a 3%
        // machine, a 0.2% runtime gap is the machine talking to itself — it
        // must not push a smaller binary off the frontier.
        let small_and_a_hair_slower = Point::new("small", 100).with_runtime(1_002);
        let large_and_a_hair_faster = Point::new("large", 900).with_runtime(1_000);

        // Ignoring noise, the faster point wins its axis and both survive.
        assert_eq!(
            frontier(&[small_and_a_hair_slower.clone(), large_and_a_hair_faster.clone()]).len(),
            2
        );

        // Told the machine varies by 3%, the runtimes are the same number and
        // the smaller binary dominates outright.
        let noisy = frontier_within(&[small_and_a_hair_slower, large_and_a_hair_faster], 0.03);
        assert_eq!(noisy, vec![0], "a sub-noise difference kept a 9x larger binary alive");
    }

    #[test]
    fn a_runtime_difference_outside_the_noise_floor_still_counts() {
        let small_but_slow = Point::new("small", 100).with_runtime(2_000);
        let large_but_fast = Point::new("large", 900).with_runtime(1_000);
        assert_eq!(frontier_within(&[small_but_slow, large_but_fast], 0.03).len(), 2);
    }

    #[test]
    fn a_candidate_that_failed_its_gates_stays_visible_but_off_the_frontier() {
        let points = vec![point("tiny-but-broken", 1, 1).ineligible(), point("honest", 100, 100)];
        assert_eq!(frontier(&points), vec![1]);
        // It is still in the table — the caller keeps every point it measured.
        assert_eq!(points.len(), 2);
    }

    #[test]
    fn the_frontier_reads_smallest_first() {
        let points = vec![point("b", 300, 100), point("a", 100, 300), point("c", 200, 200)];
        let ordered: Vec<&str> = frontier_by_size(&points)
            .into_iter()
            .map(|i| points[i].configuration.as_str())
            .collect();
        assert_eq!(ordered, ["a", "c", "b"]);
    }

    #[test]
    fn runtime_participates_when_both_points_measured_it() {
        let points =
            vec![point("a", 100, 100).with_runtime(100), point("b", 100, 100).with_runtime(200)];
        assert_eq!(frontier(&points), vec![0]);
    }

    #[test]
    fn an_empty_sweep_has_an_empty_frontier() {
        assert!(frontier(&[]).is_empty());
    }

    #[test]
    fn build_times_dropped_by_a_parallel_sweep_do_not_become_free_wins() {
        // A sweep that built concurrently cannot compare wall-clock build
        // times, so it records none. Such a point must not then dominate one
        // that measured a build time.
        let timed = point("serial", 100, 900);
        let untimed = Point::new("parallel", 100);
        assert!(!untimed.dominates(&timed));
        assert!(!timed.dominates(&untimed));
        assert_eq!(frontier(&[timed, untimed]), vec![0, 1]);
    }
}
