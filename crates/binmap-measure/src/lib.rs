//! What a build weighs, how long it takes, and which differences are real.
//!
//! The rule the whole crate serves: **the noise floor is measured once per
//! machine by timing one unchanged binary repeatedly, and a result inside it is
//! reported inconclusive, never coloured as a win** (§6). Everything here is
//! shaped so that rule is hard to break — the comparison returns a verdict with
//! an explicit `Inconclusive` variant rather than a number the caller might
//! round in its own favour.

pub mod bloaty;
pub mod hyperfine;
pub mod pareto;
pub mod sections;
pub mod size;
pub mod timing;

pub use pareto::{Point, frontier};
pub use size::measure_size;
pub use timing::{NoiseFloor, Samples, compare};
pub use hyperfine::Timing;
