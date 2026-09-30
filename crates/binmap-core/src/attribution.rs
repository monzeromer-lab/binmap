//! What an attribution *is*, separately from how it is computed.
//!
//! The vocabulary lives here for the same reason the gate vocabulary does: the
//! interface renders it and the session artifact carries it, and neither may
//! depend on `binmap-binary`, which reads ELF files and would drag the whole
//! binary layer across the boundary §2.4 draws. Computing it is
//! `binmap-binary`'s; this is the shape.

use serde::{Deserialize, Serialize};

/// What a symbol's bytes are spent on.
///
/// The categories are `F1.4`'s list, and they are the ones that recur across
/// almost every Rust binary. A category is a claim about *why* bytes exist,
/// which is more useful than a claim about where they sit — "formatting
/// machinery" tells a user what to change; ".text" does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Driver {
    /// The user's own code.
    Yours,
    /// A dependency's code.
    Dependency,
    /// `core::fmt` and everything it drags in. Usually the largest single
    /// category, and usually a surprise.
    Formatting,
    /// Panic machinery and the messages it prints.
    Panic,
    /// Unwinding tables. `panic = "abort"` removes them.
    Unwinding,
    /// `Drop` implementations the compiler generated.
    DropGlue,
    /// Vtables for trait objects.
    Vtable,
    /// Static data: tables, string literals, constants.
    StaticData,
    /// The standard library, where it is none of the above.
    StandardLibrary,
    /// Runtime support that is not Rust's: `libc` shims, compiler intrinsics.
    Runtime,
}

impl Driver {
    /// What this category is, in the words the interface uses.
    pub fn label(self) -> &'static str {
        match self {
            Driver::Yours => "your code",
            Driver::Dependency => "dependencies",
            Driver::Formatting => "formatting machinery",
            Driver::Panic => "panic strings and machinery",
            Driver::Unwinding => "unwinding tables",
            Driver::DropGlue => "Drop glue",
            Driver::Vtable => "vtables",
            Driver::StaticData => "static data",
            Driver::StandardLibrary => "the standard library",
            Driver::Runtime => "runtime support",
        }
    }

    /// What a user could do about it, where there is something.
    ///
    /// `None` where the honest answer is "nothing directly" — and saying
    /// nothing is better than inventing advice.
    pub fn remedy(self) -> Option<&'static str> {
        match self {
            Driver::Formatting => Some(
                "Reachable formatting pulls in core::fmt. Removing the last `{}` from a \
                 reachable path removes the machinery with it.",
            ),
            Driver::Panic => Some(
                "panic = \"abort\" drops the unwinding half; build-std with \
                 panic_immediate_abort drops the messages too.",
            ),
            Driver::Unwinding => Some("panic = \"abort\" removes these entirely."),
            Driver::Vtable => {
                Some("A trait object that is only ever one concrete type can be made generic.")
            }
            Driver::DropGlue => Some(
                "Types with no Drop of their own do not generate glue; check for a Drop \
                      on a widely-used wrapper.",
            ),
            Driver::Yours
            | Driver::Dependency
            | Driver::StaticData
            | Driver::StandardLibrary
            | Driver::Runtime => None,
        }
    }
}

/// Bytes, grouped by something.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Group {
    pub key: String,
    pub bytes: u64,
    /// How many symbols rolled into this figure.
    pub symbols: u32,
    /// A few of them, for the interface to show without holding the rest.
    pub examples: Vec<String>,
}

/// One generic, and every instantiation of it (`F1.3`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Monomorphization {
    /// The path with generic arguments stripped — what they have in common.
    pub generic_path: String,
    /// Total bytes across every instantiation. The number that decides whether
    /// collapsing is worth doing.
    pub total_bytes: u64,
    /// How many instantiations there are.
    pub instantiations: u32,
    /// The type arguments seen, in size order, largest first.
    pub arguments: Vec<(String, u64)>,
}

impl Monomorphization {
    /// What collapsing every instantiation to one would save, at most.
    ///
    /// The largest instantiation has to stay, so this is the total less that.
    /// An upper bound and labelled as one: collapsing usually costs an
    /// indirection, and two instantiations that differ only in a type
    /// parameter may still differ in what the optimiser did to them.
    pub fn collapsible_bytes(&self) -> u64 {
        let largest = self.arguments.first().map(|(_, bytes)| *bytes).unwrap_or(0);
        self.total_bytes.saturating_sub(largest)
    }
}

/// Everything the attribution layer produces from one symbol table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Attribution {
    /// Bytes per crate, largest first.
    pub crates: Vec<Group>,
    /// Bytes per category, largest first.
    pub drivers: Vec<(Driver, u64)>,
    /// Generics with more than one instantiation, by aggregate cost.
    pub monomorphizations: Vec<Monomorphization>,
    /// Bytes the symbol table accounts for.
    pub attributed_bytes: u64,
    /// How much of the attribution rests on inferred sizes.
    pub inferred_fraction: f64,
    /// `false` where the binary used legacy mangling, in which case generic
    /// grouping is prefix matching and worth less.
    pub generic_arguments_available: bool,
}
