//! The leaf. Nothing depends on anything below this.

/// One key/value pair from a configuration line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Setting {
    pub key: String,
    pub value: String,
}

/// Parse `key = value` lines, ignoring blanks and `#` comments.
///
/// Deliberately small: the corpus needs a workspace with real shape, not a
/// second product.
pub fn parse(text: &str) -> Vec<Setting> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| Setting {
            key: key.trim().to_string(),
            value: value.trim().to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comments_and_blank_lines_are_not_settings() {
        let parsed = parse("# a comment\n\nopt-level = 3\n  lto = fat  \n");
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0], Setting { key: "opt-level".into(), value: "3".into() });
        assert_eq!(parsed[1].value, "fat");
    }

    #[test]
    fn a_line_without_an_equals_is_skipped_rather_than_guessed_at() {
        assert!(parse("just some words").is_empty());
    }
}
