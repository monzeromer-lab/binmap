//! Diffing two builds (`F1.5`).
//!
//! The easy version of this subtracts two symbol tables and reports what
//! appeared and vanished. It is also nearly useless on real Rust binaries,
//! because so much of what "appeared and vanished" is the same code under a
//! different name — and the plan says so directly: "Diff two binaries,
//! **including generic-argument renames**."
//!
//! A monomorphized symbol carries its type arguments, and those change
//! whenever the calling code changes type, whenever an inference decision
//! moves, and whenever a closure is numbered differently. Reporting
//! `Tally<u32>` as removed and `Tally<u64>` as added is technically true and
//! tells the reader nothing. Grouping them under the generic and reporting the
//! aggregate is the answer they wanted.

use crate::symbols::SymbolTable;
use binmap_core::attribution::Driver;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One thing that changed size between two builds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Change {
    /// The symbol, or the generic where instantiations were grouped.
    pub name: String,
    pub before: u64,
    pub after: u64,
    /// Positive is growth.
    pub delta: i64,
    /// How many symbols this figure covers. Above one it is a generic whose
    /// instantiations were grouped.
    pub symbols: u32,
}

impl Change {
    pub fn grew(&self) -> bool {
        self.delta > 0
    }
}

/// What changed between two builds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SizeDiff {
    /// Present in both, and bigger. Largest growth first.
    pub grew: Vec<Change>,
    /// Present in both, and smaller. Largest saving first.
    pub shrank: Vec<Change>,
    /// Only in the second build.
    pub added: Vec<Change>,
    /// Only in the first.
    pub removed: Vec<Change>,
    /// By category, so "what kind of thing grew" is answerable without
    /// reading the list.
    pub by_driver: Vec<(Driver, i64)>,
    pub before_bytes: u64,
    pub after_bytes: u64,
    /// How many symbols were matched across the two builds only because their
    /// generic arguments were ignored.
    ///
    /// Reported because it is the number that says how much of this diff rests
    /// on the grouping being right — a diff where everything matched by
    /// generic path is a different kind of claim from one where nothing did.
    pub matched_by_generic: u32,
}

impl SizeDiff {
    pub fn total_delta(&self) -> i64 {
        self.after_bytes as i64 - self.before_bytes as i64
    }

    /// Whether anything at all changed.
    pub fn is_empty(&self) -> bool {
        self.grew.is_empty()
            && self.shrank.is_empty()
            && self.added.is_empty()
            && self.removed.is_empty()
    }
}

/// The key a symbol is matched on across two builds.
///
/// The generic path where the name carried arguments, the whole name
/// otherwise. This is the entire mechanism behind "including generic-argument
/// renames", and it is one line because `split` already did the work.
fn match_key(name: &str) -> (String, bool) {
    let origin = crate::attribution::split(name);
    match origin.arguments {
        Some(_) => (origin.generic_path, true),
        None => (name.to_string(), false),
    }
}

/// Diff two symbol tables.
///
/// `own_crates` is only used to categorise the change by driver; matching does
/// not depend on it.
pub fn diff(before: &SymbolTable, after: &SymbolTable, own_crates: &[String]) -> SizeDiff {
    // Roll each side up to its match key first, so a generic instantiated
    // three times on one side and four on the other compares as one thing.
    let mut left: BTreeMap<String, (u64, u32, bool)> = BTreeMap::new();
    let mut right: BTreeMap<String, (u64, u32, bool)> = BTreeMap::new();
    let mut driver_delta: BTreeMap<Driver, i64> = BTreeMap::new();

    for (table, side, sign) in [(before, &mut left, -1i64), (after, &mut right, 1i64)] {
        for symbol in &table.symbols {
            if symbol.size == 0 {
                continue;
            }
            let (key, generic) = match_key(&symbol.name);
            let entry = side.entry(key).or_insert((0, 0, generic));
            entry.0 += symbol.size;
            entry.1 += 1;
            *driver_delta.entry(crate::attribution::classify(symbol, own_crates)).or_default() +=
                sign * symbol.size as i64;
        }
    }

    let mut grew = Vec::new();
    let mut shrank = Vec::new();
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut matched_by_generic = 0u32;

    for (key, (after_bytes, symbols, generic)) in &right {
        match left.get(key) {
            Some((before_bytes, before_symbols, _)) => {
                if *generic && (symbols != before_symbols) {
                    // The instantiations differ, so this pair only matched
                    // because the arguments were ignored.
                    matched_by_generic += 1;
                }
                let delta = *after_bytes as i64 - *before_bytes as i64;
                if delta == 0 {
                    continue;
                }
                let change = Change {
                    name: key.clone(),
                    before: *before_bytes,
                    after: *after_bytes,
                    delta,
                    symbols: *symbols,
                };
                if delta > 0 { grew.push(change) } else { shrank.push(change) }
            }
            None => added.push(Change {
                name: key.clone(),
                before: 0,
                after: *after_bytes,
                delta: *after_bytes as i64,
                symbols: *symbols,
            }),
        }
    }

    for (key, (before_bytes, symbols, _)) in &left {
        if right.contains_key(key) {
            continue;
        }
        removed.push(Change {
            name: key.clone(),
            before: *before_bytes,
            after: 0,
            delta: -(*before_bytes as i64),
            symbols: *symbols,
        });
    }

    grew.sort_by_key(|change| std::cmp::Reverse(change.delta));
    shrank.sort_by_key(|change| change.delta);
    added.sort_by_key(|change| std::cmp::Reverse(change.delta));
    removed.sort_by_key(|change| change.delta);

    let mut by_driver: Vec<(Driver, i64)> =
        driver_delta.into_iter().filter(|(_, delta)| *delta != 0).collect();
    by_driver.sort_by_key(|(_, delta)| std::cmp::Reverse(delta.abs()));

    SizeDiff {
        grew,
        shrank,
        added,
        removed,
        by_driver,
        before_bytes: before.total_bytes(),
        after_bytes: after.total_bytes(),
        matched_by_generic,
    }
}
