//! Where a finding is, in terms no backend owns.
//!
//! A location is artifact-neutral on purpose: a Rust symbol, a JavaScript
//! chunk and a managed type all reduce to the same shape, so the views that
//! render them are written once.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A position in the artifact, the source, or both.
///
/// Every variant is optional-by-omission rather than optional-by-`None`: a
/// backend that cannot answer a question does not answer it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Location {
    /// The unit the artifact divides into: a crate, a package, an assembly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,

    /// The module path within that unit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,

    /// The symbol, demangled. The mangled form belongs in evidence, not here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,

    /// The source span, where the backend can map to one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceSpan>,

    /// The address within the artifact, where the artifact has addresses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<u64>,

    /// The configuration a measurement was taken under, when the finding is
    /// about a build rather than a place in one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configuration: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSpan {
    pub file: PathBuf,
    pub line: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
}

impl Location {
    pub fn nowhere() -> Self {
        Self::default()
    }

    pub fn configuration(name: impl Into<String>) -> Self {
        Self { configuration: Some(name.into()), ..Self::default() }
    }

    pub fn symbol(name: impl Into<String>) -> Self {
        Self { symbol: Some(name.into()), ..Self::default() }
    }

    pub fn with_unit(mut self, unit: impl Into<String>) -> Self {
        self.unit = Some(unit.into());
        self
    }

    pub fn with_source(mut self, file: impl Into<PathBuf>, line: u32) -> Self {
        self.source = Some(SourceSpan { file: file.into(), line, column: None });
        self
    }

    /// The one-line form the interface shows when it has room for one line.
    pub fn describe(&self) -> String {
        if let Some(symbol) = &self.symbol {
            return symbol.clone();
        }
        if let Some(configuration) = &self.configuration {
            return configuration.clone();
        }
        if let Some(source) = &self.source {
            return format!("{}:{}", source.file.display(), source.line);
        }
        match (&self.unit, &self.module) {
            (Some(unit), Some(module)) => format!("{unit}::{module}"),
            (Some(unit), None) => unit.clone(),
            (None, Some(module)) => module.clone(),
            (None, None) => "the artifact".to_string(),
        }
    }
}
