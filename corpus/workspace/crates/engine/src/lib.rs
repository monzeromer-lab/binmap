//! The middle of the graph: depends on `parser`, is depended on by `cli`.

use parser::Setting;
use std::collections::BTreeMap;

/// Settings resolved into a map, with later lines winning.
pub fn resolve(text: &str) -> BTreeMap<String, String> {
    let mut resolved = BTreeMap::new();
    for Setting { key, value } in parser::parse(text) {
        resolved.insert(key, value);
    }
    resolved
}

/// A one-line summary, in a stable order.
pub fn describe(text: &str) -> String {
    resolve(text)
        .into_iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_later_line_wins() {
        assert_eq!(resolve("a = 1\na = 2\n").get("a").map(String::as_str), Some("2"));
    }

    #[test]
    fn the_summary_is_ordered_so_it_can_be_compared() {
        assert_eq!(describe("b = 2\na = 1\n"), "a=1 b=2");
    }
}
