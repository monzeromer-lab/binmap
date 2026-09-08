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
    ///
    /// A point that did not measure an objective the other measured cannot
    /// dominate it. Claiming otherwise would let an unmeasured axis pass for a
    /// free win, which is exactly the mistake a tool like this must not make.
    pub fn dominates(&self, other: &Point) -> bool {
        if !self.eligible || !other.eligible {
            return false;
        }

        let mut strictly_better_somewhere = false;
        let mut compare = |mine: u64, theirs: u64| -> bool {
            if mine > theirs {
                return false;
            }
            if mine < theirs {
                strictly_better_somewhere = true;
            }
            true
        };

        if !compare(self.size_bytes, other.size_bytes) {
            return false;
        }
        for (mine, theirs) in [
            (self.runtime_nanos, other.runtime_nanos),
            (self.build_time_nanos, other.build_time_nanos),
        ] {
            match (mine, theirs) {
                (Some(mine), Some(theirs)) => {
                    if !compare(mine, theirs) {
                        return false;
                    }
                }
                // They measured this axis and we did not: our standing on it is
                // unknown, so we cannot claim to be at least as good on it.
                (None, Some(_)) => return false,
                // We measured it and they did not; the axis is simply not part
                // of the comparison.
                (Some(_), None) | (None, None) => {}
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
    (0..points.len())
        .filter(|&index| {
            points[index].eligible
                && !points.iter().enumerate().any(|(other, candidate)| {
                    other != index && candidate.dominates(&points[index])
                })
        })
        .collect()
}

/// The frontier, ordered smallest-first — the order the Profile Lab's table
/// reads in, since size is the objective Phase 0 exists to serve.
pub fn frontier_by_size(points: &[Point]) -> Vec<usize> {
    let mut indices = frontier(points);
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
        let points =
            vec![point("small-and-fast", 100, 100), point("big-and-slow", 200, 200)];
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
        // The unmeasured point ties on size and build time. If the missing
        // runtime counted as zero it would dominate; it does not.
        assert!(!unmeasured.dominates(&measured));
        // And the measured point does not dominate on an axis the other lacks.
        assert!(!measured.dominates(&unmeasured));
    }

    #[test]
    fn a_candidate_that_failed_its_gates_stays_visible_but_off_the_frontier() {
        let points = vec![
            point("tiny-but-broken", 1, 1).ineligible(),
            point("honest", 100, 100),
        ];
        assert_eq!(frontier(&points), vec![1]);
        // It is still in the table — the caller keeps every point it measured.
        assert_eq!(points.len(), 2);
    }

    #[test]
    fn the_frontier_reads_smallest_first() {
        let points = vec![point("b", 300, 100), point("a", 100, 300), point("c", 200, 200)];
        let ordered: Vec<&str> =
            frontier_by_size(&points).into_iter().map(|i| points[i].configuration.as_str()).collect();
        assert_eq!(ordered, ["a", "c", "b"]);
    }

    #[test]
    fn runtime_participates_when_both_points_measured_it() {
        let points = vec![
            point("a", 100, 100).with_runtime(100),
            point("b", 100, 100).with_runtime(200),
        ];
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
