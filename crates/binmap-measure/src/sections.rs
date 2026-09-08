//! Reading an artifact's section table.
//!
//! `object` does the container parsing (TOOLING §2.1). It is the foundation
//! the whole binary layer sits on — sections, segments, symbols and program
//! headers behind one API — and it reads Mach-O, PE and wasm besides ELF,
//! which costs nothing now and is what makes `P4`'s platform question
//! answerable later.
//!
//! WASM is not future portability, though: it is a Rust target today, and the
//! corpus has one. The `wasm` feature was off at first, which meant a `.wasm`
//! module reported no sections at all — that is not degrading honestly, it is
//! just not looking.
//!
//! What this module adds on top is the one distinction a size report gets
//! wrong if it does not think about it: whether a section occupies space in
//! the file. `.bss` does not, and a report that counts it is wrong by exactly
//! its size.

use binmap_core::artifact::Section;
use binmap_core::error::{Error, Result};
use object::SectionKind;
use object::read::{Object, ObjectSection};
use std::path::Path;

/// Read the sections of any container `object` understands.
///
/// Returns an empty list rather than an error for something that is not an
/// object file at all — a bundle or a source map is not malformed, it is
/// simply not something this reader has anything to say about, and the total
/// size still stands.
pub fn read(artifact: &Path) -> Result<Vec<Section>> {
    let bytes = std::fs::read(artifact).map_err(|source| Error::io(artifact, source))?;
    Ok(parse(&bytes))
}

pub(crate) fn parse(bytes: &[u8]) -> Vec<Section> {
    let Ok(file) = object::File::parse(bytes) else {
        return Vec::new();
    };

    file.sections()
        .filter_map(|section| {
            let name = section.name().ok()?.to_string();
            if name.is_empty() {
                return None;
            }
            Some(Section {
                bytes: section.size(),
                // `UninitializedData` is `.bss` and its kin: it occupies
                // address space and no file space. `file_range` is `None` for
                // exactly these, which is the authoritative answer rather than
                // a guess from the name.
                occupies_file: section.file_range().is_some()
                    && section.kind() != SectionKind::UninitializedData,
                name,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn something_that_is_not_an_object_file_has_no_sections_rather_than_an_error() {
        assert!(parse(b"this is not an object file, but it is not broken either").is_empty());
        assert!(parse(&[]).is_empty());
    }

    #[test]
    fn our_own_test_binary_has_the_sections_every_elf_has() {
        let path = std::env::current_exe().expect("the test binary exists");
        let sections = read(&path).expect("it is readable");
        let names: Vec<&str> = sections.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&".text"), "{names:?}");
        assert!(names.contains(&".rodata"), "{names:?}");
    }

    #[test]
    fn bss_occupies_address_space_and_not_the_file() {
        let path = std::env::current_exe().unwrap();
        let sections = read(&path).unwrap();
        let bss = sections.iter().find(|s| s.name == ".bss").expect("every Rust binary has one");
        assert!(!bss.occupies_file, ".bss counted against the file would be wrong by its size");
        assert!(bss.bytes > 0, "it still occupies address space");
    }

    #[test]
    fn a_wasm_module_is_read_rather_than_declined() {
        // A complete minimal module: one function of type `() -> ()` whose
        // body is a bare `end`. A bare header with only a type section is not
        // enough — object declines it, which is how this test first failed.
        let module: Vec<u8> = vec![
            0x00, 0x61, 0x73, 0x6d, // \0asm
            0x01, 0x00, 0x00, 0x00, // version 1
            0x01, 0x04, 0x01, 0x60, 0x00, 0x00, // type:     one () -> ()
            0x03, 0x02, 0x01, 0x00, // function: one, of type 0
            0x0a, 0x04, 0x01, 0x02, 0x00, 0x0b, // code:     one body, just `end`
        ];

        let sections = parse(&module);
        let names: Vec<&str> = sections.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["<type>", "<function>", "<code>"], "a WASM module was not read");
        assert!(sections.iter().all(|s| s.occupies_file), "every wasm section is in the file");
    }

    #[test]
    fn nothing_that_occupies_the_file_claims_to_be_bigger_than_the_file() {
        let path = std::env::current_exe().unwrap();
        let sections = read(&path).unwrap();
        let on_disk = std::fs::metadata(&path).unwrap().len();
        let counted: u64 = sections.iter().filter(|s| s.occupies_file).map(|s| s.bytes).sum();
        assert!(counted <= on_disk, "counted {counted} bytes in a {on_disk}-byte file");
    }
}
