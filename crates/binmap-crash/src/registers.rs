//! Register numbering, in the two orders that matter (`TOOLING-BINARY §1.4`).
//!
//! The reference calls this "the trap that will cost you a day", and the reason
//! is worth stating precisely: **a core dump and DWARF number the same
//! registers differently.** `user_regs_struct` is ptrace order, where index 0
//! is `r15`. DWARF's x86-64 table has index 0 as `rax`. A location list saying
//! "this variable lives in register 6" means `rbp` to DWARF and `r11` to
//! ptrace.
//!
//! Nothing in this crate indexes a register array directly. Everything goes
//! through here, and the mapping is tested in both directions — because the
//! failure mode is not a crash, it is a debugger confidently showing the
//! contents of the wrong register, which is exactly the "confident-wrong"
//! outcome the whole product is built to avoid.

use serde::{Deserialize, Serialize};

/// A DWARF x86-64 register number.
///
/// A newtype rather than a bare `u16`, because the entire bug class here is
/// passing one numbering where the other was expected, and the compiler can
/// catch that if the two are different types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DwarfRegister(pub u16);

impl DwarfRegister {
    pub const RAX: Self = Self(0);
    pub const RDX: Self = Self(1);
    pub const RCX: Self = Self(2);
    pub const RBX: Self = Self(3);
    pub const RSI: Self = Self(4);
    pub const RDI: Self = Self(5);
    pub const RBP: Self = Self(6);
    pub const RSP: Self = Self(7);
    pub const R8: Self = Self(8);
    pub const R9: Self = Self(9);
    pub const R10: Self = Self(10);
    pub const R11: Self = Self(11);
    pub const R12: Self = Self(12);
    pub const R13: Self = Self(13);
    pub const R14: Self = Self(14);
    pub const R15: Self = Self(15);
    /// DWARF's "return address" column, which on x86-64 is `rip`.
    pub const RETURN_ADDRESS: Self = Self(16);

    /// The name, for anything a user reads.
    pub fn name(self) -> &'static str {
        const NAMES: [&str; 17] = [
            "rax", "rdx", "rcx", "rbx", "rsi", "rdi", "rbp", "rsp", "r8", "r9", "r10", "r11",
            "r12", "r13", "r14", "r15", "rip",
        ];
        NAMES.get(self.0 as usize).copied().unwrap_or("unknown")
    }
}

/// Index into `user_regs_struct`, which is ptrace order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PtraceSlot(pub usize);

impl PtraceSlot {
    /// `user_regs_struct` holds 27 `u64` fields on x86-64.
    pub const COUNT: usize = 27;

    pub const RIP: Self = Self(16);
    pub const RSP: Self = Self(19);
    pub const RBP: Self = Self(4);
    /// Not a DWARF register at all: the syscall number the kernel recorded.
    pub const ORIG_RAX: Self = Self(15);
}

/// DWARF register number → index into `user_regs_struct`.
///
/// Transcribed from `TOOLING-BINARY §1.4`. The order below is deliberately
/// written one per line with the DWARF name in a comment, because a
/// transcription error here is invisible at runtime.
const DWARF_TO_PTRACE: [usize; 17] = [
    10, // 0  rax
    12, // 1  rdx
    11, // 2  rcx
    5,  // 3  rbx
    13, // 4  rsi
    14, // 5  rdi
    4,  // 6  rbp
    19, // 7  rsp
    9,  // 8  r8
    8,  // 9  r9
    7,  // 10 r10
    6,  // 11 r11
    3,  // 12 r12
    2,  // 13 r13
    1,  // 14 r14
    0,  // 15 r15
    16, // 16 return address == rip
];

impl DwarfRegister {
    /// Where this register sits in `user_regs_struct`.
    ///
    /// `None` for a DWARF register this table does not cover — the xmm and
    /// floating-point registers, which live in a different note entirely.
    /// Returning `None` rather than guessing is the point: a variable in
    /// `xmm0` is a variable we cannot read, and saying so beats reading
    /// whatever `user_regs_struct` happens to hold at that index.
    pub fn ptrace_slot(self) -> Option<PtraceSlot> {
        DWARF_TO_PTRACE.get(self.0 as usize).copied().map(PtraceSlot)
    }
}

impl PtraceSlot {
    /// The DWARF register this slot holds, if it is one.
    ///
    /// The inverse, and it is genuinely partial: `orig_rax`, the segment
    /// registers and `eflags` are in `user_regs_struct` and have no DWARF
    /// number in this table.
    pub fn dwarf_register(self) -> Option<DwarfRegister> {
        DWARF_TO_PTRACE
            .iter()
            .position(|slot| *slot == self.0)
            .map(|number| DwarfRegister(number as u16))
    }
}

/// One thread's general-purpose registers, as the core recorded them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Registers {
    /// `user_regs_struct`, verbatim and in ptrace order.
    ///
    /// Private so that nothing outside this module can index it: every read
    /// goes through `get`, which takes a DWARF number and does the conversion.
    slots: Vec<u64>,
}

impl Registers {
    /// Read `user_regs_struct` out of an `NT_PRSTATUS` note's descriptor.
    ///
    /// The registers sit at offset 112, derived field by field in
    /// `TOOLING-BINARY §1.4`. Anything shorter than that is not a prstatus
    /// note, and reading it as one would produce plausible garbage.
    pub fn from_prstatus(descriptor: &[u8]) -> Option<Self> {
        /// Offset of `pr_reg` within `struct elf_prstatus` on x86-64.
        const PR_REG_OFFSET: usize = 112;

        let registers = descriptor.get(PR_REG_OFFSET..)?;
        if registers.len() < PtraceSlot::COUNT * 8 {
            return None;
        }
        Some(Self {
            slots: registers
                .as_chunks::<8>()
                .0
                .iter()
                .take(PtraceSlot::COUNT)
                .map(|bytes| u64::from_le_bytes(*bytes))
                .collect(),
        })
    }

    /// Build from raw slots, for tests and for a synthetic frame.
    pub fn from_slots(slots: Vec<u64>) -> Self {
        Self { slots }
    }

    /// The value of a DWARF register.
    ///
    /// `None` when the register is not one the core recorded, which is the
    /// honest answer for the floating-point and vector registers.
    pub fn get(&self, register: DwarfRegister) -> Option<u64> {
        let slot = register.ptrace_slot()?;
        self.slots.get(slot.0).copied()
    }

    /// The instruction pointer.
    pub fn instruction_pointer(&self) -> Option<u64> {
        self.slots.get(PtraceSlot::RIP.0).copied()
    }

    pub fn stack_pointer(&self) -> Option<u64> {
        self.slots.get(PtraceSlot::RSP.0).copied()
    }

    pub fn frame_pointer(&self) -> Option<u64> {
        self.slots.get(PtraceSlot::RBP.0).copied()
    }

    /// Every DWARF register this core recorded, for display.
    pub fn dwarf_registers(&self) -> Vec<(DwarfRegister, u64)> {
        (0..=16u16)
            .map(DwarfRegister)
            .filter_map(|register| self.get(register).map(|value| (register, value)))
            .collect()
    }
}
