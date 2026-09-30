//! Where the bytes came from (`F1.2`), which generic they came from (`F1.3`),
//! and what kind of cost they represent (`F1.4`).
//!
//! This is the part no general-purpose tool does, and the reason the plan says
//! to own it rather than parse `bloaty`'s output: attributing bytes to a
//! *generic origin* across instantiations needs the structure v0 mangling
//! carries, and re-deriving that structure from a formatted table is worse
//! than reading it from the name.

use crate::symbols::{Mangling, Symbol, SymbolTable};
use std::collections::BTreeMap;

use binmap_core::attribution::{Attribution, Driver, Group, Monomorphization};
use serde::{Deserialize, Serialize};

/// Which crate a symbol belongs to, and the path within it.
/// Which crate a symbol belongs to, and the path within it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Origin {
    /// The crate, as the demangled path names it.
    pub crate_name: String,
    /// The module path within that crate, without the final item.
    pub module: String,
    /// The item, with generic arguments removed — the grouping key for
    /// monomorphization.
    pub generic_path: String,
    /// The generic arguments, where the name carried them. Only v0 does.
    pub arguments: Option<String>,
}

/// Split a demangled path into where it came from.
///
/// This is text work on a structured input, and the structure is the point: v0
/// names carry `<...>` around their generic arguments, so the same generic at
/// two type arguments shares everything before the `<`. That prefix is the
/// grouping key `F1.3` needs, and legacy names do not have it — which is why
/// the interface reports the mangling scheme.
pub fn split(name: &str) -> Origin {
    // Find the generic arguments, balancing brackets so a nested `<..<..>..>`
    // is not cut in the middle.
    let (path, arguments) = match find_arguments(name) {
        Some((at, end)) => (&name[..at], Some(name[at..end].to_string())),
        None => (name, None),
    };

    // A turbofish leaves the separator behind: `drop_in_place::<T>` splits at
    // the `<` and the path keeps a trailing `::`, which then shows up in the
    // interface as `core::ptr::drop_in_place::`.
    let path = path.strip_suffix("::").unwrap_or(path);

    // `<Type as Trait>::method` is a qualified self type, not a path starting
    // with a crate called `<Type`. Reading it literally put `<std`, `<core`
    // and `<alloc` in the crate list of every binary — visible the first time
    // this ran against a real one.
    let for_crate = qualified_self_type(path).unwrap_or(path);

    let mut segments: Vec<&str> =
        for_crate.split("::").filter(|segment| !segment.is_empty()).collect();
    let crate_name = segments.first().copied().unwrap_or("").to_string();
    // The item is the last segment; everything between is the module path.
    let _item = segments.pop().unwrap_or_default();
    let module = if segments.len() > 1 { segments[1..].join("::") } else { String::new() };

    Origin { crate_name, module, generic_path: path.to_string(), arguments }
}

/// The self type of a `<Type as Trait>::method` path.
///
/// `None` when the path is not qualified. The crate a qualified method belongs
/// to is the crate of its *self type* — `<std::path::PathBuf as Debug>::fmt`
/// is std's code, not `core::fmt`'s, and certainly not a crate called
/// `<std`.
fn qualified_self_type(path: &str) -> Option<&str> {
    let rest = path.strip_prefix('<')?;

    // Balance to the matching `>`, so `<Vec<u8> as Trait>` is not cut early.
    let bytes = rest.as_bytes();
    let mut depth = 0usize;
    let mut end = None;
    for (index, byte) in bytes.iter().enumerate() {
        match byte {
            b'<' => depth += 1,
            b'>' if depth == 0 => {
                end = Some(index);
                break;
            }
            b'>' => depth -= 1,
            _ => {}
        }
    }
    let inner = &rest[..end?];

    // `Type as Trait` keeps the type; a bare `<Type>` keeps all of it.
    Some(inner.split(" as ").next().unwrap_or(inner).trim())
}

/// The byte range of the outermost `<...>` in a path, if there is one.
fn find_arguments(name: &str) -> Option<(usize, usize)> {
    let bytes = name.as_bytes();
    let mut depth = 0usize;
    let mut start = None;

    for (index, byte) in bytes.iter().enumerate() {
        match byte {
            b'<' => {
                // `<T as Trait>` at the front of a path is a qualified self
                // type, not a generic argument list, so it is not the split
                // point.
                if depth == 0 && index > 0 {
                    start = Some(index);
                }
                depth += 1;
            }
            b'>' => {
                depth = depth.saturating_sub(1);
                if depth == 0
                    && let Some(at) = start
                {
                    return Some((at, index + 1));
                }
            }
            _ => {}
        }
    }
    None
}

/// Classify a symbol by what its bytes are spent on.
///
/// Deliberately conservative: a symbol that matches nothing specific is
/// attributed to its crate rather than guessed into a category, because a
/// wrong category is worse than a vague one — it sends the user to change
/// something that was never the problem.
pub fn classify(symbol: &Symbol, own_crates: &[String]) -> Driver {
    let name = &symbol.name;

    // Section first: data is data whatever its name says.
    if symbol.section.starts_with(".eh_frame") || symbol.section.starts_with(".gcc_except") {
        return Driver::Unwinding;
    }

    // Compiler-generated items, which the name marks explicitly.
    if name.contains("::{{vtable}}") || name.contains("<vtable>") {
        return Driver::Vtable;
    }
    if name.contains("core::ptr::drop_in_place") || name.contains("::{{drop}}") {
        return Driver::DropGlue;
    }

    // Then the standard library's own subsystems, most specific first.
    if name.starts_with("core::fmt") || name.starts_with("alloc::fmt") || name.contains("::fmt::") {
        return Driver::Formatting;
    }
    if name.starts_with("core::panicking")
        || name.starts_with("std::panicking")
        || name.contains("panic_fmt")
        || name.contains("panic_bounds_check")
    {
        return Driver::Panic;
    }
    if name.starts_with("_Unwind") || name.contains("rust_eh_personality") {
        return Driver::Unwinding;
    }

    let origin = split(name);
    if own_crates.iter().any(|own| own == &origin.crate_name) {
        return Driver::Yours;
    }
    if matches!(origin.crate_name.as_str(), "core" | "alloc" | "std" | "proc_macro") {
        return Driver::StandardLibrary;
    }
    if Mangling::of(&symbol.mangled) == Mangling::Foreign {
        // Not a Rust symbol at all. Data sections are data; everything else is
        // runtime support.
        return if symbol.section.starts_with(".rodata")
            || symbol.section.starts_with(".data")
            || symbol.section.starts_with(".bss")
        {
            Driver::StaticData
        } else {
            Driver::Runtime
        };
    }
    if symbol.section.starts_with(".rodata") || symbol.section.starts_with(".data") {
        return Driver::StaticData;
    }

    Driver::Dependency
}

/// Attribute a symbol table.
///
/// `own_crates` is how "your code" is told from "a dependency" — there is no
/// marker in a symbol name for it, and guessing from the crate name would be
/// wrong for anyone whose crate is called `serde`.
pub fn attribute(table: &SymbolTable, own_crates: &[String]) -> Attribution {
    let mut by_crate: BTreeMap<String, (u64, u32, Vec<String>)> = BTreeMap::new();
    let mut by_driver: BTreeMap<Driver, u64> = BTreeMap::new();
    let mut by_generic: BTreeMap<String, (u64, Vec<(String, u64)>)> = BTreeMap::new();

    for symbol in &table.symbols {
        if symbol.size == 0 {
            continue;
        }
        let origin = split(&symbol.name);
        let driver = classify(symbol, own_crates);
        *by_driver.entry(driver).or_default() += symbol.size;

        let entry = by_crate.entry(origin.crate_name.clone()).or_default();
        entry.0 += symbol.size;
        entry.1 += 1;
        if entry.2.len() < 3 {
            entry.2.push(symbol.name.clone());
        }

        // Only names that actually carried arguments group as generics.
        // Without them every non-generic function would look like a generic
        // with one instantiation, which is noise.
        if let Some(arguments) = origin.arguments {
            let generic = by_generic.entry(origin.generic_path).or_default();
            generic.0 += symbol.size;
            generic.1.push((arguments, symbol.size));
        }
    }

    let mut crates: Vec<Group> = by_crate
        .into_iter()
        .map(|(key, (bytes, symbols, examples))| Group { key, bytes, symbols, examples })
        .collect();
    crates.sort_by_key(|group| std::cmp::Reverse(group.bytes));

    let mut drivers: Vec<(Driver, u64)> = by_driver.into_iter().collect();
    drivers.sort_by_key(|(_, bytes)| std::cmp::Reverse(*bytes));

    let mut monomorphizations: Vec<Monomorphization> = by_generic
        .into_iter()
        // One instantiation is not a monomorphization problem.
        .filter(|(_, (_, arguments))| arguments.len() > 1)
        .map(|(generic_path, (total_bytes, mut arguments))| {
            arguments.sort_by_key(|(_, bytes)| std::cmp::Reverse(*bytes));
            Monomorphization {
                generic_path,
                total_bytes,
                instantiations: arguments.len() as u32,
                arguments,
            }
        })
        .collect();
    monomorphizations.sort_by_key(|m| std::cmp::Reverse(m.total_bytes));

    Attribution {
        attributed_bytes: table.total_bytes(),
        crates,
        drivers,
        monomorphizations,
        inferred_fraction: table.inferred_fraction,
        generic_arguments_available: table.mangling() != Some(Mangling::Legacy),
    }
}
