//! Attributing samples to source (`F3.2`, `F3.3`).
//!
//! Through `binmap_crash::symbolize`, deliberately: `F3.2` says "the same
//! symbolization layer the crash analyzer uses", and the reason is the inline
//! handling. An optimised binary inlines aggressively, so one sampled address
//! belongs to several functions at once — and a profiler that attributes all
//! of a hot address to the outermost function reports that
//! `core::iter::Iterator::fold` is 40% of your program, which is true and
//! useless.
//!
//! `F3.3` asks for a flamegraph "showing logical call structure, with inline
//! frames visually distinct". The logical structure is the one with inline
//! frames expanded; the physical one is what the machine did. Both are real,
//! and conflating them is how a profiler becomes unactionable.

use crate::sample::Profile;
use binmap_crash::symbolize::Resolved;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One function's share of a profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hot {
    pub function: String,
    pub file: Option<String>,
    pub line: Option<u32>,
    /// Samples where this was the innermost frame — where the program was.
    pub self_samples: u64,
    /// Samples anywhere in the stack — where the program was, or was on its
    /// way to being.
    pub total_samples: u64,
    /// Whether this frame was inlined into its caller.
    ///
    /// `F3.3`: visually distinct, because an inlined function has no call
    /// overhead and "make this not a function call" is advice that cannot help.
    pub inlined: bool,
}

impl Hot {
    pub fn self_share(&self, total: u64) -> f64 {
        if total == 0 {
            return 0.0;
        }
        self.self_samples as f64 / total as f64
    }

    pub fn describe(&self, total: u64) -> String {
        let location = match (&self.file, self.line) {
            (Some(file), Some(line)) => {
                let short = file.rsplit('/').next().unwrap_or(file);
                format!(" at {short}:{line}")
            }
            _ => String::new(),
        };
        format!(
            "{}{location} — {:.1}% self, {} samples{}",
            self.function,
            self.self_share(total) * 100.0,
            self.self_samples,
            if self.inlined { " (inlined)" } else { "" }
        )
    }
}

/// A profile, attributed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Attributed {
    /// Hottest by self time first, which is the ranking that answers "where is
    /// the time going".
    pub hot: Vec<Hot>,
    pub total_samples: u64,
    /// Samples whose address resolved to nothing.
    ///
    /// Reported rather than dropped: a profile that silently discards a third
    /// of its samples shows percentages that add up to a hundred and describe
    /// two thirds of the program.
    pub unresolved_samples: u64,
}

impl Attributed {
    pub fn coverage(&self) -> f64 {
        if self.total_samples == 0 {
            return 0.0;
        }
        (self.total_samples - self.unresolved_samples) as f64 / self.total_samples as f64
    }

    /// The functions that are the reader's own.
    pub fn yours(&self, own: &[String]) -> Vec<&Hot> {
        self.hot
            .iter()
            .filter(|hot| own.iter().any(|name| hot.function.starts_with(&format!("{name}::"))))
            .collect()
    }

    pub fn describe(&self) -> String {
        format!(
            "{} functions across {} samples; {:.0}% of samples resolved to a function",
            self.hot.len(),
            self.total_samples,
            self.coverage() * 100.0
        )
    }
}

/// Attribute a profile.
///
/// `resolve` is how an address becomes locations — injected so this is
/// testable without a binary, and so a caller that already has a symbolizer
/// per module does not build a second one.
pub fn attribute<F>(profile: &Profile, mut resolve: F) -> Attributed
where
    F: FnMut(u64) -> Resolved,
{
    // Resolved once per *distinct* address, not once per sample. A million
    // samples resolved individually is the classic way to make a profiler
    // slower than the program it profiled.
    let mut cache: BTreeMap<u64, Resolved> = BTreeMap::new();
    for stack in profile.stacks.keys() {
        for address in &stack.addresses {
            cache.entry(*address).or_insert_with(|| resolve(*address));
        }
    }

    // A key that is the *function*, not the address: one function occupies
    // many addresses, and a ranking by address is a ranking of basic blocks.
    let mut self_counts: BTreeMap<String, (u64, Option<String>, Option<u32>, bool)> =
        BTreeMap::new();
    let mut total_counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut unresolved = 0u64;

    for (stack, count) in &profile.stacks {
        // Self time: the innermost *location*, which after inlining is the
        // innermost inline frame rather than the innermost physical one.
        match stack.addresses.first().and_then(|address| cache.get(address)) {
            Some(resolved) if !resolved.is_empty() => {
                let innermost = &resolved.locations[0];
                let name = innermost.function.clone().unwrap_or_else(|| "<unknown>".to_string());
                let entry = self_counts.entry(name).or_insert((
                    0,
                    innermost.file.clone(),
                    innermost.line,
                    innermost.inlined,
                ));
                entry.0 += count;
            }
            _ => unresolved += count,
        }

        // Total time: every function on the stack, counted once per stack.
        // Recursion appearing twice in one stack does not mean the program was
        // there twice at the same instant.
        let mut seen = std::collections::BTreeSet::new();
        for address in &stack.addresses {
            let Some(resolved) = cache.get(address) else { continue };
            for location in &resolved.locations {
                let Some(name) = &location.function else { continue };
                if seen.insert(name.clone()) {
                    *total_counts.entry(name.clone()).or_insert(0) += count;
                }
            }
        }
    }

    let mut hot: Vec<Hot> = self_counts
        .into_iter()
        .map(|(function, (self_samples, file, line, inlined))| Hot {
            total_samples: total_counts.get(&function).copied().unwrap_or(self_samples),
            function,
            file,
            line,
            self_samples,
            inlined,
        })
        .collect();

    // Functions that only ever appear as callers still belong in the list: a
    // function with no self time and all the total time is where the work is
    // organised, which is frequently what a reader is looking for.
    for (function, total) in total_counts {
        if !hot.iter().any(|entry| entry.function == function) {
            hot.push(Hot {
                function,
                file: None,
                line: None,
                self_samples: 0,
                total_samples: total,
                inlined: false,
            });
        }
    }

    hot.sort_by(|left, right| {
        right
            .self_samples
            .cmp(&left.self_samples)
            .then(right.total_samples.cmp(&left.total_samples))
            .then(left.function.cmp(&right.function))
    });

    Attributed { hot, total_samples: profile.total(), unresolved_samples: unresolved }
}
