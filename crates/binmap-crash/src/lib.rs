//! Crash analysis (Phase 2).
//!
//! An ELF core dump is a snapshot of a dead process: its memory, its threads'
//! registers, and a note describing which files were mapped where. Turning
//! that into "your program died on this line, and here is what the variables
//! held" is the hardest thing in this product, and the place where a confident
//! wrong answer is most damaging — a debugger that points at the wrong line is
//! worse than one that admits it cannot tell.
//!
//! So the refusals here are load-bearing rather than defensive. A core from a
//! different build than the binary, a load bias whose two derivations
//! disagree, a register that is not in the dump: each says so instead of
//! producing something plausible.

pub mod dump;
pub mod registers;

pub use dump::{CoreDump, Mapping, Segment, Thread};
pub use registers::{DwarfRegister, PtraceSlot, Registers};
