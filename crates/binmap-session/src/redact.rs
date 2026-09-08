//! The redaction pass that runs on export (`U13`).
//!
//! A session artifact is something a user attaches to a bug report. It is full
//! of absolute paths from their home directory, the environment their build
//! ran under, and whatever their compiler happened to print. The pass is
//! deliberately conservative in one direction and loud in the other: it would
//! rather redact a harmless path than leak a token, and it reports every
//! substitution it made so nothing is quietly different from what the tool saw.
//!
//! It is not a security boundary. It is a courtesy that catches the obvious
//! cases, and the report exists so the user can check the rest themselves.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One kind of thing the pass removes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Redaction {
    /// The user's home directory, replaced with `~`.
    HomeDirectory,
    /// A value that looks like a credential.
    Secret,
    /// The machine's hostname.
    Hostname,
    /// The user's account name where it appears outside a path.
    Username,
}

impl Redaction {
    pub fn label(self) -> &'static str {
        match self {
            Redaction::HomeDirectory => "home directory paths",
            Redaction::Secret => "values that look like credentials",
            Redaction::Hostname => "the machine's hostname",
            Redaction::Username => "the account name",
        }
    }

    fn placeholder(self) -> &'static str {
        match self {
            Redaction::HomeDirectory => "~",
            Redaction::Secret => "[redacted]",
            Redaction::Hostname => "[host]",
            Redaction::Username => "[user]",
        }
    }
}

/// What the pass changed, counted by kind.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedactionReport {
    pub counts: BTreeMap<String, usize>,
}

impl RedactionReport {
    pub fn is_empty(&self) -> bool {
        self.counts.is_empty()
    }

    fn record(&mut self, redaction: Redaction, occurrences: usize) {
        if occurrences > 0 {
            *self.counts.entry(redaction.label().to_string()).or_insert(0) += occurrences;
        }
    }

    /// The sentence the export dialog shows.
    pub fn describe(&self) -> String {
        if self.counts.is_empty() {
            return "Nothing needed redacting.".to_string();
        }
        let parts: Vec<String> =
            self.counts.iter().map(|(what, count)| format!("{count} × {what}")).collect();
        format!("Redacted: {}.", parts.join(", "))
    }
}

/// The pass, configured with what to look for on this machine.
#[derive(Debug, Clone, Default)]
pub struct Redactor {
    home: Option<String>,
    username: Option<String>,
    hostname: Option<String>,
}

impl Redactor {
    /// Build a redactor from this machine's environment.
    pub fn from_environment() -> Self {
        Self {
            home: std::env::var("HOME").ok().filter(|home| home.len() > 1),
            username: std::env::var("USER").ok().filter(|user| user.len() > 2),
            hostname: std::fs::read_to_string("/etc/hostname")
                .ok()
                .map(|name| name.trim().to_string())
                .filter(|name| name.len() > 2),
        }
    }

    pub fn with_home(mut self, home: impl Into<String>) -> Self {
        self.home = Some(home.into());
        self
    }

    pub fn with_username(mut self, username: impl Into<String>) -> Self {
        self.username = Some(username.into());
        self
    }

    pub fn with_hostname(mut self, hostname: impl Into<String>) -> Self {
        self.hostname = Some(hostname.into());
        self
    }

    /// Redact one string, counting what was changed.
    pub fn redact(&self, text: &str, report: &mut RedactionReport) -> String {
        let mut out = text.to_string();

        // Home first: a path under the home directory should come out as `~/…`
        // rather than have its username replaced mid-path.
        if let Some(home) = &self.home {
            let occurrences = out.matches(home.as_str()).count();
            if occurrences > 0 {
                out = out.replace(home.as_str(), Redaction::HomeDirectory.placeholder());
                report.record(Redaction::HomeDirectory, occurrences);
            }
        }

        let secrets = redact_secrets(&out);
        out = secrets.0;
        report.record(Redaction::Secret, secrets.1);

        for (value, kind) in
            [(&self.hostname, Redaction::Hostname), (&self.username, Redaction::Username)]
        {
            if let Some(value) = value {
                let occurrences = out.matches(value.as_str()).count();
                if occurrences > 0 {
                    out = out.replace(value.as_str(), kind.placeholder());
                    report.record(kind, occurrences);
                }
            }
        }

        out
    }
}

/// Words that mark the value after them as one nobody should read.
const SECRET_KEYS: [&str; 8] =
    ["token", "secret", "password", "passwd", "api_key", "apikey", "auth", "credential"];

/// Replace the value in any `KEY=value` or `"key": "value"` pair whose key
/// looks like a credential.
///
/// Line-oriented and case-insensitive on the key. It will not catch a secret
/// printed without a label, which is why the report exists.
fn redact_secrets(text: &str) -> (String, usize) {
    let mut count = 0;
    let mut lines = Vec::new();

    for line in text.lines() {
        let Some(separator) = line.find(['=', ':']) else {
            lines.push(line.to_string());
            continue;
        };
        let key = line[..separator].to_ascii_lowercase();
        let looks_secret = SECRET_KEYS.iter().any(|marker| key.contains(marker));
        if !looks_secret || line[separator + 1..].trim().is_empty() {
            lines.push(line.to_string());
            continue;
        }

        // Keep the key and the shape of the line; replace only the value.
        let value = &line[separator + 1..];
        let trailing = value.len() - value.trim_end().len();
        let mut redacted = String::with_capacity(line.len());
        redacted.push_str(&line[..=separator]);
        let leading: String =
            value.chars().take_while(|c| c.is_whitespace() || *c == '"').collect();
        redacted.push_str(&leading);
        redacted.push_str(Redaction::Secret.placeholder());
        if value.trim_end().ends_with('"') {
            redacted.push('"');
        }
        redacted.push_str(&value[value.len() - trailing..]);
        lines.push(redacted);
        count += 1;
    }

    let mut joined = lines.join("\n");
    if text.ends_with('\n') && !joined.ends_with('\n') {
        joined.push('\n');
    }
    (joined, count)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn redactor() -> Redactor {
        Redactor::default()
            .with_home("/home/ada")
            .with_username("ada")
            .with_hostname("difference-engine")
    }

    #[test]
    fn home_paths_become_tilde_rather_than_losing_their_username_mid_path() {
        let mut report = RedactionReport::default();
        let out = redactor().redact("built /home/ada/projects/app/target/release/app", &mut report);
        assert_eq!(out, "built ~/projects/app/target/release/app");
        assert_eq!(report.counts.get("home directory paths"), Some(&1));
    }

    #[test]
    fn a_labelled_secret_loses_its_value_and_keeps_its_shape() {
        let mut report = RedactionReport::default();
        let out = redactor().redact(
            "CARGO_REGISTRY_TOKEN=cio1234567890abcdef\nRUSTFLAGS=-Copt-level=3",
            &mut report,
        );
        assert_eq!(out, "CARGO_REGISTRY_TOKEN=[redacted]\nRUSTFLAGS=-Copt-level=3");
        assert_eq!(report.counts.get("values that look like credentials"), Some(&1));
    }

    #[test]
    fn a_json_style_secret_is_caught_too() {
        let mut report = RedactionReport::default();
        let out = redactor().redact(r#"  "api_key": "sk-live-abcdef""#, &mut report);
        assert_eq!(out, r#"  "api_key": "[redacted]""#);
    }

    #[test]
    fn the_hostname_and_account_name_go_even_outside_a_path() {
        let mut report = RedactionReport::default();
        let out = redactor().redact("running as ada on difference-engine", &mut report);
        assert_eq!(out, "running as [user] on [host]");
    }

    #[test]
    fn an_ordinary_build_log_is_left_alone_and_says_so() {
        let mut report = RedactionReport::default();
        let text = "   Compiling serde v1.0.229\n    Finished `release` profile in 12.4s";
        assert_eq!(redactor().redact(text, &mut report), text);
        assert!(report.is_empty());
        assert_eq!(report.describe(), "Nothing needed redacting.");
    }

    #[test]
    fn the_report_counts_every_kind_it_changed() {
        let mut report = RedactionReport::default();
        redactor().redact("/home/ada/a\n/home/ada/b\nTOKEN=xyz", &mut report);
        assert_eq!(report.counts.get("home directory paths"), Some(&2));
        assert!(report.describe().starts_with("Redacted: "));
    }
}
