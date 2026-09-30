//! Reading an ELF core dump (`F2.1`, `TOOLING-BINARY §1.4`).
//!
//! "No crate does this. `object`'s raw ELF module is the substrate." So this
//! walks program headers itself: `PT_LOAD` segments carry the process's memory,
//! and `PT_NOTE` carries the notes describing the threads and the mappings.
//!
//! Two things here decide whether anything downstream can be trusted:
//!
//! - **The load bias.** Rust binaries are PIE, so every address in the core is
//!   a runtime address and every address in DWARF is a link-time one. Get the
//!   bias wrong and every symbol lookup is confidently wrong. There are two
//!   independent ways to derive it, and `§1.4` says to assert they agree —
//!   this refuses to symbolize when they do not, rather than producing
//!   plausible nonsense.
//! - **Register order.** See `registers.rs`; nothing here indexes a register
//!   array directly.

use crate::registers::Registers;
use binmap_core::error::{Error, Result};
use object::Endianness;
use object::elf;
use object::read::elf::{ElfFile64, FileHeader, ProgramHeader};
use serde::{Deserialize, Serialize};

/// `NT_FILE`, which is ASCII `"FILE"` and is not always in `object`'s
/// constants.
const NT_FILE: u32 = 0x4649_4c45;

/// One mapped file, as `NT_FILE` recorded it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mapping {
    pub path: String,
    pub start: u64,
    pub end: u64,
    /// Offset within the file this mapping begins at. Zero for the first
    /// mapping of an executable, which is how the bias is derived.
    pub file_offset: u64,
}

impl Mapping {
    pub fn contains(&self, address: u64) -> bool {
        (self.start..self.end).contains(&address)
    }
}

/// One thread, as the core recorded it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Thread {
    pub pid: i32,
    /// The signal that stopped it, where the note recorded one.
    pub signal: i32,
    pub registers: Registers,
}

/// A region of the process's memory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Segment {
    pub virtual_address: u64,
    /// Where in the core file the contents are.
    pub file_offset: u64,
    /// How much of it the core actually holds.
    ///
    /// Frequently less than `memory_size`: a core omits pages that were never
    /// written, and reading past this is reading the core file's *next*
    /// segment while believing it is this one's memory.
    pub file_size: u64,
    pub memory_size: u64,
}

impl Segment {
    pub fn contains(&self, address: u64) -> bool {
        (self.virtual_address..self.virtual_address + self.file_size).contains(&address)
    }
}

/// Everything a core dump says.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoreDump {
    /// The crashing thread first, as the kernel writes them.
    pub threads: Vec<Thread>,
    pub mappings: Vec<Mapping>,
    pub segments: Vec<Segment>,
    /// The executable's name, from `NT_PRPSINFO`.
    pub executable: Option<String>,
    /// `AT_PHDR` from the auxiliary vector, for the bias cross-check.
    pub auxv_phdr: Option<u64>,
    /// `AT_ENTRY`, likewise.
    pub auxv_entry: Option<u64>,
}

impl CoreDump {
    /// Parse a core dump.
    pub fn parse(data: &[u8]) -> Result<Self> {
        let file = ElfFile64::<Endianness>::parse(data)
            .map_err(|error| Error::Other(format!("this is not an ELF file: {error}")))?;
        let endian = file.endian();

        if file.elf_header().e_type(endian) != elf::ET_CORE {
            return Err(Error::Other(
                "this is an ELF file but not a core dump. A core is written by the kernel when a \
                 process dies; a binary is not one, and reading it as one would produce threads \
                 and registers that do not exist."
                    .into(),
            ));
        }

        let mut dump = Self {
            threads: Vec::new(),
            mappings: Vec::new(),
            segments: Vec::new(),
            executable: None,
            auxv_phdr: None,
            auxv_entry: None,
        };

        for header in file.elf_program_headers() {
            match header.p_type(endian) {
                elf::PT_LOAD => dump.segments.push(Segment {
                    virtual_address: header.p_vaddr(endian),
                    file_offset: header.p_offset(endian),
                    file_size: header.p_filesz(endian),
                    memory_size: header.p_memsz(endian),
                }),
                elf::PT_NOTE => {
                    let Ok(Some(mut notes)) = header.notes(endian, data) else { continue };
                    while let Ok(Some(note)) = notes.next() {
                        if note.name() != b"CORE" {
                            continue;
                        }
                        // `NT_FILE` has no constant in `object`, so the
                        // comparison is against the wrapper's inner value and
                        // the named constants are unwrapped alongside it.
                        match note.n_type(endian).0 {
                            x if x == elf::NT_PRSTATUS.0 => {
                                if let Some(thread) = parse_prstatus(note.desc()) {
                                    dump.threads.push(thread);
                                }
                            }
                            x if x == elf::NT_PRPSINFO.0 => {
                                dump.executable = parse_prpsinfo(note.desc());
                            }
                            x if x == elf::NT_AUXV.0 => {
                                let (phdr, entry) = parse_auxv(note.desc());
                                dump.auxv_phdr = phdr;
                                dump.auxv_entry = entry;
                            }
                            NT_FILE => dump.mappings.extend(parse_nt_file(note.desc())),
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }

        if dump.threads.is_empty() {
            return Err(Error::Other(
                "this core has no thread state in it, so there is nothing to unwind. A truncated \
                 core is the usual cause — check `ulimit -c` and the size of the file."
                    .into(),
            ));
        }
        Ok(dump)
    }

    /// The thread that crashed.
    ///
    /// The kernel writes it first, which is the only ordering guarantee there
    /// is; there is no flag saying which one faulted.
    pub fn crashing_thread(&self) -> &Thread {
        self.threads.first().expect("parse refuses a core with no threads")
    }

    /// Read memory at a runtime address.
    ///
    /// Bounded by `file_size` rather than `memory_size`, because a core omits
    /// pages that were never written and reading past the end of a segment's
    /// stored bytes returns the *next* segment's contents while claiming they
    /// are this one's.
    pub fn read<'a>(&self, data: &'a [u8], address: u64, length: usize) -> Option<&'a [u8]> {
        let segment = self.segments.iter().find(|segment| segment.contains(address))?;
        let offset = (address - segment.virtual_address) + segment.file_offset;
        let available = segment.file_size - (address - segment.virtual_address);
        if (length as u64) > available {
            return None;
        }
        data.get(offset as usize..offset as usize + length)
    }

    /// Read a `u64` at a runtime address, little-endian.
    pub fn read_u64(&self, data: &[u8], address: u64) -> Option<u64> {
        self.read(data, address, 8)
            .map(|bytes| u64::from_le_bytes(bytes.try_into().expect("read(8)")))
    }

    /// The mapping the executable was loaded at.
    ///
    /// The first mapping whose file offset is zero and whose path matches the
    /// executable's — the executable is mapped several times, once per
    /// segment, and only the first is at offset zero.
    pub fn executable_mapping(&self) -> Option<&Mapping> {
        let name = self.executable.as_deref();
        self.mappings.iter().filter(|mapping| mapping.file_offset == 0).find(|mapping| match name {
            Some(name) => {
                mapping.path.rsplit('/').next() == Some(name) || mapping.path.ends_with(name)
            }
            // With no name recorded, the lowest zero-offset mapping is the
            // executable in every layout the kernel produces.
            None => true,
        })
    }
}

/// One `NT_PRSTATUS` note: a thread's registers and the signal that stopped it.
fn parse_prstatus(descriptor: &[u8]) -> Option<Thread> {
    // From the field table in `§1.4`: `pr_cursig` is at 12, `pr_pid` at 32.
    let signal = descriptor
        .get(12..14)
        .map(|bytes| i16::from_le_bytes(bytes.try_into().expect("2 bytes")) as i32)?;
    let pid = descriptor
        .get(32..36)
        .map(|bytes| i32::from_le_bytes(bytes.try_into().expect("4 bytes")))?;
    let registers = Registers::from_prstatus(descriptor)?;
    Some(Thread { pid, signal, registers })
}

/// `NT_PRPSINFO`, for the executable's name.
///
/// `pr_fname` is a 16-byte NUL-padded field at offset 40 on x86-64. It is the
/// *short* name — the kernel truncates it — which is why the mapping match
/// accepts a suffix.
fn parse_prpsinfo(descriptor: &[u8]) -> Option<String> {
    const PR_FNAME_OFFSET: usize = 40;
    const PR_FNAME_LEN: usize = 16;

    let field = descriptor.get(PR_FNAME_OFFSET..PR_FNAME_OFFSET + PR_FNAME_LEN)?;
    let end = field.iter().position(|byte| *byte == 0).unwrap_or(field.len());
    let name = String::from_utf8_lossy(&field[..end]).trim().to_string();
    (!name.is_empty()).then_some(name)
}

/// The auxiliary vector: pairs of `(type, value)` terminated by `AT_NULL`.
fn parse_auxv(descriptor: &[u8]) -> (Option<u64>, Option<u64>) {
    const AT_NULL: u64 = 0;
    const AT_PHDR: u64 = 3;
    const AT_ENTRY: u64 = 9;

    let mut phdr = None;
    let mut entry = None;
    for pair in descriptor.as_chunks::<16>().0 {
        let kind = u64::from_le_bytes(pair[..8].try_into().expect("8 bytes"));
        let value = u64::from_le_bytes(pair[8..].try_into().expect("8 bytes"));
        match kind {
            AT_NULL => break,
            AT_PHDR => phdr = Some(value),
            AT_ENTRY => entry = Some(value),
            _ => {}
        }
    }
    (phdr, entry)
}

/// `NT_FILE`: `count`, `page_size`, `count` triples, then `count` paths.
///
/// The layout is why the paths cannot be read first: they are variable-length
/// and come *after* all the fixed-size triples.
fn parse_nt_file(descriptor: &[u8]) -> Vec<Mapping> {
    let Some(count) = descriptor.get(..8).map(|b| u64::from_le_bytes(b.try_into().unwrap())) else {
        return Vec::new();
    };
    let Some(page_size) = descriptor.get(8..16).map(|b| u64::from_le_bytes(b.try_into().unwrap()))
    else {
        return Vec::new();
    };

    // A malformed count would ask for a gigantic allocation from a file we do
    // not control.
    let count = count.min(65_536) as usize;
    let triples_end = 16 + count * 24;
    let Some(triples) = descriptor.get(16..triples_end) else { return Vec::new() };
    let Some(paths) = descriptor.get(triples_end..) else { return Vec::new() };

    let mut names = paths.split(|byte| *byte == 0);
    triples
        .as_chunks::<24>()
        .0
        .iter()
        .filter_map(|triple| {
            let read =
                |at: usize| u64::from_le_bytes(triple[at..at + 8].try_into().expect("8 bytes"));
            let path = String::from_utf8_lossy(names.next()?).to_string();
            Some(Mapping {
                path,
                start: read(0),
                end: read(8),
                // Recorded in pages, not bytes. Multiplying is not optional:
                // an unscaled offset makes every non-first mapping look like
                // it starts at the beginning of its file.
                file_offset: read(16) * page_size,
            })
        })
        .collect()
}
