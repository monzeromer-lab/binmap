//! Reference corpus, entry seven: one crate that touches everything.
//!
//! The other corpus entries each make one claim testable. This one is here to
//! be a small artifact that nonetheless contains every category Binmap knows
//! how to look for, so a single sweep exercises the whole pipeline:
//!
//! | What is here | What it exercises |
//! |---|---|
//! | Generics at many type arguments | Monomorphization grouping (Phase 1) |
//! | `format!` and `Display` | Formatting machinery, the largest usual driver |
//! | Panic messages with payloads | Panic strings, and `panic=abort`'s effect |
//! | Trait objects | Vtables |
//! | Types with `Drop` | Drop glue |
//! | A large static table | `.rodata`, and storage that is not code |
//! | One `unsafe` block | The `MiriClean` gate, which is otherwise skipped |
//! | Arithmetic that wraps | `TestsPass` failing under `overflow-checks` |
//!
//! It is deliberately a few hundred lines. A corpus entry that takes minutes
//! to build is one nobody runs.

use std::collections::BTreeMap;
use std::fmt::{self, Debug, Display};

// ---------------------------------------------------------------------------
// Storage: data that is not code
// ---------------------------------------------------------------------------

/// A lookup table large enough to show up in a size report on its own.
///
/// Four kilobytes of `.rodata`. The point of having it is that a size tool
/// which only attributes `.text` will report this crate as smaller than it is,
/// and that is a wrong answer delivered confidently.
pub static CRC_TABLE: [u32; 1024] = build_table();

const fn build_table() -> [u32; 1024] {
    let mut table = [0u32; 1024];
    let mut index = 0;
    while index < 1024 {
        let mut value = index as u32;
        let mut bit = 0;
        while bit < 8 {
            value = if value & 1 != 0 { (value >> 1) ^ 0xEDB8_8320 } else { value >> 1 };
            bit += 1;
        }
        table[index] = value;
        index += 1;
    }
    table
}

/// Strings, which live in `.rodata` too and are easy to forget about.
pub static MESSAGES: [&str; 8] = [
    "the configuration was measured and recorded",
    "the candidate did not build",
    "the suite did not pass",
    "the difference is inside the noise floor",
    "sanitizers are clean over the reachable code",
    "no benchmark is declared, so runtime is not an objective",
    "a near miss is informative and stays in the table",
    "the failing gate is named rather than the candidate dropped",
];

// ---------------------------------------------------------------------------
// Generics: the same code, emitted many times
// ---------------------------------------------------------------------------

/// A summariser, instantiated below at enough type arguments that
/// monomorphization grouping has something real to collapse.
#[derive(Debug, Clone, Default)]
pub struct Tally<T> {
    seen: Vec<T>,
}

// `Debug` rather than `Display`, because that is the bound real code
// reaches for and `core::fmt::Debug` is the larger driver of the two.
impl<T: Clone + Ord + Debug> Tally<T> {
    pub fn new() -> Self {
        Self { seen: Vec::new() }
    }

    pub fn record(&mut self, item: T) -> &mut Self {
        self.seen.push(item);
        self
    }

    pub fn len(&self) -> usize {
        self.seen.len()
    }

    pub fn is_empty(&self) -> bool {
        self.seen.is_empty()
    }

    /// Distinct items, in order.
    pub fn distinct(&self) -> Vec<T> {
        let mut unique: BTreeMap<T, ()> = BTreeMap::new();
        for item in &self.seen {
            unique.insert(item.clone(), ());
        }
        unique.into_keys().collect()
    }

    /// Formatting machinery, reached from every instantiation. This is the
    /// single largest size driver in most Rust binaries and it is here on
    /// purpose.
    pub fn describe(&self) -> String {
        let mut out = String::new();
        for (index, item) in self.distinct().iter().enumerate() {
            if index > 0 {
                out.push_str(", ");
            }
            out.push_str(&format!("{item:?}"));
        }
        format!("[{out}]")
    }

    /// Panics with a formatted payload. Panic strings are their own category,
    /// and `panic = "abort"` removes the unwinding tables that support them.
    pub fn nth(&self, index: usize) -> &T {
        assert!(
            index < self.seen.len(),
            "item {index} of a {}-item tally: {}",
            self.seen.len(),
            self.describe()
        );
        &self.seen[index]
    }
}

// ---------------------------------------------------------------------------
// Vtables and Drop glue
// ---------------------------------------------------------------------------

/// A trait used as an object, so the binary carries vtables for it.
pub trait Stage {
    fn name(&self) -> &str;
    fn cost(&self) -> u64;
}

/// Carries a `Drop`, so the binary carries glue for it.
pub struct Measured {
    label: String,
    bytes: u64,
}

impl Measured {
    pub fn new(label: impl Into<String>, bytes: u64) -> Self {
        Self { label: label.into(), bytes }
    }
}

impl Stage for Measured {
    fn name(&self) -> &str {
        &self.label
    }

    fn cost(&self) -> u64 {
        self.bytes
    }
}

impl Drop for Measured {
    fn drop(&mut self) {
        // Observable, so the optimiser cannot delete the glue entirely.
        if self.bytes == u64::MAX {
            println!("impossible: {}", self.label);
        }
    }
}

impl Display for Measured {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({} bytes)", self.label, self.bytes)
    }
}

/// Dispatch through the vtable, so it is not dead.
pub fn total_cost(stages: &[Box<dyn Stage>]) -> u64 {
    stages.iter().map(|stage| stage.cost()).sum()
}

// ---------------------------------------------------------------------------
// unsafe: the MiriClean gate is skipped without it
// ---------------------------------------------------------------------------

/// Reinterpret a `u32` slice as bytes, to give the sanitizer gate something to
/// check.
///
/// Sound: `u32` has no padding and no invalid bit patterns, the lifetime is
/// tied to the input, and the length is scaled rather than assumed.
pub fn table_bytes() -> &'static [u8] {
    // SAFETY: `CRC_TABLE` is `'static`, `u32` is plain old data with no
    // padding, and the byte length is computed from the element count rather
    // than guessed.
    unsafe {
        std::slice::from_raw_parts(
            CRC_TABLE.as_ptr() as *const u8,
            std::mem::size_of_val(&CRC_TABLE),
        )
    }
}

// ---------------------------------------------------------------------------
// Arithmetic that wraps: TestsPass fails under overflow-checks
// ---------------------------------------------------------------------------

/// A checksum that adds without wrapping.
///
/// Under `overflow-checks = false` — the release default — this wraps
/// silently. Under `true` it panics and the suite fails. The value of a
/// configuration sweep is that it tells you which of those you are shipping.
pub fn checksum(bytes: &[u8]) -> u8 {
    let mut total: u8 = 0;
    for byte in bytes {
        total = total + *byte;
    }
    total
}

/// Everything above, reached from one call so none of it is dead code.
pub fn exercise() -> String {
    let mut numbers: Tally<u32> = Tally::new();
    numbers.record(3).record(1).record(3);

    let mut words: Tally<String> = Tally::new();
    for message in MESSAGES.iter().take(3) {
        words.record((*message).to_string());
    }

    let mut pairs: Tally<(u8, char)> = Tally::new();
    pairs.record((1, 'a')).record((2, 'b'));

    let mut nested: Tally<Vec<i64>> = Tally::new();
    nested.record(vec![1, 2, 3]).record(vec![4]);

    let mut signed: Tally<i128> = Tally::new();
    signed.record(-1).record(i128::MAX);

    let stages: Vec<Box<dyn Stage>> = vec![
        Box::new(Measured::new("build", 1024)),
        Box::new(Measured::new("measure", 2048)),
    ];

    format!(
        "{} {} {} {} {} · {} bytes over {} stages · crc {} over {} table bytes",
        numbers.describe(),
        words.len(),
        pairs.describe(),
        nested.len(),
        signed.len(),
        total_cost(&stages),
        stages.len(),
        checksum(&table_bytes()[..64]),
        table_bytes().len(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_built_at_compile_time_and_is_not_all_zeroes() {
        assert_eq!(CRC_TABLE.len(), 1024);
        assert_ne!(CRC_TABLE[1], 0);
        assert_eq!(table_bytes().len(), 4096, "four kilobytes of .rodata");
    }

    #[test]
    fn every_instantiation_is_reachable() {
        let described = exercise();
        assert!(described.contains("stages"), "{described}");
        assert!(described.contains("4096"), "{described}");
    }

    #[test]
    fn distinct_keeps_one_of_each_in_order() {
        let mut tally: Tally<u32> = Tally::new();
        tally.record(3).record(1).record(3);
        assert_eq!(tally.distinct(), vec![1, 3]);
        assert_eq!(tally.describe(), "[1, 3]");
    }

    #[test]
    #[should_panic(expected = "item 9 of a 2-item tally")]
    fn indexing_past_the_end_panics_with_a_formatted_payload() {
        let mut tally: Tally<u32> = Tally::new();
        tally.record(1).record(2);
        let _ = tally.nth(9);
    }

    #[test]
    fn vtable_dispatch_reaches_every_stage() {
        let stages: Vec<Box<dyn Stage>> =
            vec![Box::new(Measured::new("a", 10)), Box::new(Measured::new("b", 32))];
        assert_eq!(total_cost(&stages), 42);
        assert_eq!(stages[0].name(), "a");
    }

    /// The test that makes a sweep reject something.
    ///
    /// It asserts the wrapped value, so it passes under
    /// `overflow-checks = false` and panics under `true`. A sweep over this
    /// crate must reject every overflow-checked configuration on TestsPass.
    #[test]
    fn the_checksum_wraps_when_overflow_checks_are_off() {
        // 200 + 100 = 300, which does not fit in a u8. Wrapped: 44.
        assert_eq!(checksum(&[200, 100]), 44);
    }
}
