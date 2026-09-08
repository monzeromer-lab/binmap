//! The interface.
//!
//! This crate depends on `binmap-core` and `binmap-session` and on nothing
//! that computes. It reaches analyses through
//! [`binmap_core::facade::Engine`] and it cannot reach them any other way,
//! because it does not depend on a crate that has one (§2.4). Dropping the IPC
//! boundary removed the thing that used to prevent a view from calling an
//! analysis; the dependency graph is what prevents it now, and that is only
//! true for as long as this manifest stays short.
//!
//! Everything in [`state`] is framework-independent on purpose: it applies
//! engine events, decides what the nav rail shows, and holds what the
//! Inspector renders, all as plain data that a unit test can drive with
//! scripted events and no window.

pub mod state;
pub mod theme;

pub use state::{AppState, NavEntry, RunProgress, View};
pub use theme::{Appearance, Colours, Theme};
