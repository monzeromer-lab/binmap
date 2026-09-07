//! What a built artifact weighs, and what it is made of.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// One named region of the artifact: an ELF section, a bundle chunk, a managed
/// assembly. The name is the backend's, unaltered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Section {
    pub name: String,
    pub bytes: u64,
    /// Whether the section occupies space in the file on disk. `.bss` does not,
    /// and a size report that pretends otherwise is wrong by exactly its size.
    #[serde(default = "yes")]
    pub occupies_file: bool,
}

fn yes() -> bool {
    true
}

/// The size of one built artifact, per section and in total.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactSize {
    pub path: PathBuf,
    /// The file's size on disk. The headline number, because it is the number
    /// the user's disk and their release page both report.
    pub total_bytes: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sections: Vec<Section>,
    /// Set for artifacts that are served compressed; `None` where compression
    /// is not part of how the artifact is delivered, rather than zero.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compressed: Option<CompressedSize>,
}

/// Sizes under the compressions that are actually served, with the settings
/// recorded — a claim under brotli 11 served at quality 4 is a wrong number
/// delivered confidently.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompressedSize {
    pub gzip_bytes: u64,
    pub brotli_bytes: u64,
    pub gzip_level: u32,
    pub brotli_quality: u32,
}

impl ArtifactSize {
    pub fn new(path: impl Into<PathBuf>, total_bytes: u64) -> Self {
        Self { path: path.into(), total_bytes, sections: Vec::new(), compressed: None }
    }

    /// The bytes actually present in the file, summed over sections. Differs
    /// from `total_bytes` by headers and padding, and the difference is
    /// reported rather than hidden.
    pub fn section_bytes(&self) -> u64 {
        self.sections.iter().filter(|s| s.occupies_file).map(|s| s.bytes).sum()
    }

    pub fn section(&self, name: &str) -> Option<&Section> {
        self.sections.iter().find(|s| s.name == name)
    }
}
