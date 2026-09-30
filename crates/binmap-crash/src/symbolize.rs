//! Turning an address into a place in the source (`F2.3`).
//!
//! The DWARF work already exists: `F1.6` built `binmap_binary::SourceMap`,
//! which resolves an address to a file, a line, and the frames the compiler
//! inlined into it. Writing a second addr2line wrapper here would mean two
//! implementations of the same lookup drifting apart, and the inline handling
//! is the part that would drift.
//!
//! So this is the crash-side view of that map: a stack frame's worth of
//! locations rather than a symbol's, and an honest answer when DWARF says
//! nothing. An address with no debug information gets its name from the symbol
//! table and reports the line as unknown, rather than borrowing the nearest
//! line from a neighbouring function — which is how a debugger points
//! confidently at the wrong place.

use binmap_binary::{SourceMap, SymbolTable};
use binmap_core::error::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// One source location a frame corresponds to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Location {
    /// Demangled where it demangles, left alone where it does not.
    pub function: Option<String>,
    pub file: Option<String>,
    pub line: Option<u32>,
    /// Whether this was inlined into the frame below it.
    ///
    /// The distinction a stack pane draws: an inlined frame never had its own
    /// machine-level frame, and showing it as though it did misrepresents what
    /// the stack looked like.
    pub inlined: bool,
}

impl Location {
    /// The one line a stack trace shows.
    pub fn describe(&self) -> String {
        let function = self.function.as_deref().unwrap_or("<unknown>");
        match (&self.file, self.line) {
            (Some(file), Some(line)) => format!("{function} at {file}:{line}"),
            (Some(file), None) => format!("{function} at {file}"),
            // Stated rather than omitted: "no line information" is a fact
            // about the build, and a reader who does not see it will assume
            // the tool failed instead.
            _ => format!("{function} (no line information)"),
        }
    }
}

/// Everything one address resolves to, innermost inline frame first.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Resolved {
    pub locations: Vec<Location>,
}

impl Resolved {
    pub fn is_empty(&self) -> bool {
        self.locations.is_empty()
    }

    /// The location that actually had a machine-level frame.
    ///
    /// The last one: everything above it was inlined into it.
    pub fn physical(&self) -> Option<&Location> {
        self.locations.last()
    }

    pub fn inline_depth(&self) -> usize {
        self.locations.iter().filter(|location| location.inlined).count()
    }
}

/// A binary, ready to answer address lookups.
pub struct Symbolizer {
    source: SourceMap,
    symbols: SymbolTable,
}

impl Symbolizer {
    /// Load a binary's symbols and resolve the addresses a stack visits.
    ///
    /// The addresses are passed in rather than discovered, because a faulting
    /// program counter is in the *middle* of a function and resolving only
    /// symbol starts misses every frame — which it did, reporting "no line
    /// information" for a binary with full debug info.
    ///
    /// Missing DWARF is not an error: a release build with `debug = 0` has
    /// none, and a stack from one can still name every function. The caller
    /// finds out through the resolutions, which report no line rather than no
    /// answer.
    pub fn load(artifact: &Path, addresses: &[u64]) -> Result<Self> {
        let symbols = SymbolTable::read(artifact)?;
        let source = SourceMap::for_addresses(artifact, addresses)?;
        Ok(Self { source, symbols })
    }

    /// What fraction of the addresses asked about got a line.
    ///
    /// Shown once beside a stack rather than repeated per frame: a stack where
    /// every frame resolved and one where a third did are very different
    /// things, and the reader should know which they have before reading it.
    pub fn line_coverage(&self, addresses: &[u64]) -> f64 {
        if addresses.is_empty() {
            return 0.0;
        }
        let resolved =
            addresses.iter().filter(|address| self.source.at(**address).is_some()).count();
        resolved as f64 / addresses.len() as f64
    }

    pub fn has_debug_information(&self) -> bool {
        !self.source.is_empty()
    }

    /// Resolve one link-time address.
    pub fn resolve(&self, address: u64) -> Resolved {
        let symbol = self.symbols.containing(address);

        let Some(origin) = self.source.at(address) else {
            // No DWARF here. The symbol table still knows the function, and
            // "this function, line unknown" is far more useful than nothing —
            // as long as it does not invent a line to go with it.
            return Resolved {
                locations: symbol
                    .map(|symbol| Location {
                        function: Some(symbol.name.clone()),
                        file: None,
                        line: None,
                        inlined: false,
                    })
                    .into_iter()
                    .collect(),
            };
        };

        // Inlined frames are recorded outermost first and a stack reads
        // innermost first, so they are reversed. The *last* of them is the
        // physical function — the one everything else was inlined into — so
        // appending a separate physical entry printed it twice.
        let mut locations: Vec<Location> = origin
            .inlined
            .iter()
            .rev()
            .map(|frame| Location {
                function: Some(frame.function.clone()),
                file: frame.file.as_ref().map(|path| path.display().to_string()),
                line: frame.line,
                inlined: true,
            })
            .collect();

        match locations.last_mut() {
            Some(physical) => {
                physical.inlined = false;
                // The line table knows exactly where the machine was; the
                // inline record only knows where the call site was.
                physical.file = Some(origin.file.display().to_string());
                physical.line = Some(origin.line);
            }
            // No inline records at all: one frame, straight from the line
            // table.
            None => locations.push(Location {
                function: symbol.map(|symbol| symbol.name.clone()),
                file: Some(origin.file.display().to_string()),
                line: Some(origin.line),
                inlined: false,
            }),
        }

        Resolved { locations }
    }
}
