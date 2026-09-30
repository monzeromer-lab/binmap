//! The symbol table, and what each symbol actually is (`F1.1`).
//!
//! `object` does the container parsing (TOOLING §2.1) and `rustc-demangle`
//! turns mangled names back into paths (§2.4). Neither is reimplemented here:
//! the plan's `F1.1` says "a v0 demangling parser of our own", and TOOLING
//! §2.4 says use the crate, which is the more recent judgement and the right
//! one — `rustc-demangle` is what Rust's own backtrace machinery uses, and a
//! second implementation would be a second set of bugs for no gain.
//!
//! What *is* ours is everything after the name: deciding which crate a symbol
//! belongs to, which generic it is an instantiation of, and what category of
//! cost it represents. No general-purpose tool does that, and it is the whole
//! reason this crate exists.
//!
//! **The size caveat, stated once.** ELF symbol sizes are frequently zero or
//! wrong for compiler-generated symbols. Deriving a size from the next
//! symbol's address is the usual workaround and is itself wrong across section
//! boundaries and with alignment padding. Both problems are handled below, and
//! neither is fully solved — which is why `F1.2` requires cross-checking
//! against an independent tool to a stated tolerance rather than trusting this.

use binmap_core::error::{Error, Result};
use object::read::{Object, ObjectSection, ObjectSymbol};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// One symbol, with its name resolved and its size established.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Symbol {
    /// The name as it appears in the table.
    pub mangled: String,
    /// The demangled path, or the mangled name where it is not a Rust symbol.
    pub name: String,
    pub address: u64,
    pub size: u64,
    /// The section it lives in — `.text`, `.rodata` and so on.
    pub section: String,
    /// How `size` was arrived at, because one of the two ways is a guess.
    pub sizing: Sizing,
}

/// Where a symbol's size came from.
///
/// Worth recording per symbol rather than assuming: an attribution built
/// mostly from inferred sizes deserves less confidence than one built from
/// declared ones, and the difference is invisible unless it is carried.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sizing {
    /// The symbol table declared it. Trustworthy.
    Declared,
    /// Derived from the distance to the next symbol. A guess, and wrong where
    /// alignment padding sits between them.
    InferredFromNeighbour,
    /// Declared as zero with no neighbour to measure against.
    Unknown,
}

/// Every symbol in an artifact, with the section table for context.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SymbolTable {
    pub symbols: Vec<Symbol>,
    /// Symbols whose size had to be inferred, as a fraction of the total.
    ///
    /// Reported rather than hidden: it is the single number that says how much
    /// to trust everything derived from this table.
    pub inferred_fraction: f64,
    /// Whether the table came from `.symtab` or the much poorer `.dynsym`.
    pub stripped: bool,
}

impl SymbolTable {
    /// Read an artifact's symbols.
    ///
    /// A binary with no symbol table at all is not an error — it is a stripped
    /// binary, which is a normal thing to ship and a fact worth reporting
    /// rather than a failure. The caller gets an empty table and
    /// `stripped: true`.
    pub fn read(artifact: &Path) -> Result<Self> {
        let bytes = std::fs::read(artifact).map_err(|source| Error::io(artifact, source))?;
        let file = object::File::parse(&*bytes).map_err(|error| {
            Error::Other(format!("{} is not an object file: {error}", artifact.display()))
        })?;
        Ok(Self::from_object(&file))
    }

    pub(crate) fn from_object(file: &object::File<'_>) -> Self {
        // `.symtab` where it survives, `.dynsym` otherwise. The latter holds
        // only exported symbols, so a stripped binary yields a table that is
        // correct and nearly empty — which is the honest answer.
        let full: Vec<_> = file.symbols().collect();
        let stripped = full.is_empty();
        let entries: Vec<_> = if stripped { file.dynamic_symbols().collect() } else { full };

        let mut symbols: Vec<Symbol> = entries
            .iter()
            .filter(|symbol| symbol.is_definition())
            .filter_map(|symbol| {
                let mangled = symbol.name().ok()?.to_string();
                if mangled.is_empty() {
                    return None;
                }
                let section = symbol
                    .section_index()
                    .and_then(|index| file.section_by_index(index).ok())
                    .and_then(|section| section.name().ok().map(str::to_string))
                    .unwrap_or_default();

                Some(Symbol {
                    name: demangle(&mangled),
                    mangled,
                    address: symbol.address(),
                    size: symbol.size(),
                    section,
                    sizing: if symbol.size() > 0 { Sizing::Declared } else { Sizing::Unknown },
                })
            })
            .collect();

        symbols.sort_by_key(|symbol| (symbol.section.clone(), symbol.address));
        infer_missing_sizes(&mut symbols);

        let inferred = symbols.iter().filter(|symbol| symbol.sizing != Sizing::Declared).count();
        let inferred_fraction =
            if symbols.is_empty() { 0.0 } else { inferred as f64 / symbols.len() as f64 };

        Self { symbols, inferred_fraction, stripped }
    }

    /// The bytes these symbols account for.
    pub fn total_bytes(&self) -> u64 {
        self.symbols.iter().map(|symbol| symbol.size).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.symbols.is_empty()
    }
}

/// Fill in zero sizes from the distance to the next symbol.
///
/// Only within one section: the gap between the last symbol of `.text` and the
/// first of `.rodata` is not a symbol's size, it is the distance between two
/// sections, and treating it as a size is how a naive implementation reports a
/// function as megabytes.
///
/// Symbols are assumed sorted by `(section, address)`.
fn infer_missing_sizes(symbols: &mut [Symbol]) {
    for index in 0..symbols.len() {
        if symbols[index].sizing != Sizing::Unknown {
            continue;
        }
        let Some(next) = symbols.get(index + 1) else { continue };
        if next.section != symbols[index].section {
            continue;
        }
        let Some(gap) = next.address.checked_sub(symbols[index].address) else { continue };

        // A gap larger than this is padding or a hole, not a function. The
        // bound is arbitrary and deliberately generous; the alternative is
        // attributing a megabyte to a symbol that declared nothing.
        const PLAUSIBLE: u64 = 1 << 20;
        if gap > 0 && gap <= PLAUSIBLE {
            symbols[index].size = gap;
            symbols[index].sizing = Sizing::InferredFromNeighbour;
        }
    }
}

/// Turn a mangled name into a readable path.
///
/// Both schemes: legacy `_ZN..` with its hash suffix, and v0 `_R..` which
/// carries the generic arguments. `{:#}` asks `rustc-demangle` to drop the
/// legacy hash, which is noise in every context we display a name.
pub fn demangle(mangled: &str) -> String {
    format!("{:#}", rustc_demangle::demangle(mangled))
}

/// Which mangling scheme a name uses.
///
/// Worth knowing, because v0 carries the generic arguments and legacy does
/// not: monomorphization grouping degrades to prefix matching on a legacy
/// binary, and the interface says so rather than silently producing worse
/// results (TOOLING §2.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mangling {
    /// `_R…`, which encodes crate, path and generic arguments.
    V0,
    /// `_ZN…`, which encodes a path and a disambiguating hash, and loses the
    /// generic arguments.
    Legacy,
    /// Not a Rust symbol: C, assembly, or a compiler intrinsic.
    Foreign,
}

impl Mangling {
    pub fn of(mangled: &str) -> Self {
        if mangled.starts_with("_R") {
            Mangling::V0
        } else if mangled.starts_with("_ZN") || mangled.starts_with("__ZN") {
            Mangling::Legacy
        } else {
            Mangling::Foreign
        }
    }
}

impl SymbolTable {
    /// The symbol covering an address, if any.
    ///
    /// The fallback for an address DWARF does not describe — most of libc, and
    /// anything built without debug info. Bounded by the symbol's own size
    /// rather than by "the nearest symbol below": the nearest-below answer is
    /// never `None`, so a wild pointer would be reported as being inside
    /// whichever function happened to be last in the table.
    pub fn containing(&self, address: u64) -> Option<&Symbol> {
        if let Some(exact) = self.symbols.iter().find(|symbol| {
            symbol.size > 0 && (symbol.address..symbol.address + symbol.size).contains(&address)
        }) {
            return Some(exact);
        }
        self.nearest_before(address)
    }

    /// The closest symbol at or below `address`, within a bound.
    ///
    /// The fallback for a symbol table that declares no size, which is most of
    /// a stripped shared library: libc exports `__libc_start_main` with a size
    /// and plenty of its internals without one, so an exact-containment lookup
    /// alone left real frames unnamed.
    ///
    /// **Bounded, and that is the whole difference.** An unbounded
    /// nearest-below never returns `None`, so a wild pointer is reported as
    /// being inside whichever function happened to be last in the table — a
    /// confident, wrong answer of exactly the kind this product exists to
    /// avoid. A function larger than this bound exists; a frame more than this
    /// far past the last symbol is far more likely to be a bad address.
    pub fn nearest_before(&self, address: u64) -> Option<&Symbol> {
        /// Generous for a function, mean for a wild pointer.
        const BOUND: u64 = 64 * 1024;

        self.symbols
            .iter()
            .filter(|symbol| symbol.address <= address && symbol.address > 0)
            .filter(|symbol| address - symbol.address <= BOUND)
            // Executable sections only: a code address inside `.rodata` is not
            // a function, it is a misread.
            .filter(|symbol| symbol.section == ".text" || symbol.section.starts_with(".text"))
            .max_by_key(|symbol| symbol.address)
    }
}

impl SymbolTable {
    /// Which scheme this binary's Rust symbols were mangled with.
    ///
    /// `None` when there are no Rust symbols to judge by. Where it is
    /// `Legacy`, generic grouping will be worse and the interface must say so.
    pub fn mangling(&self) -> Option<Mangling> {
        let mut v0 = 0usize;
        let mut legacy = 0usize;
        for symbol in &self.symbols {
            match Mangling::of(&symbol.mangled) {
                Mangling::V0 => v0 += 1,
                Mangling::Legacy => legacy += 1,
                Mangling::Foreign => {}
            }
        }
        match (v0, legacy) {
            (0, 0) => None,
            (v0, legacy) if v0 >= legacy => Some(Mangling::V0),
            _ => Some(Mangling::Legacy),
        }
    }
}
