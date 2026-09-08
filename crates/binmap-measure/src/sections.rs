//! Reading an ELF section table, without a dependency and without binutils.
//!
//! This is deliberately the smallest thing that answers "what is this binary
//! made of" for Phase 0's size reporting. It reads the section header table
//! and nothing else — no symbols, no DWARF, no relocations. Phase 1's
//! `binmap-binary` supersedes it, and cross-checks its numbers against an
//! independent tool to a stated tolerance (`F1.2`).
//!
//! It is native rather than a call to `size -A` because the environment probe
//! should not have to find binutils before the application can report how big
//! a binary is.

use binmap_core::artifact::Section;
use binmap_core::error::{Error, Result};
use std::path::Path;

const MAGIC: [u8; 4] = [0x7f, b'E', b'L', b'F'];
const SHT_NOBITS: u32 = 8;

/// Read the sections of a 64-bit little-endian ELF file.
///
/// Returns an empty list rather than an error for an artifact that is not ELF
/// — a WASM module or a bundle is not malformed, it is simply not something
/// this reader has anything to say about, and the total size still stands.
pub fn read(artifact: &Path) -> Result<Vec<Section>> {
    let bytes = std::fs::read(artifact).map_err(|source| Error::io(artifact, source))?;
    Ok(parse(&bytes))
}

pub(crate) fn parse(bytes: &[u8]) -> Vec<Section> {
    if bytes.len() < 64 || bytes[0..4] != MAGIC {
        return Vec::new();
    }
    // 2 = ELFCLASS64, 1 = ELFDATA2LSB. Linux x86-64 is the platform Phase 0
    // supports, and anything else is reported as "no sections" rather than
    // guessed at.
    if bytes[4] != 2 || bytes[5] != 1 {
        return Vec::new();
    }

    let u16_at = |offset: usize| -> Option<u16> {
        Some(u16::from_le_bytes(bytes.get(offset..offset + 2)?.try_into().ok()?))
    };
    let u32_at = |offset: usize| -> Option<u32> {
        Some(u32::from_le_bytes(bytes.get(offset..offset + 4)?.try_into().ok()?))
    };
    let u64_at = |offset: usize| -> Option<u64> {
        Some(u64::from_le_bytes(bytes.get(offset..offset + 8)?.try_into().ok()?))
    };

    let Some(table_offset) = u64_at(0x28).map(|v| v as usize) else { return Vec::new() };
    let (Some(entry_size), Some(count), Some(name_index)) =
        (u16_at(0x3a), u16_at(0x3c), u16_at(0x3e))
    else {
        return Vec::new();
    };
    if table_offset == 0 || entry_size < 64 || count == 0 {
        return Vec::new();
    }

    // The section holding the section names, so the table can be labelled.
    let names_header = table_offset + name_index as usize * entry_size as usize;
    let names_offset = u64_at(names_header + 0x18).unwrap_or(0) as usize;
    let names_size = u64_at(names_header + 0x20).unwrap_or(0) as usize;
    let names = bytes.get(names_offset..names_offset.saturating_add(names_size)).unwrap_or(&[]);

    let mut sections = Vec::with_capacity(count as usize);
    for index in 0..count as usize {
        let header = table_offset + index * entry_size as usize;
        let (Some(name_offset), Some(kind), Some(size)) =
            (u32_at(header), u32_at(header + 0x04), u64_at(header + 0x20))
        else {
            break;
        };
        let name = read_c_string(names, name_offset as usize);
        // The null section at index zero labels nothing and weighs nothing.
        if index == 0 && name.is_empty() {
            continue;
        }
        sections.push(Section {
            name,
            bytes: size,
            // `.bss` is declared, not stored. A size report that counts it is
            // wrong by exactly its size.
            occupies_file: kind != SHT_NOBITS,
        });
    }
    sections
}

fn read_c_string(table: &[u8], offset: usize) -> String {
    let Some(tail) = table.get(offset..) else { return String::new() };
    let end = tail.iter().position(|&byte| byte == 0).unwrap_or(tail.len());
    String::from_utf8_lossy(&tail[..end]).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn something_that_is_not_elf_has_no_sections_rather_than_an_error() {
        assert!(parse(b"MZ\x90\x00 this is not an ELF file at all, but it is not broken").is_empty());
        assert!(parse(&[]).is_empty());
    }

    #[test]
    fn a_32_bit_or_big_endian_elf_is_declined_rather_than_guessed_at() {
        let mut header = vec![0u8; 64];
        header[0..4].copy_from_slice(&MAGIC);
        header[4] = 1; // ELFCLASS32
        header[5] = 1;
        assert!(parse(&header).is_empty());
    }

    #[test]
    fn our_own_test_binary_has_the_sections_every_elf_has() {
        let path = std::env::current_exe().expect("the test binary exists");
        let sections = read(&path).expect("it is readable");
        let names: Vec<&str> = sections.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&".text"), "{names:?}");
        assert!(names.contains(&".rodata"), "{names:?}");

        // `.bss` occupies address space but not the file, and is marked so.
        if let Some(bss) = sections.iter().find(|s| s.name == ".bss") {
            assert!(!bss.occupies_file);
        }

        // Nothing that occupies the file claims to be bigger than the file.
        let on_disk = std::fs::metadata(&path).unwrap().len();
        let counted: u64 = sections.iter().filter(|s| s.occupies_file).map(|s| s.bytes).sum();
        assert!(counted <= on_disk, "counted {counted} bytes in a {on_disk}-byte file");
    }
}
