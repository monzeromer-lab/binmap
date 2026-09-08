//! Reference corpus, entry one: a binary that is bigger than it needs to be
//! for the ordinary reasons — formatting machinery, panic strings, unwinding
//! tables and symbols nobody reads.
//!
//! It exists so the sweep has something to find. Every configuration in the
//! matrix should still build and still pass the test below, which is what
//! makes it a fair test of the gates rather than only of the measurement.

use std::collections::BTreeMap;
use std::fmt::Write as _;

fn tally(words: &[&str]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for word in words {
        *counts.entry(word.to_string()).or_insert(0) += 1;
    }
    counts
}

fn render(counts: &BTreeMap<String, usize>) -> String {
    let mut out = String::new();
    for (word, count) in counts {
        let _ = writeln!(out, "{word:<12} {count:>4} {:>8.2}%", *count as f64 * 100.0 / 6.0);
    }
    out
}

fn main() {
    let words = ["measure", "then", "claim", "measure", "then", "verify"];
    print!("{}", render(&tally(&words)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_word_is_counted_once_per_appearance() {
        let counts = tally(&["a", "b", "a"]);
        assert_eq!(counts.get("a"), Some(&2));
        assert_eq!(counts.get("b"), Some(&1));
    }

    #[test]
    fn rendering_lists_every_word() {
        let rendered = render(&tally(&["alpha", "beta"]));
        assert!(rendered.contains("alpha"));
        assert!(rendered.contains("beta"));
    }
}
