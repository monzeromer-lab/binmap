//! What a target can and cannot support, declared rather than discovered at
//! the point of use.
//!
//! The rule the interface follows: a view a target cannot support is *absent*
//! from the nav rail rather than present and empty, and a blocked action
//! states its reason rather than disappearing (§2.5). Those are different
//! failures and they get different treatment — a missing view is a property of
//! the target, a blocked action is a property of the session.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Capability {
    /// The build system can sweep a configuration matrix.
    ConfigurationSweep,
    /// Bytes can be attributed to units of the artifact.
    SizeAttribution,
    /// Compressed sizes are meaningful, because the artifact is served
    /// compressed.
    CompressedSize,
    /// Symbols map to source spans.
    SourceMapping,
    /// Generic instantiations can be grouped.
    Monomorphization,
    /// Core dumps can be loaded and unwound.
    CrashAnalysis,
    /// The artifact can be disassembled.
    Disassembly,
    /// Sampling profiles can be collected and attributed.
    PerformanceAttribution,
    /// Load-time phases can be measured. Web only.
    LoadTime,
    /// Execution can be recorded and replayed.
    ReplayDebugging,
}

impl Capability {
    /// The plain-words form the project view uses when it states what a target
    /// can do. Sentence fragments, not identifiers.
    pub fn describe(self) -> &'static str {
        match self {
            Capability::ConfigurationSweep => "sweep build configurations",
            Capability::SizeAttribution => "attribute size to crates and functions",
            Capability::CompressedSize => "report gzip and brotli sizes",
            Capability::SourceMapping => "map the artifact back to source",
            Capability::Monomorphization => "group generic instantiations",
            Capability::CrashAnalysis => "analyse core dumps",
            Capability::Disassembly => "disassemble",
            Capability::PerformanceAttribution => "attribute samples to source lines",
            Capability::LoadTime => "measure load time",
            Capability::ReplayDebugging => "record and replay execution",
        }
    }
}

impl std::fmt::Display for Capability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.describe())
    }
}

/// The capabilities one target declares.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Capabilities(BTreeSet<Capability>);

impl Capabilities {
    pub fn none() -> Self {
        Self::default()
    }

    pub fn has(&self, capability: Capability) -> bool {
        self.0.contains(&capability)
    }

    pub fn iter(&self) -> impl Iterator<Item = Capability> + '_ {
        self.0.iter().copied()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The sentence the project view puts under a target's name.
    pub fn sentence(&self) -> String {
        let mut parts: Vec<&str> = self.iter().map(Capability::describe).collect();
        match parts.len() {
            0 => "Nothing Binmap can measure yet.".to_string(),
            1 => format!("Binmap can {}.", parts[0]),
            _ => {
                let last = parts.pop().expect("checked above");
                format!("Binmap can {} and {}.", parts.join(", "), last)
            }
        }
    }

    /// Require a capability, producing the error that names what is missing.
    pub fn require(&self, capability: Capability) -> crate::Result<()> {
        if self.has(capability) { Ok(()) } else { Err(crate::Error::Unsupported(capability)) }
    }
}

impl FromIterator<Capability> for Capabilities {
    fn from_iter<T: IntoIterator<Item = Capability>>(iter: T) -> Self {
        Self(iter.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capabilities_read_as_a_sentence() {
        let one: Capabilities = [Capability::ConfigurationSweep].into_iter().collect();
        assert_eq!(one.sentence(), "Binmap can sweep build configurations.");

        let several: Capabilities =
            [Capability::ConfigurationSweep, Capability::SizeAttribution, Capability::Disassembly]
                .into_iter()
                .collect();
        assert_eq!(
            several.sentence(),
            "Binmap can sweep build configurations, attribute size to crates and functions and disassemble."
        );

        assert_eq!(Capabilities::none().sentence(), "Nothing Binmap can measure yet.");
    }

    #[test]
    fn a_missing_capability_names_itself() {
        let capabilities: Capabilities = [Capability::SizeAttribution].into_iter().collect();
        let error = capabilities.require(Capability::ReplayDebugging).unwrap_err();
        assert!(error.to_string().contains("record and replay execution"));
    }
}
