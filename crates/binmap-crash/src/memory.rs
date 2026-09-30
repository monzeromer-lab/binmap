//! Reading another process's memory, dead or alive (`F2.3`, `F3.1`).
//!
//! The unwinder started out welded to a core dump, which was right until
//! Phase 3 needed the same walk over a *running* process. A sampling profiler
//! is an unwinder that runs a thousand times a second, and writing a second
//! one would mean two CFI implementations drifting apart — with the register
//! rules, the part hardest to get right, drifting first.
//!
//! So the unwinder reads through this trait and does not know which it has. A
//! core is a file with holes in it; a live process is a file with different
//! holes. Both answer the only question unwinding asks: what is at this
//! address, and is it there at all.

/// Somewhere a stack can be read from.
pub trait Memory {
    /// Read `length` bytes at a runtime address.
    ///
    /// `None` when the address is not readable, which is a normal answer
    /// rather than an error: a core omits pages that were never written, and a
    /// live process unmaps things. An unwinder that treated an unreadable
    /// address as zero would walk into invented frames.
    fn read(&self, address: u64, length: usize) -> Option<Vec<u8>>;

    /// Whether an address is inside something mapped.
    ///
    /// Distinct from being *readable*: a core routinely omits the pages of a
    /// mapped file it did not need to store, and a return address into one of
    /// those is entirely plausible. Rejecting it truncated every stack at the
    /// first library frame.
    fn is_mapped(&self, address: u64) -> bool;

    /// Read a little-endian `u64`, which is what a stack is made of.
    fn read_u64(&self, address: u64) -> Option<u64> {
        let bytes = self.read(address, 8)?;
        Some(u64::from_le_bytes(bytes.try_into().ok()?))
    }
}

/// A core dump, read through its own segments.
pub struct CoreMemory<'a> {
    pub dump: &'a crate::dump::CoreDump,
    pub data: &'a [u8],
}

impl Memory for CoreMemory<'_> {
    fn read(&self, address: u64, length: usize) -> Option<Vec<u8>> {
        self.dump.read(self.data, address, length).map(<[u8]>::to_vec)
    }

    fn is_mapped(&self, address: u64) -> bool {
        self.dump.mappings.iter().any(|mapping| mapping.contains(address))
            || self.dump.segments.iter().any(|segment| segment.contains(address))
    }
}
