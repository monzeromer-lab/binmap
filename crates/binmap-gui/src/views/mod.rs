//! The views.
//!
//! Each takes the [`AppState`](crate::state::AppState) and a
//! [`Theme`](crate::theme::Theme) and draws it. None of them computes: the
//! only thing they can reach is `binmap_core::facade::Engine`, and this crate
//! does not depend on anything that has one (§2.4).

pub mod chrome;
pub mod environment;
pub mod inspector;
pub mod palette;
pub mod profile_lab;
pub mod targets;

pub use chrome::{NavRail, StatusBar, TitleBar};
