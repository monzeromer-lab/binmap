//! Reference corpus, entry six: a WASM target.
//!
//! Size is the whole story for something served over a network, and the levers
//! are not a native binary's. `strip` and `panic=abort` matter less; `opt-level
//! = "z"`, `lto` and what the standard library drags in matter more. A `.wasm`
//! module also has no ELF symbol table, so this is the entry that proves the
//! reader degrades honestly rather than reporting zero sections as though it
//! had measured them.
//!
//! `crate-type` includes `rlib` so `cargo test` still runs on the host; the
//! `cdylib` is what a sweep measures.

/// A run-length encoder, the sort of small pure routine that ships to a
/// browser.
pub fn encode(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        let mut run = 1usize;
        while index + run < bytes.len() && bytes[index + run] == byte && run < 255 {
            run += 1;
        }
        out.push(run as u8);
        out.push(byte);
        index += run;
    }
    out
}

pub fn decode(pairs: &[u8]) -> Vec<u8> {
    pairs
        .chunks_exact(2)
        .flat_map(|pair| std::iter::repeat_n(pair[1], pair[0] as usize))
        .collect()
}

/// The entry point a host would call. `no_mangle` so it survives into the
/// module's exports, which is what makes it worth attributing.
///
/// # Safety
/// The caller must pass a valid pointer to `len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn encoded_len(pointer: *const u8, len: usize) -> usize {
    if pointer.is_null() {
        return 0;
    }
    let bytes = unsafe { std::slice::from_raw_parts(pointer, len) };
    encode(bytes).len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_run_round_trips() {
        let original = b"aaabbbccccd";
        assert_eq!(decode(&encode(original)), original);
    }

    #[test]
    fn a_run_longer_than_a_byte_can_count_is_split() {
        let original = vec![7u8; 300];
        let encoded = encode(&original);
        assert_eq!(encoded.len(), 4, "300 does not fit in one length byte");
        assert_eq!(decode(&encoded), original);
    }

    #[test]
    fn nothing_encodes_to_nothing() {
        assert!(encode(&[]).is_empty());
        assert!(decode(&[]).is_empty());
    }
}
