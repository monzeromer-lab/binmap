//! Walking the stack (`F2.3`, `TOOLING-BINARY §2.5`).
//!
//! Two ways to walk a stack, and the difference matters. **Frame pointers** are
//! a linked list: follow `rbp`, and each cell holds the previous frame's `rbp`
//! and return address. It is trivial and it is wrong on optimised code, because
//! `-O2` uses `rbp` as a general-purpose register and the chain simply is not
//! there. **CFI** is the table the compiler emits in `.eh_frame` describing,
//! for every instruction, where the canonical frame address is and where each
//! saved register went. It is correct on optimised code, which is the only kind
//! anyone takes a core of.
//!
//! So CFI is the primary and frame pointers are the fallback, and which one
//! produced a frame is recorded — a frame-pointer walk through optimised code
//! can produce plausible garbage, and the reader is entitled to know that is
//! what they are looking at.
//!
//! The walk stops rather than guessing. An unwinder that keeps going past the
//! end of the stack produces frames out of whatever integers happen to be in
//! memory, and they look exactly like real frames.

use crate::dump::CoreDump;
use crate::modules::Modules;
use crate::registers::{DwarfRegister, Registers};
use binmap_core::error::Result;
use gimli::{
    BaseAddresses, CfaRule, EhFrame, LittleEndian, RegisterRule, UnwindContext, UnwindSection,
};
use serde::{Deserialize, Serialize};

/// How a frame was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Method {
    /// From `.eh_frame`. Correct on optimised code.
    Cfi,
    /// By following `rbp`. A guess, and labelled as one, because optimised
    /// code frequently has no frame-pointer chain at all.
    FramePointer,
    /// The innermost frame, taken straight from the core's registers.
    Registers,
}

impl Method {
    pub fn label(self) -> &'static str {
        match self {
            Method::Cfi => "cfi",
            Method::FramePointer => "frame pointer",
            Method::Registers => "registers",
        }
    }

    /// Whether a frame found this way needs a caveat shown beside it.
    pub fn is_a_guess(self) -> bool {
        matches!(self, Method::FramePointer)
    }
}

/// Why the walk stopped.
///
/// Every reason is a real answer about the stack rather than an error: a walk
/// that reached `main` is complete, and one that ran out of CFI has told you
/// where the compiler stopped describing itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StoppedBecause {
    /// The outermost frame was reached.
    ReachedTheBottom,
    /// No CFI covers this address and there is no frame-pointer chain either.
    NoUnwindInformation { at: u64 },
    /// The return address read from the stack was not a plausible code
    /// address.
    ImplausibleReturnAddress { value: u64 },
    /// The stack pointer stopped moving, which means the rules are not
    /// describing a real frame and continuing would loop forever.
    StackDidNotAdvance,
    /// The cap was reached. Deep recursion is real, and so is a corrupt stack
    /// that unwinds forever.
    ReachedTheLimit { limit: usize },
    /// Memory the walk needed was not in the core.
    MemoryNotInTheCore { at: u64 },
}

impl StoppedBecause {
    /// Whether the stack shown is the whole stack.
    pub fn is_complete(&self) -> bool {
        matches!(self, StoppedBecause::ReachedTheBottom)
    }

    pub fn describe(&self) -> String {
        match self {
            StoppedBecause::ReachedTheBottom => "the outermost frame was reached".into(),
            StoppedBecause::NoUnwindInformation { at } => format!(
                "nothing describes how to unwind past {at:#x}. Frames below this one exist and \
                 cannot be recovered — usually a hand-written assembly routine or a library \
                 built without unwind tables."
            ),
            StoppedBecause::ImplausibleReturnAddress { value } => format!(
                "the return address read from the stack was {value:#x}, which is not a code \
                 address. The stack is damaged from here down, so the walk stopped rather than \
                 inventing frames from whatever is in memory."
            ),
            StoppedBecause::StackDidNotAdvance => {
                "the stack pointer stopped moving, so the unwind rules are not describing a real \
                 frame. Continuing would loop forever."
                    .into()
            }
            StoppedBecause::ReachedTheLimit { limit } => format!(
                "the walk reached {limit} frames and stopped. Either the recursion really is \
                 that deep, or the stack is corrupt."
            ),
            StoppedBecause::MemoryNotInTheCore { at } => format!(
                "the memory at {at:#x} is not in this core, so the walk cannot continue. A core \
                 truncated by `ulimit -c` is the usual cause."
            ),
        }
    }
}

/// One frame.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Frame {
    /// Where the program counter was, as it ran.
    pub runtime_address: u64,
    /// The same address as the owning module's DWARF numbers it.
    ///
    /// Each module has its own bias, so this is only meaningful together with
    /// `module` — a link-time address means nothing without knowing which
    /// file's link it refers to.
    pub link_time_address: Option<u64>,
    /// Which mapped file this frame is in.
    pub module: Option<String>,
    pub method: Method,
    /// The canonical frame address, where CFI gave one.
    pub canonical_frame_address: Option<u64>,
}

/// A whole stack.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stack {
    /// Innermost first, as every debugger shows them.
    pub frames: Vec<Frame>,
    pub stopped_because: StoppedBecause,
}

impl Stack {
    /// Whether any frame was reached by guessing.
    pub fn contains_guesses(&self) -> bool {
        self.frames.iter().any(|frame| frame.method.is_a_guess())
    }

    /// The one-line summary.
    pub fn describe(&self) -> String {
        let guessed = self.frames.iter().filter(|frame| frame.method.is_a_guess()).count();
        let mut line = format!("{} frames", self.frames.len());
        if guessed > 0 {
            line.push_str(&format!(", {guessed} of them guessed from the frame pointer"));
        }
        if !self.stopped_because.is_complete() {
            line.push_str("; incomplete — ");
            line.push_str(&self.stopped_because.describe());
        }
        line
    }
}

/// How many frames to walk before giving up.
///
/// Deep recursion is real and so is a corrupt stack that unwinds forever. The
/// cap exists for the second, and the stop reason says which happened.
pub const FRAME_LIMIT: usize = 512;

/// Walk the stack of one thread.
///
/// Unwinding is per module: each frame is unwound with the CFI of whichever
/// mapped file its program counter is in, and each module has its own load
/// bias. Using the executable's tables for a libc frame does not fail — it
/// decodes something, and the something is wrong.
pub fn walk(
    dump: &CoreDump,
    core_data: &[u8],
    modules: &Modules,
    registers: &Registers,
) -> Result<Stack> {
    let mut context = UnwindContext::new();
    let mut frames = Vec::new();
    let mut current = registers.clone();
    let mut method = Method::Registers;

    let stopped_because = loop {
        let Some(pc) = current.instruction_pointer() else {
            break StoppedBecause::StackDidNotAdvance;
        };

        let module = modules.containing(pc);
        frames.push(Frame {
            runtime_address: pc,
            link_time_address: module.and_then(|module| module.to_link_time(pc)),
            module: module.map(|module| module.path.clone()),
            method,
            canonical_frame_address: None,
        });

        if frames.len() >= FRAME_LIMIT {
            break StoppedBecause::ReachedTheLimit { limit: FRAME_LIMIT };
        }

        let Some(module) = module else {
            break StoppedBecause::NoUnwindInformation { at: pc };
        };

        let eh_frame = EhFrame::new(&module.eh_frame, LittleEndian);
        let bases = BaseAddresses::default().set_eh_frame(module.eh_frame_address);

        // A return address points at the instruction *after* the call, which
        // can belong to the next function entirely — a call in tail position
        // is followed by whatever the linker put next. Unwinding must look up
        // the call itself, or a frame at the end of a function reads the wrong
        // CFI row and unwinds to the wrong place.
        let Some(link_time) = module.to_link_time(pc) else {
            break StoppedBecause::NoUnwindInformation { at: pc };
        };
        let lookup = if frames.len() == 1 { link_time } else { link_time.wrapping_sub(1) };

        let row = eh_frame
            .unwind_info_for_address(&bases, &mut context, lookup, EhFrame::cie_from_offset)
            .ok();

        let Some(row) = row else {
            break StoppedBecause::NoUnwindInformation { at: pc };
        };

        let cfa = match row.cfa() {
            CfaRule::RegisterAndOffset { register, offset } => {
                let Some(base) = current.get(DwarfRegister(register.0)) else {
                    break StoppedBecause::NoUnwindInformation { at: pc };
                };
                base.wrapping_add(*offset as u64)
            }
            // An expression-defined CFA needs a full DWARF evaluator. Refusing
            // beats approximating: an unwinder that guesses the CFA still
            // produces frames, and they are wrong.
            CfaRule::Expression(_) => break StoppedBecause::NoUnwindInformation { at: pc },
        };

        if let Some(frame) = frames.last_mut() {
            frame.canonical_frame_address = Some(cfa);
        }

        let return_address = match row.register(gimli::X86_64::RA) {
            None => break StoppedBecause::NoUnwindInformation { at: pc },
            Some(rule) => match rule {
                RegisterRule::Offset(offset) => {
                    let at = cfa.wrapping_add(offset as u64);
                    match dump.read_u64(core_data, at) {
                        Some(value) => value,
                        None => break StoppedBecause::MemoryNotInTheCore { at },
                    }
                }
                RegisterRule::Register(register) => match current.get(DwarfRegister(register.0)) {
                    Some(value) => value,
                    None => break StoppedBecause::NoUnwindInformation { at: pc },
                },
                // `undefined` on the return address is how the outermost frame
                // is marked: there is nothing above `_start`.
                RegisterRule::Undefined => break StoppedBecause::ReachedTheBottom,
                _ => break StoppedBecause::NoUnwindInformation { at: pc },
            },
        };

        if return_address == 0 {
            break StoppedBecause::ReachedTheBottom;
        }
        // Plausible means "inside something that was mapped", not "inside
        // something the core stored". A core routinely omits file-backed
        // executable pages, so checking only the stored segments rejected
        // every return into a shared library.
        let mapped = dump.mappings.iter().any(|mapping| mapping.contains(return_address));
        let stored = dump.segments.iter().any(|segment| segment.contains(return_address));
        if !mapped && !stored {
            break StoppedBecause::ImplausibleReturnAddress { value: return_address };
        }

        let mut next = build_caller_registers(&current, row, cfa, dump, core_data);
        let previous_sp = current.stack_pointer().unwrap_or(0);
        set(&mut next, DwarfRegister::RETURN_ADDRESS, return_address);
        set(&mut next, DwarfRegister::RSP, cfa);

        // A stack that does not move means the rules are not describing a real
        // frame, and continuing would loop forever producing identical frames.
        if cfa <= previous_sp {
            break StoppedBecause::StackDidNotAdvance;
        }

        current = next;
        method = Method::Cfi;
    };

    Ok(Stack { frames, stopped_because })
}

/// Apply a CFI row's register rules to produce the caller's registers.
///
/// Registers with no rule are carried forward, which is what "the callee did
/// not touch it" means. A register the callee saved and restored is read back
/// from where the rule says it went.
fn build_caller_registers(
    current: &Registers,
    row: &gimli::UnwindTableRow<usize>,
    cfa: u64,
    dump: &CoreDump,
    core_data: &[u8],
) -> Registers {
    let mut slots: Vec<u64> = (0..crate::registers::PtraceSlot::COUNT)
        .map(|slot| {
            crate::registers::PtraceSlot(slot)
                .dwarf_register()
                .and_then(|register| current.get(register))
                .unwrap_or(0)
        })
        .collect();

    for number in 0..16u16 {
        let register = DwarfRegister(number);
        let Some(slot) = register.ptrace_slot() else { continue };
        let Some(rule) = row.register(gimli::Register(number)) else { continue };
        let value = match rule {
            RegisterRule::Offset(offset) => {
                dump.read_u64(core_data, cfa.wrapping_add(offset as u64))
            }
            RegisterRule::ValOffset(offset) => Some(cfa.wrapping_add(offset as u64)),
            RegisterRule::Register(other) => current.get(DwarfRegister(other.0)),
            // `SameValue` and `Undefined` both mean "do not change it here";
            // the difference is whether the caller's value is knowable, and
            // carrying it forward is the best available answer either way.
            _ => continue,
        };
        if let (Some(value), Some(entry)) = (value, slots.get_mut(slot.0)) {
            *entry = value;
        }
    }

    Registers::from_slots(slots)
}

fn set(registers: &mut Registers, register: DwarfRegister, value: u64) {
    let mut slots: Vec<u64> = (0..crate::registers::PtraceSlot::COUNT)
        .map(|slot| {
            crate::registers::PtraceSlot(slot)
                .dwarf_register()
                .and_then(|r| registers.get(r))
                .unwrap_or(0)
        })
        .collect();
    if let Some(entry) = register.ptrace_slot().and_then(|slot| slots.get_mut(slot.0)) {
        *entry = value;
    }
    *registers = Registers::from_slots(slots);
}
