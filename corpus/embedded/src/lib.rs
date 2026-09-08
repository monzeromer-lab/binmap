//! Reference corpus, entry four: `no_std`.
//!
//! The audience §8 has in mind for this entry counts bytes for a living, and
//! the levers that move the number for them are not the ones that move it for
//! a desktop binary. A `no_std` crate has no formatting machinery, no
//! allocator and no panic strings to remove, so the sweep has to find its
//! wins elsewhere — which is exactly why it belongs in the corpus. A tool
//! tuned only against crates full of `println!` will report large, easy wins
//! and be useless here.
//!
//! It is a library rather than a binary because a `no_std` *binary* needs a
//! panic handler and a target with no standard library, and pinning the corpus
//! to a cross-compilation toolchain would make it unbuildable on a machine
//! that has not installed one. As a library it builds anywhere, and the
//! attribution work still has real code to attribute.

#![no_std]

/// A fixed-capacity ring buffer, the shape of thing that actually appears in
/// firmware.
///
/// No allocation, no panics on the happy path, and every operation is
/// constant time.
pub struct Ring<const N: usize> {
    slots: [u8; N],
    head: usize,
    len: usize,
}

impl<const N: usize> Default for Ring<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> Ring<N> {
    pub const fn new() -> Self {
        Self { slots: [0; N], head: 0, len: 0 }
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub const fn is_full(&self) -> bool {
        self.len == N
    }

    /// Push a byte. Returns `false` when full rather than panicking, because
    /// a dropped sample is recoverable and a reset is not.
    pub fn push(&mut self, byte: u8) -> bool {
        if self.is_full() {
            return false;
        }
        let index = (self.head + self.len) % N;
        self.slots[index] = byte;
        self.len += 1;
        true
    }

    pub fn pop(&mut self) -> Option<u8> {
        if self.len == 0 {
            return None;
        }
        let byte = self.slots[self.head];
        self.head = (self.head + 1) % N;
        self.len -= 1;
        Some(byte)
    }
}

/// A CRC-8 over a slice, table-free so it costs no `.rodata`.
///
/// One of the size trade-offs the sweep exists to measure: a table would be
/// faster and 256 bytes larger, which on this audience's targets is a real
/// decision rather than a rounding error.
pub fn crc8(bytes: &[u8]) -> u8 {
    let mut crc: u8 = 0xFF;
    for byte in bytes {
        crc ^= byte;
        for _ in 0..8 {
            crc = if crc & 0x80 != 0 { (crc << 1) ^ 0x31 } else { crc << 1 };
        }
    }
    crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_ring_refuses_rather_than_panicking() {
        let mut ring: Ring<2> = Ring::new();
        assert!(ring.push(1));
        assert!(ring.push(2));
        assert!(!ring.push(3), "a dropped sample is recoverable; a reset is not");
        assert!(ring.is_full());
    }

    #[test]
    fn it_wraps_around_its_own_capacity() {
        let mut ring: Ring<3> = Ring::new();
        for byte in 1..=3 {
            ring.push(byte);
        }
        assert_eq!(ring.pop(), Some(1));
        assert!(ring.push(4));
        assert_eq!(ring.pop(), Some(2));
        assert_eq!(ring.pop(), Some(3));
        assert_eq!(ring.pop(), Some(4));
        assert!(ring.is_empty());
    }

    #[test]
    fn the_checksum_is_stable() {
        assert_eq!(crc8(&[]), 0xFF);
        assert_eq!(crc8(b"binmap"), crc8(b"binmap"));
        assert_ne!(crc8(b"binmap"), crc8(b"binmaq"));
    }
}
