//! Mapping an address back to a source line (`F1.6`).
//!
//! `addr2line` does this, and TOOLING §2.3 is emphatic that it is the thing
//! not to reimplement: its `find_frames` returns the inlined frames as well as
//! the physical one, inline reconstruction is subtle, and this crate is what
//! Rust's own backtrace machinery uses. Phase 2 depends on the same call, so
//! getting it wrong here would be wrong twice.
//!
//! **The load-bias trap, noted before it bites.** Every address in a core dump
//! is a runtime address and every address in DWARF is a link-time one, so
//! Phase 2 will have to subtract a bias before any lookup. It does not arise
//! here — a symbol table read from the file on disk is already in link-time
//! addresses — and it is written down because getting it wrong produces
//! plausible-looking, entirely wrong symbolization, which is the worst failure
//! mode available.

use crate::symbols::Symbol;
use binmap_core::error::{Error, Result};
use binmap_core::location::SourceSpan;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Where a symbol's code came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceOrigin {
    pub file: PathBuf,
    pub line: u32,
    /// Frames the compiler inlined into this one, outermost first.
    ///
    /// A physical frame in an optimized Rust binary routinely represents
    /// several logical ones, and "this function did not physically exist" is
    /// exactly the confusion the product exists to resolve.
    pub inlined: Vec<InlinedFrame>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InlinedFrame {
    pub function: String,
    pub file: Option<PathBuf>,
    pub line: Option<u32>,
}

impl SourceOrigin {
    pub fn span(&self) -> SourceSpan {
        SourceSpan { file: self.file.clone(), line: self.line, column: None }
    }
}

/// A DWARF context for one artifact.
///
/// Construction parses and indexes the debug info and is expensive; lookups
/// are cheap. TOOLING §2.3 says to build one per binary and reuse it, so this
/// owns the mapped file and hands out lookups.
#[derive(Debug)]
pub struct SourceMap {
    /// Resolved spans by address, built eagerly for the symbols asked about.
    ///
    /// Holding the `addr2line::Context` itself would mean holding a borrow of
    /// the file bytes, which makes the type self-referential. The symbols are
    /// known up front, so resolving them all at once and keeping the answers
    /// is simpler and no slower.
    by_address: BTreeMap<u64, SourceOrigin>,
    /// Whether the artifact carried debug info at all.
    pub has_debug_info: bool,
}

impl SourceMap {
    /// Resolve every symbol's address in one pass.
    ///
    /// A binary with no debug info is not an error — it is the normal result
    /// of a release build, and `strip` or `debug = 0` produces it. The caller
    /// gets an empty map and `has_debug_info: false`, which is a fact to
    /// report rather than a failure to raise.
    pub fn resolve(artifact: &Path, symbols: &[Symbol]) -> Result<Self> {
        let bytes = std::fs::read(artifact).map_err(|source| Error::io(artifact, source))?;
        let object = object::File::parse(&*bytes).map_err(|error| {
            Error::Other(format!("{} is not an object file: {error}", artifact.display()))
        })?;

        let context = match addr2line::Context::from_dwarf(
            match addr2line::gimli::Dwarf::load(|id| load_section(&object, id)) {
                Ok(dwarf) => dwarf,
                Err(_) => return Ok(Self { by_address: BTreeMap::new(), has_debug_info: false }),
            },
        ) {
            Ok(context) => context,
            // No debug info, or none we can read. Either way the answer is
            // "nothing to map", not "the read failed".
            Err(_) => return Ok(Self { by_address: BTreeMap::new(), has_debug_info: false }),
        };

        let mut by_address = BTreeMap::new();
        for symbol in symbols {
            if let Some(origin) = lookup(&context, symbol.address) {
                by_address.insert(symbol.address, origin);
            }
        }

        let has_debug_info = !by_address.is_empty();
        Ok(Self { by_address, has_debug_info })
    }

    pub fn of(&self, symbol: &Symbol) -> Option<&SourceOrigin> {
        self.by_address.get(&symbol.address)
    }

    pub fn at(&self, address: u64) -> Option<&SourceOrigin> {
        self.by_address.get(&address)
    }

    pub fn len(&self) -> usize {
        self.by_address.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_address.is_empty()
    }

    /// What fraction of the symbols asked about were mapped.
    ///
    /// Reported rather than assumed: an attribution that maps a tenth of its
    /// symbols to source is a much weaker thing than one that maps all of
    /// them, and the difference is invisible unless it is carried.
    pub fn coverage(&self, symbols: &[Symbol]) -> f64 {
        if symbols.is_empty() {
            return 0.0;
        }
        self.by_address.len() as f64 / symbols.len() as f64
    }
}

/// One address, with its inlined frames.
fn lookup<R>(context: &addr2line::Context<R>, address: u64) -> Option<SourceOrigin>
where
    R: addr2line::gimli::Reader,
{
    let mut frames = context.find_frames(address).skip_all_loads().ok()?;

    let mut physical: Option<SourceOrigin> = None;
    let mut inlined = Vec::new();

    while let Ok(Some(frame)) = frames.next() {
        let function = frame
            .function
            .as_ref()
            .and_then(|name| name.demangle().ok().map(|name| name.into_owned()))
            .unwrap_or_default();
        let file = frame.location.as_ref().and_then(|l| l.file).map(PathBuf::from);
        let line = frame.location.as_ref().and_then(|l| l.line);

        match &mut physical {
            // The first frame addr2line yields is the innermost; the last is
            // the physical one the address actually lies in.
            None => {
                physical = Some(SourceOrigin {
                    file: file.clone().unwrap_or_default(),
                    line: line.unwrap_or(0),
                    inlined: Vec::new(),
                });
                if !function.is_empty() {
                    inlined.push(InlinedFrame { function, file, line });
                }
            }
            Some(origin) => {
                // A later frame is the enclosing one, so it is the better
                // answer for "where does this code live".
                if let Some(file) = file.clone() {
                    origin.file = file;
                }
                if let Some(line) = line {
                    origin.line = line;
                }
                if !function.is_empty() {
                    inlined.push(InlinedFrame { function, file, line });
                }
            }
        }
    }

    let mut origin = physical?;
    if origin.file.as_os_str().is_empty() {
        return None;
    }
    // Outermost first, and the physical frame itself is not an inlined one.
    inlined.reverse();
    inlined.pop();
    origin.inlined = inlined;
    Some(origin)
}

/// Hand one DWARF section to `gimli`.
///
/// A section that is absent is empty, not an error: a binary built without
/// `.debug_loclists` simply has none, and refusing to read the rest because of
/// it would make partial debug info useless.
fn load_section<'a>(
    object: &object::File<'a>,
    id: addr2line::gimli::SectionId,
) -> std::result::Result<
    addr2line::gimli::EndianSlice<'a, addr2line::gimli::RunTimeEndian>,
    std::convert::Infallible,
> {
    use object::read::{Object, ObjectSection};
    let endian = if object.is_little_endian() {
        addr2line::gimli::RunTimeEndian::Little
    } else {
        addr2line::gimli::RunTimeEndian::Big
    };
    let data =
        object.section_by_name(id.name()).and_then(|section| section.data().ok()).unwrap_or(&[]);
    Ok(addr2line::gimli::EndianSlice::new(data, endian))
}
