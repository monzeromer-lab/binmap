//! Sweeping a web project (`TOOLING-WEB §5`).
//!
//! The same discipline as Phase 0, for the same reason. Every build goes to a
//! directory of ours, driven entirely by CLI flags, and the user's config files
//! and output directory are never touched — a sweep that edited
//! `vite.config.ts` and then crashed would leave someone's project broken, and
//! a sweep that wrote into `dist/` would destroy the build they already had.
//!
//! What is measured is transfer size, not raw size, because that is what a web
//! user pays. And both are kept, because `§4.1`'s disagreement between the two
//! rankings is a finding nobody else produces — a configuration can remove raw
//! bytes and add transfer bytes, and only a tool holding both numbers can say
//! so.

use crate::configuration::{WebConfiguration, WebMatrix};
use crate::size::{CompressionSettings, Ranking, TransferSize, measure, rank};
use binmap_core::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// What one configuration produced.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Measured {
    pub configuration: WebConfiguration,
    /// Every asset the build emitted, summed.
    pub size: TransferSize,
    /// Bytes on the critical path, where the build emitted a metafile.
    ///
    /// `None` without one, and reported as absent rather than as the total —
    /// which would silently claim a split build's lazy chunks are part of the
    /// initial load.
    pub initial_load: Option<u64>,
    pub assets: usize,
}

/// A whole sweep.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebSweep {
    pub baseline: Option<Measured>,
    pub measured: Vec<Measured>,
    pub settings: CompressionSettings,
    /// Configurations that did not build, and why.
    ///
    /// Kept rather than dropped. A configuration that fails is a real answer
    /// about that configuration — `es2015` with syntax esbuild cannot
    /// down-level is worth knowing — and a sweep that silently reported "0
    /// configurations built" with no reason is a sweep nobody can debug. That
    /// is exactly what the first version of this did.
    pub failed: Vec<(WebConfiguration, String)>,
}

impl WebSweep {
    /// The smallest shippable configuration by transfer size.
    ///
    /// Shippable, because the matrix contains controls — an unminified
    /// development build is a measurement, not a candidate, and ranking it
    /// alongside real options would make the list untrustworthy.
    pub fn smallest(&self) -> Option<&Measured> {
        self.measured
            .iter()
            .filter(|m| m.configuration.is_shippable())
            .min_by_key(|m| m.size.transfer())
    }

    /// Where raw and transfer size disagree about which configuration is
    /// smaller (`§4.1`).
    ///
    /// The finding nobody else produces. Each entry is a configuration whose
    /// ranking against the baseline flips depending on which number you look
    /// at, which is precisely the case where a raw-bytes tool misleads.
    pub fn disagreements(&self) -> Vec<(&Measured, Ranking)> {
        let Some(baseline) = &self.baseline else { return Vec::new() };
        self.measured
            .iter()
            .map(|candidate| (candidate, rank(&baseline.size, &candidate.size)))
            .filter(|(_, ranking)| ranking.is_a_disagreement())
            .collect()
    }

    /// How much the best shippable configuration saves, as a fraction of the
    /// baseline's transfer size.
    pub fn best_reduction(&self) -> Option<f64> {
        let baseline = self.baseline.as_ref()?.size.transfer();
        let best = self.smallest()?.size.transfer();
        if baseline == 0 {
            return None;
        }
        Some((baseline as f64 - best as f64) / baseline as f64)
    }

    /// The one-line result.
    pub fn summary(&self) -> String {
        let disagreements = self.disagreements().len();
        let mut line = format!(
            "{} configurations built, measured at {}",
            self.measured.len(),
            self.settings.describe()
        );
        if !self.failed.is_empty() {
            line.push_str(&format!("; {} did not build", self.failed.len()));
        }
        if let Some(reduction) = self.best_reduction() {
            line.push_str(&format!("; the smallest is {:.1}% under baseline", reduction * 100.0));
        }
        if disagreements > 0 {
            line.push_str(&format!(
                "; {disagreements} where raw and transfer size disagree about which is smaller"
            ));
        }
        line
    }
}

/// What to build, and how.
pub struct WebSweepPlan<'a> {
    pub root: &'a Path,
    /// The entry point, relative to the root.
    pub entry: &'a str,
    pub matrix: WebMatrix,
    pub settings: CompressionSettings,
}

impl WebSweepPlan<'_> {
    /// Where our builds go.
    ///
    /// Under the project so a relative entry point still resolves, and in a
    /// directory of our own so the user's `dist/` is never written to. Phase 0
    /// learned this the same way: our target directory is separate precisely
    /// so a sweep cannot disturb a build someone is relying on.
    fn output_root(&self) -> PathBuf {
        // Absolute, because the bundler runs with the project root as its
        // working directory: a relative path would be resolved against the
        // root a *second* time and the build would land in
        // `project/project/node_modules/...`. The native side hit exactly this
        // and writes `project/project/target` in its own comment.
        let root = self.root.canonicalize().unwrap_or_else(|_| self.root.to_path_buf());
        root.join("node_modules").join(".binmap")
    }

    /// Build and measure every configuration.
    ///
    /// `run` runs the bundler: it takes the flags and the output directory and
    /// returns whether the build succeeded. Injected rather than called
    /// directly so the sweep's arithmetic is testable without node installed.
    pub fn run<F>(&self, mut build: F) -> Result<WebSweep>
    where
        F: FnMut(&WebConfiguration, &[String], &Path) -> Result<()>,
    {
        let output_root = self.output_root();
        std::fs::create_dir_all(&output_root).map_err(|error| {
            Error::Other(format!("could not create {}: {error}", output_root.display()))
        })?;

        // The baseline first, and it is measured rather than assumed — the
        // native side shipped a baseline that asserted cargo's defaults
        // wrongly and made every number nine times too flattering.
        let baseline_configuration = WebConfiguration::default();
        let mut failed: Vec<(WebConfiguration, String)> = Vec::new();
        let baseline =
            match self.build_and_measure(&baseline_configuration, &output_root, &mut build) {
                Ok(baseline) => Some(baseline),
                Err(error) => {
                    failed.push((baseline_configuration, error.to_string()));
                    None
                }
            };

        let mut measured = Vec::new();
        for configuration in self.matrix.configurations() {
            if configuration == baseline_configuration {
                if let Some(baseline) = &baseline {
                    measured.push(baseline.clone());
                }
                continue;
            }
            // A configuration that fails to build is not a failure of the
            // sweep: `es2015` with a syntax esbuild cannot down-level is a
            // real answer about that target. It is left out of the results
            // rather than aborting the run.
            match self.build_and_measure(&configuration, &output_root, &mut build) {
                Ok(result) => measured.push(result),
                Err(error) => failed.push((configuration, error.to_string())),
            }
        }

        Ok(WebSweep { baseline, measured, settings: self.settings, failed })
    }

    fn build_and_measure<F>(
        &self,
        configuration: &WebConfiguration,
        output_root: &Path,
        build: &mut F,
    ) -> Result<Measured>
    where
        F: FnMut(&WebConfiguration, &[String], &Path) -> Result<()>,
    {
        let directory = output_root.join(configuration.name());
        // A stale directory from an earlier run would be measured as though it
        // were this one's output.
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).map_err(|error| {
            Error::Other(format!("could not create {}: {error}", directory.display()))
        })?;

        build(configuration, &configuration.flags(), &directory)?;
        self.measure_directory(*configuration, &directory)
    }

    /// Sum every emitted asset, three ways.
    fn measure_directory(
        &self,
        configuration: WebConfiguration,
        directory: &Path,
    ) -> Result<Measured> {
        let entries = std::fs::read_dir(directory).map_err(|error| {
            Error::Other(format!("could not read {}: {error}", directory.display()))
        })?;

        let mut raw = 0u64;
        let mut gzip = 0u64;
        let mut brotli = 0u64;
        let mut assets = 0usize;

        for entry in entries.filter_map(|entry| entry.ok()) {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            // Source maps and the metafile are not page weight, and counting
            // them would make every configuration look several times larger
            // than it is.
            if name.ends_with(".map") || name == "meta.json" {
                continue;
            }

            let bytes = std::fs::read(&path).map_err(|error| {
                Error::Other(format!("could not read {}: {error}", path.display()))
            })?;
            let size = measure(&bytes, self.settings)
                .map_err(|error| Error::Other(format!("could not compress {name}: {error}")))?;
            raw += size.raw;
            gzip += size.gzip;
            brotli += size.brotli;
            assets += 1;
        }

        if assets == 0 {
            return Err(Error::Other(format!(
                "{} built nothing into {}",
                configuration.describe(),
                directory.display()
            )));
        }

        // Per-chunk, where the build left a metafile.
        let initial_load = std::fs::read(directory.join("meta.json"))
            .ok()
            .and_then(|raw| crate::metafile::attribute(&raw).ok())
            .map(|attribution| attribution.initial_load_bytes());

        Ok(Measured {
            configuration,
            size: TransferSize { raw, gzip, brotli, settings: self.settings },
            initial_load,
            assets,
        })
    }
}
