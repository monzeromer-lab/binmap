//! Whether a project contains `unsafe`, and whether it calls across the FFI
//! boundary.
//!
//! Two gate behaviours depend on this and neither worked, because nothing ever
//! set the flags: `MiriClean` reported "the change does not touch unsafe" for
//! every project including ones full of it, and the FFI caveat — the sentence
//! §6 says the interface must show, because a clean sanitizer run over heavy
//! foreign calls says far less than it sounds — could never appear.
//!
//! This is a text scan, and it is deliberately the coarse kind. A precise
//! answer needs the compiler, and the question here is not "where exactly is
//! the unsafe" but "should the sanitizer gate run at all" — for which
//! over-reporting costs a Miri run and under-reporting costs a false sense of
//! safety. It errs toward running.

use std::path::Path;

/// What a scan of the project's own sources found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Unsafety {
    /// `unsafe` blocks, functions, traits or impls appear in the project.
    pub present: bool,
    /// How many files mentioned it, for the environment panel's copy.
    pub files: usize,
    /// `extern "C"`, `#[link]` or a `-sys` style binding appears.
    ///
    /// Miri cannot cross this boundary, so a clean run over a project with
    /// substantial foreign calls is a much weaker statement than it sounds.
    pub foreign_calls: bool,
}

impl Unsafety {
    /// Scan a project's `src` directories.
    ///
    /// Only the project's own sources — not `target/`, not vendored
    /// dependencies. The question is whether *this* change needs the
    /// sanitizer, and every Rust program depends transitively on `unsafe`
    /// somewhere, so counting dependencies would make the answer always yes
    /// and therefore useless.
    pub fn scan(root: &Path) -> Self {
        let mut found = Unsafety::default();
        walk(root, 0, &mut found);
        found
    }
}

/// Directories whose contents are not the project's own source.
fn is_skippable(name: &str) -> bool {
    matches!(name, "target" | ".git" | "node_modules" | "vendor" | ".cargo")
}

fn walk(directory: &Path, depth: usize, found: &mut Unsafety) {
    // Deep enough for a workspace of workspaces, shallow enough not to walk a
    // home directory if someone points us at one.
    if depth > 8 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(directory) else { return };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();

        if path.is_dir() {
            if !is_skippable(&name) {
                walk(&path, depth + 1, found);
            }
            continue;
        }

        if path.extension().is_none_or(|extension| extension != "rs") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else { continue };

        if mentions_unsafe(&text) {
            found.present = true;
            found.files += 1;
        }
        if mentions_foreign_calls(&text) {
            found.foreign_calls = true;
        }
    }
}

/// Whether the text contains `unsafe` as code rather than as prose.
///
/// A doc comment explaining why something is *not* unsafe is extremely common
/// in careful code — this file has several — and counting those would make
/// every well-documented crate look like it needed Miri.
fn mentions_unsafe(text: &str) -> bool {
    text.lines().filter(|line| !is_comment(line)).any(|line| {
        line.split(|c: char| !c.is_ascii_alphanumeric() && c != '_').any(|word| word == "unsafe")
    })
}

fn mentions_foreign_calls(text: &str) -> bool {
    text.lines().filter(|line| !is_comment(line)).any(|line| {
        line.contains("extern \"C\"")
            || line.contains("extern \"system\"")
            || line.trim_start().starts_with("#[link")
    })
}

fn is_comment(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('*')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan_text(text: &str) -> Unsafety {
        let directory = tempfile::tempdir().expect("a temporary directory");
        std::fs::write(directory.path().join("lib.rs"), text).unwrap();
        Unsafety::scan(directory.path())
    }

    #[test]
    fn code_that_uses_unsafe_is_found() {
        let found = scan_text("fn f() {\n    unsafe { *p }\n}\n");
        assert!(found.present);
        assert_eq!(found.files, 1);
    }

    #[test]
    fn prose_about_unsafe_is_not_code_that_uses_it() {
        // A doc comment explaining why something is *not* unsafe is extremely
        // common in careful code. Counting those would make every
        // well-documented crate look like it needed Miri.
        let found = scan_text(
            "//! This module contains no unsafe code.\n\
             /// SAFETY: not applicable, there is no unsafe here.\n\
             // unsafe would be wrong here\n\
             fn f() -> u8 { 0 }\n",
        );
        assert!(!found.present, "a comment was read as code");
    }

    #[test]
    fn a_word_containing_unsafe_is_not_the_keyword() {
        let found = scan_text("fn unsafely_named() {}\nstruct Unsafe;\nlet unsafe_count = 1;\n");
        assert!(!found.present, "`{}` matched a substring", "unsafely_named");
    }

    #[test]
    fn foreign_calls_are_noticed_because_miri_cannot_cross_them() {
        let found = scan_text("unsafe extern \"C\" {\n    fn getpid() -> i32;\n}\n");
        assert!(found.present);
        assert!(found.foreign_calls, "a clean Miri run here says less than it sounds");

        let pure = scan_text("fn f() {\n    unsafe { *p }\n}\n");
        assert!(!pure.foreign_calls);
    }

    #[test]
    fn the_scan_ignores_build_output_and_dependencies() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        for skipped in ["target", "vendor", "node_modules", ".git"] {
            let nested = directory.path().join(skipped);
            std::fs::create_dir_all(&nested).unwrap();
            std::fs::write(nested.join("dep.rs"), "unsafe { }\n").unwrap();
        }
        std::fs::write(directory.path().join("main.rs"), "fn main() {}\n").unwrap();

        let found = Unsafety::scan(directory.path());
        assert!(!found.present, "a dependency's unsafe was counted as the project's");
    }

    #[test]
    fn the_corpus_stress_crate_is_found_to_use_unsafe() {
        // It has one block, on purpose, so the sanitizer gate has something to
        // run against.
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .unwrap()
            .join("corpus/stress");
        if !root.exists() {
            return;
        }
        let found = Unsafety::scan(&root);
        assert!(found.present, "corpus/stress has an unsafe block");
        assert!(!found.foreign_calls, "and no foreign calls");
    }
}
