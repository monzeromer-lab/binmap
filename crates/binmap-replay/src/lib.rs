//! Replay debugging (Phase 4).
//!
//! A recording is the only artefact in this product that can answer "who wrote
//! this value" rather than "what was true when it crashed". That is worth a
//! great deal and costs a great deal: recording is slow, `rr` is Linux x86-64
//! only, and it needs performance counters most machines have turned off.
//!
//! So the preflight is not a formality. `F4.1` asks for the constraints to be
//! "stated before a user's first failed attempt rather than after", and the
//! reason is that the failure arrives at the end of a slow recording.
pub mod gdbmi;
pub mod preflight;

pub use preflight::{Obstacle, Readiness, readiness};
