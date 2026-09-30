//! The PIE load bias, derived twice (`F2.2`, `TOOLING-BINARY §1.4`).
//!
//! Rust binaries are position-independent by default, so **every address in a
//! core is a runtime address and every address in DWARF is a link-time one.**
//! The difference is the load bias, and getting it wrong does not produce an
//! error — it produces a symbol, a file and a line number, all confidently
//! wrong. That is the single worst failure available to a debugger, and it is
//! why this module exists rather than a one-line subtraction at the call site.
//!
//! `§1.4` gives two independent derivations and says to assert they agree:
//!
//! 1. From `NT_FILE`: the runtime start of the executable's first mapping,
//!    minus the link-time vaddr of its first `PT_LOAD`.
//! 2. From `AT_PHDR` in the auxiliary vector: the runtime address of the
//!    program header table, minus its link-time address.
//!
//! When they disagree we refuse. A core that has been edited, a binary that is
//! not the one that ran, a layout nobody anticipated — the cause does not
//! matter, because in every case the honest output is "I cannot place these
//! addresses" rather than a stack trace nobody should trust.

use crate::dump::CoreDump;
use binmap_core::error::{Error, Result};
use object::Endianness;
use object::elf;
use object::read::elf::{ElfFile64, FileHeader, ProgramHeader};
use serde::{Deserialize, Serialize};

/// How the bias was arrived at.
///
/// Carried so the interface can say which derivations agreed, which is the
/// difference between a number a user can trust and one they cannot check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Derivation {
    /// Both derivations agreed. The only case that may be used silently.
    Corroborated,
    /// Only `NT_FILE` was available — the core carried no `AT_PHDR`.
    FromMappingsAlone,
    /// Only `AT_PHDR` was available, which happens when `NT_FILE` is absent.
    FromAuxvAlone,
}

impl Derivation {
    /// Whether this needs saying out loud beside every symbolized frame.
    pub fn needs_a_caveat(self) -> bool {
        !matches!(self, Derivation::Corroborated)
    }

    pub fn describe(self) -> &'static str {
        match self {
            Derivation::Corroborated => {
                "The load bias was derived two independent ways and they agree, so runtime \
                 addresses map onto the binary's own."
            }
            Derivation::FromMappingsAlone => {
                "The load bias came from the mapped file table alone: this core carries no \
                 auxiliary vector, so there was nothing to cross-check it against."
            }
            Derivation::FromAuxvAlone => {
                "The load bias came from the auxiliary vector alone: this core carries no mapped \
                 file table, so there was nothing to cross-check it against."
            }
        }
    }
}

/// A verified load bias.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoadBias {
    pub bias: u64,
    pub derivation: Derivation,
}

impl LoadBias {
    /// A runtime address, as DWARF would number it.
    ///
    /// `None` when the address is below the bias, which means it is not in the
    /// executable at all — a libc frame, or a wild pointer. Returning `None`
    /// rather than wrapping is deliberate: a wrapped subtraction produces an
    /// enormous address that looks like a plausible lookup failure rather than
    /// a category error.
    pub fn to_link_time(&self, runtime: u64) -> Option<u64> {
        runtime.checked_sub(self.bias)
    }

    /// The inverse, for putting a DWARF address back where it ran.
    pub fn to_runtime(&self, link_time: u64) -> Option<u64> {
        link_time.checked_add(self.bias)
    }
}

/// Derive the bias, or refuse.
///
/// `executable` is the binary's bytes — the one believed to correspond to the
/// core.
pub fn derive(dump: &CoreDump, executable: &[u8]) -> Result<LoadBias> {
    let file = ElfFile64::<Endianness>::parse(executable)
        .map_err(|error| Error::Other(format!("the executable is not an ELF file: {error}")))?;
    let endian = file.endian();

    // The link-time vaddr of the first PT_LOAD. For a PIE this is usually 0,
    // but assuming so is how a non-PIE binary gets a bias equal to its own
    // load address and every lookup lands in the wrong place.
    let first_load = file
        .elf_program_headers()
        .iter()
        .filter(|header| header.p_type(endian) == elf::PT_LOAD)
        .map(|header| header.p_vaddr(endian))
        .min();

    let from_mappings = match (dump.executable_mapping(), first_load) {
        (Some(mapping), Some(vaddr)) => mapping.start.checked_sub(vaddr),
        _ => None,
    };

    // AT_PHDR is the runtime address of the program header table. Its
    // link-time address is `e_phoff` interpreted as a vaddr, which holds
    // because the headers live inside the first PT_LOAD at the same offset.
    let from_auxv =
        dump.auxv_phdr.and_then(|phdr| phdr.checked_sub(file.elf_header().e_phoff(endian)));

    match (from_mappings, from_auxv) {
        (Some(mappings), Some(auxv)) if mappings == auxv => {
            Ok(LoadBias { bias: mappings, derivation: Derivation::Corroborated })
        }
        (Some(mappings), Some(auxv)) => Err(Error::Other(format!(
            "the load bias cannot be established: the mapped file table says {mappings:#x} and \
             the auxiliary vector says {auxv:#x}. Symbolizing with either would give a stack \
             trace that looks right and is not, so this refuses instead. The usual cause is that \
             the binary is not the one that produced this core."
        ))),
        (Some(mappings), None) => {
            Ok(LoadBias { bias: mappings, derivation: Derivation::FromMappingsAlone })
        }
        (None, Some(auxv)) => Ok(LoadBias { bias: auxv, derivation: Derivation::FromAuxvAlone }),
        (None, None) => Err(Error::Other(
            "the load bias cannot be established: this core carries neither a mapped file table \
             nor an auxiliary vector, so there is nothing to place its addresses against."
                .into(),
        )),
    }
}
