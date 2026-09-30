//! Samples, and where they came from (`F3.1`, `F3.2`, `TOOLING §4.3`).
//!
//! A profile is a bag of stacks with counts. What makes attributing one hard
//! is not the counting — it is that **every number in a profile is a
//! statistic**, and a tool that renders 3 samples and 3000 samples with the
//! same confidence is lying about one of them.
//!
//! So a sample count is never shown without the total it is a fraction of, and
//! a function whose count is within sampling noise is reported as
//! indistinguishable rather than ranked. That is the same rule the sweep
//! applies to its noise floor, for the same reason: a difference smaller than
//! the measurement's own error is not a difference.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Where the samples came from.
///
/// Recorded because the two sources have genuinely different properties, and
/// a reader comparing profiles needs to know they are comparable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Source {
    /// `perf record`, parsed from `perf script`.
    Perf,
    /// samply, via the Firefox Profiler format.
    Samply,
    /// A profile we produced ourselves.
    Internal,
}

impl Source {
    pub fn label(self) -> &'static str {
        match self {
            Source::Perf => "perf",
            Source::Samply => "samply",
            Source::Internal => "binmap",
        }
    }
}

/// One stack, as sampled.
///
/// Addresses, innermost first, exactly as the unwinder produced them. Nothing
/// is symbolized at this stage: a million samples resolved individually is the
/// classic way to make a profiler take longer than the program it profiled.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Stack {
    pub addresses: Vec<u64>,
}

/// A whole profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    /// Stacks and how often each was seen. Identical stacks collapse, which is
    /// what makes a profile of a million samples a few thousand entries.
    pub stacks: BTreeMap<Stack, u64>,
    pub source: Source,
    /// The command that produced it, for the evidence record.
    pub command: String,
}

impl Profile {
    pub fn new(source: Source, command: impl Into<String>) -> Self {
        Self { stacks: BTreeMap::new(), source, command: command.into() }
    }

    pub fn record(&mut self, stack: Stack, count: u64) {
        *self.stacks.entry(stack).or_insert(0) += count;
    }

    /// Every sample, counted.
    pub fn total(&self) -> u64 {
        self.stacks.values().sum()
    }

    pub fn is_empty(&self) -> bool {
        self.total() == 0
    }

    /// How many samples land *in* each address, as opposed to through it.
    ///
    /// "Self" time: the innermost frame of each stack. This is where the
    /// program actually was.
    pub fn self_samples(&self) -> BTreeMap<u64, u64> {
        let mut counts = BTreeMap::new();
        for (stack, count) in &self.stacks {
            if let Some(innermost) = stack.addresses.first() {
                *counts.entry(*innermost).or_insert(0) += count;
            }
        }
        counts
    }

    /// How many samples pass *through* each address.
    ///
    /// "Total" time. An address appearing twice in one stack — recursion — is
    /// counted once for that stack, because the program was not there twice at
    /// the same instant and counting it twice makes a recursive function look
    /// like it dominates.
    pub fn total_samples(&self) -> BTreeMap<u64, u64> {
        let mut counts = BTreeMap::new();
        for (stack, count) in &self.stacks {
            let unique: std::collections::BTreeSet<u64> = stack.addresses.iter().copied().collect();
            for address in unique {
                *counts.entry(address).or_insert(0) += count;
            }
        }
        counts
    }

    /// The sampling error on a count, as a fraction of the total.
    ///
    /// Sampling is a binomial process, so the standard error on a count of `k`
    /// out of `n` is `sqrt(k(n-k)/n)`. Two counts closer together than this
    /// are not distinguishable, and ranking them as though they were is the
    /// profiler equivalent of reporting a size difference inside the noise
    /// floor.
    pub fn standard_error(&self, count: u64) -> f64 {
        let total = self.total();
        if total == 0 || count > total {
            return 0.0;
        }
        let k = count as f64;
        let n = total as f64;
        (k * (n - k) / n).sqrt()
    }

    /// Whether two counts are far enough apart to rank.
    ///
    /// Two standard errors, which is the same ~95% convention the sweep's
    /// significance test uses.
    pub fn distinguishable(&self, left: u64, right: u64) -> bool {
        let difference = left.abs_diff(right) as f64;
        let error = self.standard_error(left).max(self.standard_error(right));
        difference > 2.0 * error
    }

    /// How few samples is too few to say anything.
    ///
    /// Below this the profile is reported as inconclusive rather than ranked.
    /// A hundred samples spread over a real program gives every function a
    /// count of one or two, and ranking those is ranking noise.
    pub const TOO_FEW: u64 = 500;

    pub fn has_enough_samples(&self) -> bool {
        self.total() >= Self::TOO_FEW
    }

    /// The sentence shown above any ranking.
    pub fn describe(&self) -> String {
        let total = self.total();
        let mut line = format!(
            "{total} samples in {} distinct stacks, from {}",
            self.stacks.len(),
            self.source.label()
        );
        if !self.has_enough_samples() {
            line.push_str(&format!(
                ". That is fewer than {} — too few to rank functions against each other, so \
                 what follows is indicative rather than a measurement.",
                Self::TOO_FEW
            ));
        }
        line
    }
}
