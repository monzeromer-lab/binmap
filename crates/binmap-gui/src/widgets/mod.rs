//! Every custom widget in the application.
//!
//! DESIGN-GUI §7 rule 2 puts them all in one module so a framework API change
//! touches one place rather than every view. Nothing here knows about the
//! engine; each takes plain data and draws it.

pub mod badge;
pub mod panel;
pub mod provenance;

pub use badge::{Badge, Tone};
pub use panel::{Section, eyebrow};
pub use provenance::ProvenanceBadge;
