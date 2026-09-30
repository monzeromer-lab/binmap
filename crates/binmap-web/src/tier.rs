//! Which technique produced an attribution (`TOOLING-WEB §3.4`).
//!
//! The detection order runs from bundler metadata down to raw asset sizes, and
//! the rule is short: *always report which tier was used.* An attribution from
//! source maps alone is genuinely less precise than one from an esbuild
//! metafile — it has no module identity and cannot say *why* a module is in the
//! bundle — and the interface must not present the two identically.
//!
//! This is the web backend's version of the same honesty the native backend
//! applies to an inferred symbol size. The number is real either way; what
//! differs is how much it is entitled to claim.

use serde::{Deserialize, Serialize};

/// How an attribution was produced, best first.
///
/// Ordered so that `>` means "more precise", which is what the detection
/// strategy needs: take the best tier available and say which it was.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Tier {
    /// Raw asset sizes only. No attribution at all.
    AssetSizesOnly,
    /// Source maps. Always available, no module identity.
    SourceMaps,
    /// A Rollup or Vite bundle graph.
    BundleGraph,
    /// webpack's `stats.json` — the best answer to "why is this here", and the
    /// most expensive to parse.
    WebpackStats,
    /// An esbuild metafile. Module identity, imports, and why each module is
    /// in the bundle.
    EsbuildMetafile,
}

impl Tier {
    pub fn label(self) -> &'static str {
        match self {
            Tier::AssetSizesOnly => "asset sizes only",
            Tier::SourceMaps => "source maps",
            Tier::BundleGraph => "bundle graph",
            Tier::WebpackStats => "webpack stats",
            Tier::EsbuildMetafile => "esbuild metafile",
        }
    }

    /// What this tier can and cannot say, in the terms a reader needs.
    ///
    /// Shown beside the result rather than in documentation, because the
    /// limitation matters at the moment someone is about to act on a number.
    pub fn describe(self) -> &'static str {
        match self {
            Tier::AssetSizesOnly => {
                "No attribution: there is no source map or bundler metadata here, so these are \
                 file sizes and nothing more. Build with source maps to see where the bytes \
                 came from."
            }
            Tier::SourceMaps => {
                "Attributed from source maps. Every byte is traced to an original file, but \
                 there is no module identity and no import graph — this cannot say *why* a \
                 module is in the bundle, only that it is."
            }
            Tier::BundleGraph => {
                "Attributed from the bundler's own module graph, so module identity and imports \
                 are real rather than inferred from paths."
            }
            Tier::WebpackStats => {
                "Attributed from webpack's stats, which knows why each module was included — \
                 the most complete answer available, and the slowest to produce."
            }
            Tier::EsbuildMetafile => {
                "Attributed from esbuild's metafile: module identity, imports, and why each \
                 module is in the bundle, from the bundler itself."
            }
        }
    }

    /// Whether this tier knows why a module is in the bundle.
    ///
    /// The question a size tool is really asked, and the one source maps
    /// cannot answer at all.
    pub fn explains_inclusion(self) -> bool {
        matches!(self, Tier::EsbuildMetafile | Tier::WebpackStats)
    }

    /// Whether modules have real identity rather than paths guessed from a
    /// source map's `sources` array.
    pub fn has_module_identity(self) -> bool {
        self >= Tier::BundleGraph
    }

    /// Whether anything is attributed at all.
    pub fn attributes_anything(self) -> bool {
        self > Tier::AssetSizesOnly
    }

    /// The confidence ceiling a finding from this tier may carry.
    ///
    /// The same rule the native backend applies to inferred symbol sizes: a
    /// measurement is a measurement, but a measurement that had to guess at
    /// module boundaries cannot be Certain about them.
    pub fn ceiling(self) -> binmap_core::finding::Confidence {
        use binmap_core::finding::Confidence;
        match self {
            // The bundler told us. There is nothing to infer.
            Tier::EsbuildMetafile | Tier::WebpackStats => Confidence::Certain,
            Tier::BundleGraph => Confidence::High,
            // Real mappings, but coarse after minification (`§2.3`), and
            // module identity is inferred from paths.
            Tier::SourceMaps => Confidence::High,
            Tier::AssetSizesOnly => Confidence::Probable,
        }
    }
}

/// The best tier available, and what was tried.
///
/// Carries the rejected options so the interface can say "esbuild metafile not
/// found, fell back to source maps" rather than silently producing a weaker
/// answer — `§2.3` makes the same point about a missing source map: say so
/// rather than degrading quietly.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Detection {
    pub chosen: Tier,
    /// Why each better tier was unavailable, best first.
    pub rejected: Vec<(Tier, String)>,
}

impl Detection {
    pub fn new(chosen: Tier) -> Self {
        Self { chosen, rejected: Vec::new() }
    }

    pub fn rejecting(mut self, tier: Tier, why: impl Into<String>) -> Self {
        self.rejected.push((tier, why.into()));
        self
    }

    /// The full sentence: what was used, and what was not available.
    pub fn describe(&self) -> String {
        let mut line = self.chosen.describe().to_string();
        if let Some((tier, why)) = self.rejected.first() {
            line.push_str(&format!(" ({} was not used: {why})", tier.label()));
        }
        line
    }
}
