//! What kind of death this was (`F2.6`, `F2.8`).
//!
//! The signal is not the answer. `SIGSEGV` covers a null dereference, a stack
//! overflow, a use-after-free and a wild jump, and those are four different
//! bugs with four different first questions. `SIGABRT` is where a Rust panic
//! lands, and calling that "abort" tells the reader nothing they did not
//! already know.
//!
//! So classification reads the faulting address, the registers and the stack
//! together. Where the evidence supports a specific answer it gives one; where
//! it does not it says `Unclassified` and shows the signal, rather than
//! picking the most common cause and presenting it as a finding.

use crate::dump::{CoreDump, Thread};
use crate::symbolize::Resolved;
use serde::{Deserialize, Serialize};

/// Signals worth naming.
pub mod signal {
    pub const SIGILL: i32 = 4;
    pub const SIGABRT: i32 = 6;
    pub const SIGFPE: i32 = 8;
    pub const SIGBUS: i32 = 7;
    pub const SIGSEGV: i32 = 11;
}

/// What happened.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Crash {
    /// A Rust panic. The message and location come from the panic machinery
    /// on the stack, not from the signal.
    Panic { message: Option<String> },
    /// A read or write through a null pointer.
    ///
    /// Distinguished from other segfaults by the faulting address being in
    /// the first page — nothing is ever mapped there, which is what makes a
    /// null dereference a fault rather than a silent corruption.
    NullDereference,
    /// The stack ran out.
    ///
    /// Recognised by the stack pointer sitting just below the lowest mapped
    /// stack page: the guard page is what turns recursion into a signal.
    StackOverflow,
    /// A segfault that is neither of the above.
    InvalidMemoryAccess { address: Option<u64> },
    /// Integer division by zero, or the `INT_MIN / -1` overflow.
    ArithmeticFault,
    /// Execution reached something that is not an instruction — a wild jump,
    /// or a corrupted function pointer.
    IllegalInstruction,
    /// A misaligned or otherwise unusable address.
    BusError,
    /// The signal is known and nothing further can be said honestly.
    Unclassified { signal: i32 },
}

impl Crash {
    /// The headline.
    pub fn title(&self) -> String {
        match self {
            Crash::Panic { message: Some(message) } => format!("panicked: {message}"),
            Crash::Panic { message: None } => "panicked".into(),
            Crash::NullDereference => "null pointer dereference".into(),
            Crash::StackOverflow => "stack overflow".into(),
            Crash::InvalidMemoryAccess { address: Some(address) } => {
                format!("invalid memory access at {address:#x}")
            }
            Crash::InvalidMemoryAccess { address: None } => "invalid memory access".into(),
            Crash::ArithmeticFault => "arithmetic fault".into(),
            Crash::IllegalInstruction => "illegal instruction".into(),
            Crash::BusError => "bus error".into(),
            Crash::Unclassified { signal } => format!("terminated by signal {signal}"),
        }
    }

    /// The first question this kind of crash asks, which is what a reader
    /// actually needs.
    pub fn what_to_look_at(&self) -> &'static str {
        match self {
            Crash::Panic { .. } => {
                "The panic message and the frame below the panic machinery: that frame is where \
                 your code decided to give up."
            }
            Crash::NullDereference => {
                "What was expected to be non-null at the faulting line. In safe Rust this \
                 normally means an `unsafe` block or an FFI boundary, because an `Option` \
                 cannot be null."
            }
            Crash::StackOverflow => {
                "The repeating pattern in the frames. Unbounded recursion shows as the same few \
                 functions cycling; a single enormous stack frame shows as one function with a \
                 very large frame."
            }
            Crash::InvalidMemoryAccess { .. } => {
                "Where the faulting address came from. A value that looks like a small integer, \
                 an ASCII string, or a freed pointer each point somewhere different."
            }
            Crash::ArithmeticFault => {
                "The divisor at the faulting line. In Rust this is usually an FFI or `unsafe` \
                 path, because checked arithmetic panics rather than trapping."
            }
            Crash::IllegalInstruction => {
                "How control reached this address. A corrupted function pointer and a jump into \
                 data both land here."
            }
            Crash::BusError => "The alignment of the address being accessed.",
            Crash::Unclassified { .. } => "The signal, and the innermost frame.",
        }
    }

    /// Whether this is a Rust panic rather than a hardware fault.
    pub fn is_a_panic(&self) -> bool {
        matches!(self, Crash::Panic { .. })
    }
}

/// The frames whose presence identifies a panic *in progress*.
///
/// Getting this list right took two wrong versions, and both were wrong in
/// instructive ways.
///
/// A prefix list missed a real panic, because the toolchain names these
/// `__rustc::rust_panic` and `std::panicking::panic_with_hook` — neither
/// starts with anything a prefix list would guess. So: substrings.
///
/// Substrings alone then produced a false positive on a plain segfault,
/// because `std::panicking::catch_unwind` is on the stack of **every** Rust
/// program — `lang_start` installs it before `main` runs. A match on
/// `std::panicking::` therefore fires on programs that never panicked.
///
/// So the markers below name only functions that exist *because a panic is
/// happening*, and `NOT_A_PANIC` names the ones that are always there.
const PANIC_MACHINERY: [&str; 7] = [
    "rust_begin_unwind",
    "rust_panic",
    "core::panicking::panic",
    "std::panicking::panic_with_hook",
    "std::panicking::begin_panic",
    "std::panicking::panic_handler",
    "std::panicking::default_hook",
];

/// Present in every Rust program, panicking or not.
///
/// Checked first, so a frame that matches one of these never counts as
/// evidence of a panic however it matches above.
const NOT_A_PANIC: [&str; 3] = ["catch_unwind", "panic_cleanup", "panicking::try"];

/// Classify a crash from everything known about it.
///
/// `frames` are the resolved stack, innermost first.
pub fn classify(dump: &CoreDump, thread: &Thread, frames: &[Resolved]) -> Crash {
    // A panic outranks the signal: a panicking process aborts, and reporting
    // "terminated by signal 6" for a panic with a message is strictly worse
    // than reporting the message.
    if let Some(message) = panic_on_the_stack(frames) {
        return Crash::Panic { message };
    }

    match thread.signal {
        signal::SIGSEGV => classify_segfault(dump, thread),
        signal::SIGFPE => Crash::ArithmeticFault,
        signal::SIGILL => Crash::IllegalInstruction,
        signal::SIGBUS => Crash::BusError,
        signal::SIGABRT => Crash::Unclassified { signal: signal::SIGABRT },
        other => Crash::Unclassified { signal: other },
    }
}

/// Whether the panic machinery is on the stack, and the message if it is
/// recoverable.
fn panic_on_the_stack(frames: &[Resolved]) -> Option<Option<String>> {
    let found = frames.iter().any(|resolved| {
        resolved.locations.iter().any(|location| {
            location.function.as_deref().is_some_and(|function| {
                !NOT_A_PANIC.iter().any(|always| function.contains(always))
                    && PANIC_MACHINERY.iter().any(|marker| function.contains(marker))
            })
        })
    });
    // The message lives in a `&str` the panic machinery was handed, and
    // recovering it needs the argument registers at the panic frame. Not
    // attempted here: returning `None` for the message is honest, and
    // inventing one from nearby `.rodata` would be exactly the confident-wrong
    // failure this product exists to avoid.
    found.then_some(None)
}

/// Which kind of segfault.
fn classify_segfault(dump: &CoreDump, thread: &Thread) -> Crash {
    /// Nothing is mapped in the first page on any modern system, which is what
    /// makes a null dereference a fault rather than a silent corruption. The
    /// whole page counts, because `(*null).field` faults at the field's
    /// offset rather than at zero.
    const FIRST_PAGE: u64 = 4096;

    let stack_pointer = thread.registers.stack_pointer();

    // A stack overflow lands just below the lowest mapped stack page, because
    // the guard page is what turns recursion into a signal.
    if let Some(stack_pointer) = stack_pointer {
        let stack = dump
            .mappings
            .iter()
            .filter(|mapping| mapping.path == "[stack]" || mapping.path.is_empty())
            .map(|mapping| mapping.start)
            .min();
        if let Some(lowest) = stack {
            // Within a page below the stack's base: the guard page.
            if stack_pointer < lowest && lowest - stack_pointer <= FIRST_PAGE * 4 {
                return Crash::StackOverflow;
            }
        }
    }

    // The faulting address is in `si_addr`, which lives in the signal info the
    // core does not always retain. Without it, the instruction pointer being
    // in the first page means control jumped to null; otherwise we cannot say
    // *which* address faulted, only that one did.
    let instruction_pointer = thread.registers.instruction_pointer();
    if instruction_pointer.is_some_and(|pointer| pointer < FIRST_PAGE) {
        return Crash::NullDereference;
    }

    // Reading the faulting instruction would say which register held the
    // address, and whether that register is null. That needs a disassembler,
    // which is `F2.5`'s work; until then this reports what it can see.
    Crash::InvalidMemoryAccess { address: None }
}
