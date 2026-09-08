//! Reference corpus, entry two: generics, and a suite that is configuration-
//! dependent on purpose.
//!
//! Two jobs.
//!
//! **Monomorphization has something to find.** `Pipeline` is instantiated at
//! several unrelated type arguments, so Phase 1's grouping has real
//! instantiations to collapse rather than a synthetic one.
//!
//! **The gates have something to reject.** `checksum` adds without wrapping.
//! Under `overflow-checks = false` — the release default — it wraps silently
//! and the test below passes. Under `overflow-checks = true` it panics and the
//! suite fails. So a sweep over this crate must reject every
//! `overflow-checks=true` configuration on `TestsPass`, and accept the rest.
//!
//! That is the end-to-end proof that the gates run under the configuration
//! they are judging. With a fixed gate command every configuration passes
//! identically and the tool cheerfully recommends one that breaks the suite.
//!
//! It is also a realistic bug rather than a contrived one: silent wrapping in
//! release and a panic in debug is exactly the latent defect a configuration
//! sweep ought to surface.
//!
//! `panic = "abort"` was the first thing tried here and it does not work:
//! cargo ignores the panic setting for test targets, because the harness needs
//! to unwind to catch a `#[should_panic]`. Worth knowing before designing a
//! test around it.

use std::collections::BTreeMap;
use std::fmt::Debug;

/// A small transform chain, generic over what it carries.
///
/// Deliberately monomorphized at several unrelated type arguments below, each
/// of which the compiler emits separately.
#[derive(Debug, Clone, Default)]
pub struct Pipeline<T> {
    stages: Vec<T>,
}

impl<T: Clone + Ord + Debug> Pipeline<T> {
    pub fn new() -> Self {
        Self { stages: Vec::new() }
    }

    pub fn push(&mut self, stage: T) -> &mut Self {
        self.stages.push(stage);
        self
    }

    pub fn len(&self) -> usize {
        self.stages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.stages.is_empty()
    }

    /// The stages in order, deduplicated.
    pub fn distinct(&self) -> Vec<T> {
        let mut seen: BTreeMap<T, ()> = BTreeMap::new();
        for stage in &self.stages {
            seen.insert(stage.clone(), ());
        }
        seen.into_keys().collect()
    }

    /// The stage at `index`.
    ///
    /// Panics when the index is out of range, which is what the
    /// configuration-dependent tests below exercise.
    pub fn stage(&self, index: usize) -> &T {
        assert!(
            index < self.stages.len(),
            "stage {index} of a {}-stage pipeline",
            self.stages.len()
        );
        &self.stages[index]
    }
}

/// A checksum that adds without wrapping.
///
/// Under `overflow-checks = false` this wraps silently, which is what the
/// release profile does by default and what the test below asserts. Under
/// `overflow-checks = true` it panics. The value of a configuration sweep is
/// that it tells you which of those you are shipping.
pub fn checksum(bytes: &[u8]) -> u8 {
    let mut total: u8 = 0;
    for byte in bytes {
        total = total + *byte;
    }
    total
}

/// Instantiate the generic several ways, so the artifact carries several
/// copies for size attribution to find.
pub fn exercise() -> usize {
    let mut integers: Pipeline<u32> = Pipeline::new();
    integers.push(3).push(1).push(3);

    let mut strings: Pipeline<String> = Pipeline::new();
    strings.push("parse".into()).push("verify".into());

    let mut pairs: Pipeline<(u8, char)> = Pipeline::new();
    pairs.push((1, 'a')).push((2, 'b'));

    let mut nested: Pipeline<Vec<i64>> = Pipeline::new();
    nested.push(vec![1, 2, 3]).push(vec![4]);

    integers.distinct().len() + strings.distinct().len() + pairs.len() + nested.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distinct_keeps_one_of_each_in_order() {
        let mut pipeline: Pipeline<u32> = Pipeline::new();
        pipeline.push(3).push(1).push(3);
        assert_eq!(pipeline.distinct(), vec![1, 3]);
    }

    #[test]
    fn every_instantiation_is_exercised() {
        assert_eq!(exercise(), 8);
    }

    #[test]
    #[should_panic(expected = "stage 5 of a 2-stage pipeline")]
    fn indexing_past_the_end_panics_with_a_useful_message() {
        let mut pipeline: Pipeline<u32> = Pipeline::new();
        pipeline.push(1).push(2);
        let _ = pipeline.stage(5);
    }

    /// This is why the crate is in the corpus.
    ///
    /// It asserts the wrapped value, so it passes under
    /// `overflow-checks = false` and panics under `overflow-checks = true`.
    /// A sweep must therefore reject every overflow-checked configuration on
    /// TestsPass — and a gate that ran a fixed build would never notice.
    #[test]
    fn the_checksum_wraps_when_overflow_checks_are_off() {
        // 200 + 100 = 300, which does not fit in a u8. Wrapped: 44.
        assert_eq!(checksum(&[200, 100]), 44);
    }
}
