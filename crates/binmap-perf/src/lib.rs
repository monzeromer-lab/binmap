//! Performance attribution (Phase 3).
//!
//! Built on Phase 2's symbolization rather than beside it: `F3.2` says samples
//! are attributed "through the same symbolization layer the crash analyzer
//! uses", and two implementations of that lookup would drift — with the
//! inline handling, which is the part that matters most here, drifting first.
//!
//! The discipline that makes a profiler trustworthy is the one the sweep
//! already applies to sizes: **every number is a statistic, and a difference
//! inside the measurement's own error is not a difference.** A profiler that
//! ranks two functions three samples apart out of four hundred is inventing an
//! ordering.

pub mod attribute;
pub mod capture;
pub mod flame;
pub mod sample;

pub use sample::{Profile, Source, Stack};
