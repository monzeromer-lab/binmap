//! The C# backend (Phase 2.5, Phase 3.5).
//!
//! Two things here have no analogue in the Rust or web backends, and both are
//! why the backend is worth having rather than a third way to count bytes.
//!
//! The **ILC dependency graph** answers why the compiler kept an item — the
//! chain of references from a root that stopped trimming from removing it.
//! DWARF says what is in a binary and cannot say why.
//!
//! **Trim and AOT warnings** identify code that cannot be statically analysed:
//! reflection over types that might be trimmed, dynamic code generation that
//! AOT cannot support. Each is a finding with a source location, a
//! deterministic origin and a known class of remedy, which makes it the C#
//! answer to monomorphization findings.
//!
//! `TOOLING-DOTNET §2`'s central instruction shapes both: **do not
//! reverse-engineer NativeAOT's symbol mangling.** The compiler emits
//! structured accounting already, and the mangling is not a stable interface.
pub mod configuration;
pub mod dgml;
pub mod warnings;

pub use dgml::{Graph, Item, Reason};
pub use warnings::{Warning, Warnings};
