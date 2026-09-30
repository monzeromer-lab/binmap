//! Every module the process had mapped (`F2.3`).
//!
//! A crash rarely happens in one binary. A Rust panic goes through `abort` in
//! libc, so the innermost frames are libc's, and an unwinder that only knows
//! the executable's `.eh_frame` stops at the first of them — which for an
//! aborting process is frame zero, leaving the entire Rust stack unreachable.
//! That is precisely what happened here before this module existed.
//!
//! So unwinding is per module: for each frame, find which mapped file the
//! program counter is in, and use *that* file's CFI and *that* file's load
//! bias. Each module gets its own bias because each is mapped independently.
//!
//! Libraries are read from the paths `NT_FILE` recorded. When one is missing —
//! a container, a different machine, an upgraded package — that module is
//! simply unavailable, and a frame in it says so instead of being unwound with
//! another module's tables.

use crate::dump::CoreDump;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One mapped file we could load.
pub struct Module {
    pub path: String,
    /// Where its first mapping starts at runtime.
    pub base: u64,
    /// Runtime bias: add to a link-time address to get a runtime one.
    pub bias: u64,
    /// The file's bytes.
    pub data: Vec<u8>,
    /// Its `.eh_frame`, extracted once.
    pub eh_frame: Vec<u8>,
    /// The link-time address of `.eh_frame`, which its pointer encodings are
    /// relative to. An unwinder given the wrong base decodes garbage without
    /// ever failing.
    pub eh_frame_address: u64,
    /// Where the mapping ends, so a lookup can tell which module owns an
    /// address.
    pub end: u64,
}

impl Module {
    pub fn contains(&self, address: u64) -> bool {
        (self.base..self.end).contains(&address)
    }

    /// A runtime address as this module's DWARF numbers it.
    pub fn to_link_time(&self, runtime: u64) -> Option<u64> {
        runtime.checked_sub(self.bias)
    }
}

impl std::fmt::Debug for Module {
    /// Without this the `data` field prints megabytes of bytes.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Module")
            .field("path", &self.path)
            .field("base", &format_args!("{:#x}", self.base))
            .field("bias", &format_args!("{:#x}", self.bias))
            .field("eh_frame", &format_args!("{} bytes", self.eh_frame.len()))
            .finish()
    }
}

/// A module that could not be loaded, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Unavailable {
    pub path: String,
    pub base: u64,
    pub because: String,
}

/// Everything mapped, loaded where possible.
#[derive(Debug, Default)]
pub struct Modules {
    pub loaded: Vec<Module>,
    /// Recorded rather than dropped: a frame in an unavailable module is a
    /// frame we cannot symbolize, and the reader should be told which library
    /// is missing rather than shown a gap.
    pub unavailable: Vec<Unavailable>,
}

impl Modules {
    /// Load every mapped file the core names.
    ///
    /// `substitute` lets a caller point at a copy of the executable — the
    /// usual case, where the core is being read on a different machine or the
    /// binary has moved.
    pub fn load(dump: &CoreDump, substitute: Option<(&str, &[u8])>) -> Self {
        let mut modules = Self::default();

        // The lowest mapping of each path is where that file was loaded. A
        // library is mapped several times, once per segment, and only the
        // first is at file offset zero.
        let mut lowest: BTreeMap<&str, (u64, u64)> = BTreeMap::new();
        for mapping in &dump.mappings {
            let entry = lowest.entry(mapping.path.as_str()).or_insert((mapping.start, mapping.end));
            entry.0 = entry.0.min(mapping.start);
            entry.1 = entry.1.max(mapping.end);
        }

        for (path, (base, end)) in lowest {
            // Anonymous and special mappings are not files.
            if path.is_empty() || path.starts_with('[') {
                continue;
            }

            let data = match substitute {
                Some((name, bytes)) if path.ends_with(name) || name.ends_with(path) => {
                    bytes.to_vec()
                }
                _ => match std::fs::read(path) {
                    Ok(data) => data,
                    Err(error) => {
                        modules.unavailable.push(Unavailable {
                            path: path.to_string(),
                            base,
                            because: format!("could not be read: {error}"),
                        });
                        continue;
                    }
                },
            };

            match Self::describe(path, base, end, data) {
                Ok(module) => modules.loaded.push(module),
                Err(because) => {
                    modules.unavailable.push(Unavailable { path: path.to_string(), base, because })
                }
            }
        }

        modules
    }

    /// Extract what unwinding needs from one file.
    fn describe(
        path: &str,
        base: u64,
        end: u64,
        data: Vec<u8>,
    ) -> std::result::Result<Module, String> {
        use object::{Object, ObjectSection, ObjectSegment};

        let file = object::File::parse(&*data)
            .map_err(|error| format!("is not an object file: {error}"))?;

        // Each module's bias is its own: base minus the link-time address of
        // its first loadable segment. Using the executable's bias for a
        // library would place every library frame in the wrong function.
        // The lowest loadable address, with no filtering. An earlier version
        // discarded a zero vaddr — which is exactly what a shared library has —
        // so every library's bias was computed from its *second* segment and
        // every library frame unwound to the wrong place.
        let first_load = file.segments().map(|segment| segment.address()).min().unwrap_or(0);
        let bias = base.saturating_sub(first_load);

        let section = file.section_by_name(".eh_frame");
        let eh_frame = section
            .as_ref()
            .and_then(|section| section.uncompressed_data().ok())
            .map(|data| data.to_vec())
            .unwrap_or_default();
        let eh_frame_address = section.as_ref().map(|section| section.address()).unwrap_or(0);

        if eh_frame.is_empty() {
            return Err("carries no .eh_frame, so frames in it cannot be unwound".into());
        }

        Ok(Module { path: path.to_string(), base, bias, data, eh_frame, eh_frame_address, end })
    }

    /// Which module a runtime address belongs to.
    pub fn containing(&self, address: u64) -> Option<&Module> {
        self.loaded.iter().find(|module| module.contains(address))
    }

    /// Why an address could not be handled, when it is in a module we could
    /// not load.
    pub fn why_unavailable(&self, address: u64) -> Option<&Unavailable> {
        self.unavailable.iter().find(|module| module.base <= address)
    }
}
