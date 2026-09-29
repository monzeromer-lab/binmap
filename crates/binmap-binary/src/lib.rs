//! Reading a built artifact and saying where its bytes went.
//!
//! Phase 1's engine layer. Everything artifact-specific lives behind the
//! traits in `binmap-core`, and this is the native implementation of them.

pub mod attribution;
pub mod symbols;

pub use attribution::{Attribution, Driver, Monomorphization, attribute};
pub use symbols::{Mangling, Sizing, Symbol, SymbolTable};
