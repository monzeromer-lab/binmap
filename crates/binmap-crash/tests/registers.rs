//! Register numbering (`TOOLING-BINARY §1.4`).
//!
//! The reference calls this "the trap that will cost you a day". The failure
//! mode is not a crash — it is a debugger confidently showing the contents of
//! the wrong register — so these check the mapping exhaustively rather than by
//! sampling.

use binmap_crash::registers::{DwarfRegister, PtraceSlot, Registers};
use proptest::prelude::*;

#[test]
fn dwarf_and_ptrace_disagree_about_register_zero() {
    // The specific collision the reference warns about, stated as a test so it
    // cannot be "fixed" by making the two numberings agree.
    assert_eq!(DwarfRegister::RAX.0, 0, "DWARF numbers rax as 0");
    assert_eq!(DwarfRegister::RAX.ptrace_slot(), Some(PtraceSlot(10)), "ptrace puts rax at 10");
    assert_eq!(PtraceSlot(0).dwarf_register(), Some(DwarfRegister::R15), "ptrace slot 0 is r15");
}

#[test]
fn register_six_means_different_things_in_the_two_numberings() {
    // The reference's own example: "a location list saying the variable is in
    // register 6 means rbp in DWARF and r11 in ptrace order".
    assert_eq!(DwarfRegister(6), DwarfRegister::RBP);
    assert_eq!(PtraceSlot(6).dwarf_register(), Some(DwarfRegister::R11));
}

#[test]
fn the_mapping_round_trips_for_every_general_purpose_register() {
    for number in 0..=16u16 {
        let register = DwarfRegister(number);
        let slot = register.ptrace_slot().unwrap_or_else(|| panic!("{number} has no slot"));
        assert_eq!(
            slot.dwarf_register(),
            Some(register),
            "{} ({number}) did not round-trip through slot {}",
            register.name(),
            slot.0
        );
    }
}

#[test]
fn no_two_dwarf_registers_share_a_ptrace_slot() {
    // A duplicate would make two registers alias, and every read of one would
    // silently return the other.
    let slots: Vec<usize> =
        (0..=16u16).filter_map(|n| DwarfRegister(n).ptrace_slot()).map(|slot| slot.0).collect();
    let unique: std::collections::BTreeSet<usize> = slots.iter().copied().collect();
    assert_eq!(unique.len(), slots.len(), "aliased slots: {slots:?}");
}

#[test]
fn every_slot_is_inside_user_regs_struct() {
    for number in 0..=16u16 {
        let slot = DwarfRegister(number).ptrace_slot().unwrap();
        assert!(slot.0 < PtraceSlot::COUNT, "{} is out of range", slot.0);
    }
}

#[test]
fn a_register_the_core_does_not_record_is_absent_rather_than_guessed() {
    // xmm and the floating-point registers live in a different note entirely.
    // Reading whatever `user_regs_struct` holds at that index would produce a
    // number, and the number would be meaningless.
    assert_eq!(DwarfRegister(17).ptrace_slot(), None, "xmm0 is not in user_regs_struct");
    assert_eq!(DwarfRegister(64).ptrace_slot(), None);

    let registers = Registers::from_slots(vec![0; PtraceSlot::COUNT]);
    assert_eq!(registers.get(DwarfRegister(17)), None);
}

#[test]
fn slots_that_are_not_dwarf_registers_say_so() {
    // `orig_rax`, the segment registers and `eflags` are in
    // `user_regs_struct` and have no DWARF number.
    assert_eq!(PtraceSlot::ORIG_RAX.dwarf_register(), None);
    for slot in [17usize, 18, 20, 21, 22, 23, 24, 25, 26] {
        assert_eq!(PtraceSlot(slot).dwarf_register(), None, "slot {slot}");
    }
}

#[test]
fn the_named_slots_match_the_mapping() {
    assert_eq!(DwarfRegister::RETURN_ADDRESS.ptrace_slot(), Some(PtraceSlot::RIP));
    assert_eq!(DwarfRegister::RSP.ptrace_slot(), Some(PtraceSlot::RSP));
    assert_eq!(DwarfRegister::RBP.ptrace_slot(), Some(PtraceSlot::RBP));
}

#[test]
fn every_register_has_a_name_a_person_can_read() {
    for number in 0..=16u16 {
        let name = DwarfRegister(number).name();
        assert!(!name.is_empty());
        assert_ne!(name, "unknown", "register {number} is unnamed");
    }
    assert_eq!(DwarfRegister(99).name(), "unknown", "and an unknown one says so");
}

#[test]
fn reading_registers_out_of_a_prstatus_note_uses_the_documented_offset() {
    // `pr_reg` sits at offset 112, derived field by field in §1.4. A note
    // shorter than that is not a prstatus, and reading it as one would produce
    // plausible garbage.
    let mut descriptor = vec![0u8; 112 + PtraceSlot::COUNT * 8];
    // Put a recognisable value in the rip slot.
    let rip_offset = 112 + PtraceSlot::RIP.0 * 8;
    descriptor[rip_offset..rip_offset + 8].copy_from_slice(&0xdead_beefu64.to_le_bytes());

    let registers = Registers::from_prstatus(&descriptor).expect("a full note");
    assert_eq!(registers.instruction_pointer(), Some(0xdead_beef));
    assert_eq!(registers.get(DwarfRegister::RETURN_ADDRESS), Some(0xdead_beef));
}

#[test]
fn a_truncated_prstatus_note_is_refused() {
    assert!(Registers::from_prstatus(&[0u8; 50]).is_none());
    assert!(Registers::from_prstatus(&[]).is_none());
    // Exactly one byte short of a full register block.
    assert!(Registers::from_prstatus(&vec![0u8; 112 + PtraceSlot::COUNT * 8 - 1]).is_none());
}

#[test]
fn dwarf_registers_lists_only_what_was_recorded() {
    let registers = Registers::from_slots(vec![7; PtraceSlot::COUNT]);
    let listed = registers.dwarf_registers();
    assert_eq!(listed.len(), 17, "the seventeen general-purpose registers");
    assert!(listed.iter().all(|(_, value)| *value == 7));
}

proptest! {
    /// Any DWARF number is handled without panicking.
    ///
    /// Location lists come from a file we did not write, so an out-of-range
    /// register number is untrusted input rather than a programming error.
    #[test]
    fn any_register_number_is_handled(number in any::<u16>()) {
        let register = DwarfRegister(number);
        let _ = register.name();
        let _ = register.ptrace_slot();
    }

    /// Any slot index is handled likewise.
    #[test]
    fn any_slot_index_is_handled(slot in any::<usize>()) {
        let _ = PtraceSlot(slot).dwarf_register();
    }

    /// Reading from an arbitrary register file never panics and never invents
    /// a value for a register outside it.
    #[test]
    fn reading_an_arbitrary_register_file_is_total(
        slots in proptest::collection::vec(any::<u64>(), 0..40),
        number in any::<u16>(),
    ) {
        let registers = Registers::from_slots(slots.clone());
        let value = registers.get(DwarfRegister(number));
        if let Some(value) = value {
            let slot = DwarfRegister(number).ptrace_slot().expect("a value implies a slot");
            prop_assert_eq!(value, slots[slot.0]);
        }
    }
}
