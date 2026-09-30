//! Reading a built artifact and saying where its bytes went.
//!
//! Phase 1's engine layer. Everything artifact-specific lives behind the
//! traits in `binmap-core`, and this is the native implementation of them.

pub mod attribution;
pub mod diff;
pub mod source;
pub mod symbols;

pub use attribution::attribute;
pub use binmap_core::attribution::{Attribution, Driver, Group, Monomorphization};
pub use diff::{Change, SizeDiff, diff};
pub use source::{InlinedFrame, SourceMap, SourceOrigin};
pub use symbols::{Mangling, Sizing, Symbol, SymbolTable};
