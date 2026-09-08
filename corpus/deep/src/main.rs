//! Reference corpus, entry five: a deep dependency tree.
//!
//! The point of this entry is proportion. In `tiny` and `generics` almost
//! every byte belongs to code in the crate; here most of them belong to
//! `regex`, `serde_json` and their transitive dependencies. That is the common
//! case, and it is the one size attribution has to get right — a tool that
//! reports "your code is 4 KB" without saying where the other 1.5 MB went has
//! answered the wrong question.
//!
//! It is also the entry where `lto` and `codegen-units` earn their keep, since
//! there is cross-crate inlining to do.

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Record {
    name: String,
    bytes: u64,
    tags: Vec<String>,
}

/// Keep the records whose name matches, ordered by size.
fn heaviest(records: Vec<Record>, pattern: &str) -> Vec<Record> {
    let Ok(matcher) = regex::Regex::new(pattern) else {
        return Vec::new();
    };
    let mut kept: Vec<Record> =
        records.into_iter().filter(|record| matcher.is_match(&record.name)).collect();
    kept.sort_by(|a, b| b.bytes.cmp(&a.bytes));
    kept
}

fn main() {
    let json = r#"[
        {"name":"core::fmt","bytes":41208,"tags":["formatting"]},
        {"name":"core::panicking","bytes":8814,"tags":["panic"]},
        {"name":"app::router","bytes":2044,"tags":["yours"]}
    ]"#;

    let records: Vec<Record> = serde_json::from_str(json).expect("the fixture parses");
    for record in heaviest(records, "^core::") {
        println!("{:>8}  {}  {:?}", record.bytes, record.name, record.tags);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Vec<Record> {
        vec![
            Record { name: "core::fmt".into(), bytes: 41208, tags: vec!["formatting".into()] },
            Record { name: "app::router".into(), bytes: 2044, tags: vec!["yours".into()] },
            Record { name: "core::panicking".into(), bytes: 8814, tags: vec!["panic".into()] },
        ]
    }

    #[test]
    fn matching_records_come_back_heaviest_first() {
        let kept = heaviest(fixture(), "^core::");
        assert_eq!(kept.len(), 2);
        assert_eq!(kept[0].name, "core::fmt");
        assert_eq!(kept[1].name, "core::panicking");
    }

    #[test]
    fn a_pattern_that_does_not_compile_keeps_nothing_rather_than_everything() {
        assert!(heaviest(fixture(), "core::[").is_empty());
    }

    #[test]
    fn records_round_trip_through_json() {
        let records = fixture();
        let json = serde_json::to_string(&records).unwrap();
        let restored: Vec<Record> = serde_json::from_str(&json).unwrap();
        assert_eq!(records, restored);
    }
}
