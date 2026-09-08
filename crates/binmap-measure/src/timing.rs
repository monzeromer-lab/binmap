//! Timing, the noise floor, and the question "is this difference real".
//!
//! Timing a build twice on a laptop produces two different numbers. The
//! difference between a measurement tool and a random number generator is
//! knowing how much of that is the machine, so the machine is measured first
//! and every comparison is made against that measurement.

use binmap_verify::BenchmarkVerdict;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// The times one workload took, kept individually.
///
/// Samples, never a mean: a mean throws away the shape of the distribution,
/// and the shape is what says whether a difference is real.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Samples {
    pub durations: Vec<Duration>,
}

impl Samples {
    pub fn new(durations: impl IntoIterator<Item = Duration>) -> Self {
        Self { durations: durations.into_iter().collect() }
    }

    pub fn len(&self) -> usize {
        self.durations.len()
    }

    pub fn is_empty(&self) -> bool {
        self.durations.is_empty()
    }

    fn sorted_nanos(&self) -> Vec<f64> {
        let mut values: Vec<f64> = self.durations.iter().map(|d| d.as_nanos() as f64).collect();
        values.sort_by(|a, b| a.partial_cmp(b).expect("durations are never NaN"));
        values
    }

    /// The median, which is what the interface shows. Robust to the one slow
    /// run every benchmark suite produces.
    pub fn median(&self) -> Option<Duration> {
        let values = self.sorted_nanos();
        if values.is_empty() {
            return None;
        }
        let middle = values.len() / 2;
        let nanos = if values.len().is_multiple_of(2) {
            (values[middle - 1] + values[middle]) / 2.0
        } else {
            values[middle]
        };
        Some(Duration::from_nanos(nanos as u64))
    }

    pub fn min(&self) -> Option<Duration> {
        self.durations.iter().copied().min()
    }

    pub fn mean_nanos(&self) -> Option<f64> {
        if self.durations.is_empty() {
            return None;
        }
        let total: f64 = self.durations.iter().map(|d| d.as_nanos() as f64).sum();
        Some(total / self.durations.len() as f64)
    }

    /// The interquartile range as a fraction of the median. The spread the
    /// noise floor is expressed in.
    pub fn relative_spread(&self) -> Option<f64> {
        let values = self.sorted_nanos();
        if values.len() < 4 {
            // Too few to speak of quartiles; fall back to the full range,
            // which is the pessimistic answer and the right one here.
            let (first, last) = (values.first()?, values.last()?);
            let median = self.median()?.as_nanos() as f64;
            if median == 0.0 {
                return Some(0.0);
            }
            return Some((last - first) / median);
        }
        let quartile = |fraction: f64| -> f64 {
            let position = fraction * (values.len() - 1) as f64;
            let lower = position.floor() as usize;
            let upper = position.ceil() as usize;
            if lower == upper {
                values[lower]
            } else {
                values[lower] + (position - lower as f64) * (values[upper] - values[lower])
            }
        };
        let median = self.median()?.as_nanos() as f64;
        if median == 0.0 {
            return Some(0.0);
        }
        Some((quartile(0.75) - quartile(0.25)) / median)
    }
}

/// How much of a difference this machine invents on its own.
///
/// Measured once per machine by timing one unchanged binary repeatedly. It is
/// shown beside every timing number rather than kept as an internal threshold,
/// because a user comparing two numbers deserves to know how far apart they
/// have to be before the comparison means anything.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoiseFloor {
    /// The relative spread of the unchanged binary's samples, as a fraction.
    /// `0.03` is a machine that varies by three per cent for no reason.
    pub relative: f64,
    pub samples: Samples,
}

impl NoiseFloor {
    /// Derive a floor from repeated runs of one unchanged binary.
    pub fn from_samples(samples: Samples) -> Option<Self> {
        let relative = samples.relative_spread()?;
        Some(Self { relative, samples })
    }

    /// The floor as the interface states it: "3.0%".
    pub fn describe(&self) -> String {
        format!("{:.1}%", self.relative * 100.0)
    }

    /// Whether a relative difference is small enough to be this machine
    /// talking to itself.
    pub fn covers(&self, relative_difference: f64) -> bool {
        relative_difference.abs() <= self.relative
    }
}

/// Compare a candidate against a baseline, given how noisy the machine is.
///
/// Two questions have to be answered the same way every time, and this is the
/// only place they are answered:
///
/// 1. Is the difference bigger than this machine's noise? If not, the answer is
///    `Inconclusive` — not "no change", which would be a claim we cannot make.
/// 2. Is it statistically significant? Decided by a Mann-Whitney U test, which
///    makes no assumption that timings are normally distributed, because they
///    are not.
pub fn compare(baseline: &Samples, candidate: &Samples, floor: &NoiseFloor) -> BenchmarkVerdict {
    let (Some(baseline_median), Some(candidate_median)) = (baseline.median(), candidate.median())
    else {
        return BenchmarkVerdict::Inconclusive {
            detail: "the benchmark produced no samples".into(),
        };
    };

    if baseline.len() < 3 || candidate.len() < 3 {
        return BenchmarkVerdict::Inconclusive {
            detail: format!(
                "{} baseline and {} candidate samples are too few to compare",
                baseline.len(),
                candidate.len()
            ),
        };
    }

    let baseline_nanos = baseline_median.as_nanos() as f64;
    let candidate_nanos = candidate_median.as_nanos() as f64;
    let relative = if baseline_nanos == 0.0 {
        0.0
    } else {
        (candidate_nanos - baseline_nanos) / baseline_nanos
    };
    let percent = relative * 100.0;

    if floor.covers(relative) {
        return BenchmarkVerdict::Inconclusive {
            detail: format!(
                "the {:.1}% difference is inside this machine's {} noise floor",
                percent.abs(),
                floor.describe()
            ),
        };
    }

    let significance = mann_whitney(&baseline.sorted_nanos(), &candidate.sorted_nanos());
    if significance.p_value > 0.05 {
        return BenchmarkVerdict::Inconclusive {
            detail: format!(
                "{:.1}% {}, but the samples overlap too much to call it (p = {:.2}, {} runs)",
                percent.abs(),
                if relative < 0.0 { "faster" } else { "slower" },
                significance.p_value,
                baseline.len() + candidate.len()
            ),
        };
    }

    if relative < 0.0 {
        BenchmarkVerdict::Improved {
            detail: format!(
                "{:.1}% faster, outside the {} noise floor (p = {:.3})",
                percent.abs(),
                floor.describe(),
                significance.p_value
            ),
        }
    } else {
        BenchmarkVerdict::Regressed {
            detail: format!(
                "{percent:.1}% slower, outside the {} noise floor (p = {:.3})",
                floor.describe(),
                significance.p_value
            ),
        }
    }
}

struct Significance {
    p_value: f64,
}

/// Mann-Whitney U with a tie-corrected normal approximation.
///
/// Chosen over a t-test because timing distributions are skewed and heavy in
/// the right tail — a scheduler hiccup is not a normal deviate — and a test
/// that assumes otherwise reports significance that is not there.
fn mann_whitney(first: &[f64], second: &[f64]) -> Significance {
    let (n1, n2) = (first.len() as f64, second.len() as f64);
    if n1 == 0.0 || n2 == 0.0 {
        return Significance { p_value: 1.0 };
    }

    let mut combined: Vec<(f64, usize)> = first
        .iter()
        .map(|&v| (v, 0usize))
        .chain(second.iter().map(|&v| (v, 1usize)))
        .collect();
    combined.sort_by(|a, b| a.0.partial_cmp(&b.0).expect("durations are never NaN"));

    // Average ranks over ties, and accumulate the tie correction as we go.
    let mut ranks = vec![0.0; combined.len()];
    let mut tie_correction = 0.0;
    let mut index = 0;
    while index < combined.len() {
        let mut end = index + 1;
        while end < combined.len() && combined[end].0 == combined[index].0 {
            end += 1;
        }
        let run = (end - index) as f64;
        let average = (index + end + 1) as f64 / 2.0;
        for rank in ranks.iter_mut().take(end).skip(index) {
            *rank = average;
        }
        tie_correction += run * run * run - run;
        index = end;
    }

    let rank_sum_first: f64 = combined
        .iter()
        .zip(&ranks)
        .filter(|((_, group), _)| *group == 0)
        .map(|(_, rank)| rank)
        .sum();

    let u1 = rank_sum_first - n1 * (n1 + 1.0) / 2.0;
    let u = u1.min(n1 * n2 - u1);

    let n = n1 + n2;
    let mean = n1 * n2 / 2.0;
    let variance = (n1 * n2 / 12.0) * ((n + 1.0) - tie_correction / (n * (n - 1.0)));
    if variance <= 0.0 {
        return Significance { p_value: 1.0 };
    }

    // Continuity correction, then two-tailed.
    let z = ((u - mean).abs() - 0.5) / variance.sqrt();
    Significance { p_value: (2.0 * (1.0 - standard_normal_cdf(z))).clamp(0.0, 1.0) }
}

/// The standard normal CDF, via the Abramowitz and Stegun 7.1.26 error
/// function. Accurate to about 1.5e-7, which is far beyond what a p-value
/// rounded to three places needs.
fn standard_normal_cdf(z: f64) -> f64 {
    0.5 * (1.0 + erf(z / std::f64::consts::SQRT_2))
}

fn erf(x: f64) -> f64 {
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();
    let t = 1.0 / (1.0 + 0.3275911 * x);
    let y = 1.0
        - (((((1.061405429 * t - 1.453152027) * t) + 1.421413741) * t - 0.284496736) * t
            + 0.254829592)
            * t
            * (-x * x).exp();
    sign * y
}

#[cfg(test)]
mod tests {
    use super::*;

    fn millis(values: &[u64]) -> Samples {
        Samples::new(values.iter().map(|&ms| Duration::from_millis(ms)))
    }

    fn floor(relative: f64) -> NoiseFloor {
        NoiseFloor { relative, samples: millis(&[100, 101, 102, 103]) }
    }

    #[test]
    fn the_median_ignores_the_one_slow_run_every_suite_produces() {
        let samples = millis(&[100, 101, 102, 103, 900]);
        assert_eq!(samples.median(), Some(Duration::from_millis(102)));
        // The mean does not, which is why the median is what we show.
        assert!(samples.mean_nanos().unwrap() > 200e6);
    }

    #[test]
    fn a_difference_inside_the_noise_floor_is_inconclusive_not_no_change() {
        let baseline = millis(&[100, 101, 102, 103, 104, 105]);
        let candidate = millis(&[99, 100, 101, 102, 103, 104]);
        let verdict = compare(&baseline, &candidate, &floor(0.05));
        match verdict {
            BenchmarkVerdict::Inconclusive { detail } => {
                assert!(detail.contains("noise floor"), "{detail}");
            }
            other => panic!("a 1% difference on a 5% machine should be inconclusive: {other:?}"),
        }
    }

    #[test]
    fn a_large_consistent_win_is_reported_as_one() {
        let baseline = millis(&[200, 201, 202, 203, 204, 205, 206]);
        let candidate = millis(&[100, 101, 102, 103, 104, 105, 106]);
        let verdict = compare(&baseline, &candidate, &floor(0.03));
        assert!(matches!(verdict, BenchmarkVerdict::Improved { .. }), "{verdict:?}");
    }

    #[test]
    fn a_large_consistent_loss_is_reported_as_one() {
        let baseline = millis(&[100, 101, 102, 103, 104, 105, 106]);
        let candidate = millis(&[200, 201, 202, 203, 204, 205, 206]);
        let verdict = compare(&baseline, &candidate, &floor(0.03));
        match verdict {
            BenchmarkVerdict::Regressed { detail } => assert!(detail.contains("slower"), "{detail}"),
            other => panic!("expected a regression: {other:?}"),
        }
    }

    #[test]
    fn a_big_median_shift_with_overlapping_samples_is_not_claimed() {
        // The medians differ by well over the noise floor, but the two sets
        // interleave: the honest answer is that we cannot tell.
        let baseline = millis(&[100, 140, 180, 220, 260]);
        let candidate = millis(&[110, 150, 190, 230, 270]);
        let verdict = compare(&baseline, &candidate, &floor(0.01));
        assert!(matches!(verdict, BenchmarkVerdict::Inconclusive { .. }), "{verdict:?}");
    }

    #[test]
    fn too_few_samples_is_inconclusive_rather_than_a_coin_flip() {
        let verdict = compare(&millis(&[100, 200]), &millis(&[10, 20]), &floor(0.01));
        match verdict {
            BenchmarkVerdict::Inconclusive { detail } => assert!(detail.contains("too few")),
            other => panic!("expected inconclusive: {other:?}"),
        }
    }

    #[test]
    fn identical_samples_are_never_significant() {
        let samples = millis(&[100, 100, 100, 100, 100, 100]);
        let verdict = compare(&samples, &samples, &floor(0.0));
        assert!(matches!(verdict, BenchmarkVerdict::Inconclusive { .. }), "{verdict:?}");
    }

    #[test]
    fn the_noise_floor_states_itself_in_the_interface_s_words() {
        let floor = NoiseFloor::from_samples(millis(&[100, 101, 102, 103, 104])).unwrap();
        assert!(floor.describe().ends_with('%'));
        assert!(floor.covers(0.001));
    }
}
