//! Finding a web project and what it can be asked (`TOOLING-WEB §9`).
//!
//! Discovery has to be honest about two separate things, and conflating them
//! is the easy mistake. A project *has* a bundler configured — that is a fact
//! about `package.json`. A project *has been built* — that is a fact about the
//! filesystem, and it decides what can be attributed right now. Reporting the
//! first as though it were the second gives a target that looks ready and then
//! produces nothing.
//!
//! The capability set follows from what is actually on disk, which is why this
//! returns a target carrying its own detection rather than a bare path.

use crate::tier::{Detection, Tier};
use binmap_core::capability::{Capabilities, Capability};
use binmap_core::error::{Error, Result};
use binmap_core::traits::{Target, TargetFamily};
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// The parts of `package.json` we read.
#[derive(Debug, Clone, Deserialize)]
struct PackageJson {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    scripts: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    dependencies: std::collections::BTreeMap<String, String>,
    #[serde(rename = "devDependencies", default)]
    dev_dependencies: std::collections::BTreeMap<String, String>,
}

impl PackageJson {
    /// Which bundler this project uses, from its dependencies and scripts.
    ///
    /// Both are checked because neither alone is reliable: a project can
    /// depend on vite and build with esbuild directly, and a project can call
    /// a bundler through a script without declaring it.
    fn bundler(&self) -> Option<Bundler> {
        let declared = |name: &str| {
            self.dependencies.contains_key(name) || self.dev_dependencies.contains_key(name)
        };
        let scripted = |name: &str| {
            self.scripts.values().any(|script| script.split_whitespace().any(|word| word == name))
        };

        // Ordered by how good the metadata is, so a project with several gets
        // the best one rather than whichever was checked first.
        for (name, bundler) in [
            ("esbuild", Bundler::Esbuild),
            ("webpack", Bundler::Webpack),
            ("vite", Bundler::Vite),
            ("rollup", Bundler::Rollup),
        ] {
            if declared(name) || scripted(name) {
                return Some(bundler);
            }
        }
        None
    }
}

/// Which bundler a project builds with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bundler {
    Esbuild,
    Webpack,
    Vite,
    Rollup,
}

impl Bundler {
    pub fn label(self) -> &'static str {
        match self {
            Bundler::Esbuild => "esbuild",
            Bundler::Webpack => "webpack",
            Bundler::Vite => "Vite",
            Bundler::Rollup => "Rollup",
        }
    }

    /// The best tier this bundler could provide, if asked correctly.
    ///
    /// What it *could* give, not what is on disk — those are different
    /// questions and only the second decides what can be measured now.
    pub fn best_possible_tier(self) -> Tier {
        match self {
            Bundler::Esbuild => Tier::EsbuildMetafile,
            Bundler::Webpack => Tier::WebpackStats,
            Bundler::Vite | Bundler::Rollup => Tier::BundleGraph,
        }
    }

    /// How to make it emit that metadata.
    ///
    /// Named as a command because `§9` wants the remedy stated, the same way
    /// the environment panel states a missing tool's exact fix.
    pub fn how_to_emit_metadata(self) -> &'static str {
        match self {
            Bundler::Esbuild => "add `--metafile=dist/meta.json` to the esbuild command",
            Bundler::Webpack => "run webpack with `--json > stats.json`",
            Bundler::Vite => "set `build.rollupOptions.output.sourcemap` and keep the manifest",
            Bundler::Rollup => "keep the bundle graph with `--sourcemap`",
        }
    }
}

/// A built asset we can measure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    pub path: PathBuf,
    /// Its source map, when one sits beside it.
    pub map: Option<PathBuf>,
}

/// A discovered web project.
#[derive(Debug, Clone)]
pub struct WebProject {
    pub root: PathBuf,
    pub name: String,
    pub bundler: Option<Bundler>,
    /// Where the build landed, if it has been built.
    pub output_directory: Option<PathBuf>,
    pub assets: Vec<Asset>,
    pub metafile: Option<PathBuf>,
    pub detection: Detection,
}

impl WebProject {
    /// What this project can be asked, given what is actually on disk.
    ///
    /// Deliberately derived from the filesystem rather than from
    /// `package.json`: a project that declares esbuild but has never been
    /// built cannot be attributed, and a target claiming otherwise would fail
    /// at the moment someone clicked it.
    pub fn capabilities(&self) -> Capabilities {
        // Always: a web asset is served compressed, so transfer size is
        // meaningful even with no metadata at all, and the configuration sweep
        // ports straight across from Phase 0 (`§5`).
        let mut offered = vec![Capability::CompressedSize, Capability::ConfigurationSweep];

        if self.detection.chosen.attributes_anything() {
            offered.push(Capability::SizeAttribution);
        }
        if self.detection.chosen >= Tier::SourceMaps {
            // A source map is exactly a generated-to-source mapping.
            offered.push(Capability::SourceMapping);
        }

        // Never offered, and that is the honest part: there is no machine code
        // to disassemble, no unwinding, no core dumps, and no generics to
        // group. `§1` says the entire hard half of the native backend is
        // absent, and the capability set is where a user is told that rather
        // than finding out by clicking something that fails.
        offered.into_iter().collect()
    }

    /// The target the project view lists.
    pub fn target(&self) -> Target {
        Target {
            id: format!("web::{}", self.name),
            name: self.name.clone(),
            family: TargetFamily::TypeScript,
            package: self.name.clone(),
            kind: "bundle".into(),
            manifest: self.root.join("package.json"),
            capabilities: self.capabilities(),
        }
    }

    /// What was found, and what was not, in one sentence.
    pub fn describe(&self) -> String {
        let bundler = match self.bundler {
            Some(bundler) => bundler.label(),
            None => "no bundler detected",
        };
        // The tier's *label* only. The full explanation is its own line, and
        // printing both put the same paragraph on screen twice.
        format!(
            "{} · {bundler} · {} assets · {}",
            self.name,
            self.assets.len(),
            self.detection.chosen.label()
        )
    }
}

/// Find the web project at `root`.
pub fn discover(root: &Path) -> Result<WebProject> {
    let manifest = root.join("package.json");
    let raw = std::fs::read(&manifest).map_err(|error| {
        Error::Other(format!(
            "no package.json at {}: {error}. A web project is identified by one.",
            root.display()
        ))
    })?;
    let package: PackageJson = serde_json::from_slice(&raw).map_err(|error| {
        Error::Other(format!("{} is not readable JSON: {error}", manifest.display()))
    })?;

    let name = package
        .name
        .clone()
        .or_else(|| root.file_name().map(|name| name.to_string_lossy().to_string()))
        .unwrap_or_else(|| "web".to_string());
    let bundler = package.bundler();

    // Where a build lands, in the order these tools default to.
    let output_directory = ["dist", "build", "out", ".next", "public"]
        .iter()
        .map(|name| root.join(name))
        .find(|path| path.is_dir());

    let (assets, metafile) = match &output_directory {
        Some(directory) => (collect_assets(directory), find_metafile(directory)),
        None => (Vec::new(), None),
    };

    let detection = detect(bundler, &output_directory, &assets, metafile.as_deref());

    Ok(WebProject {
        root: root.to_path_buf(),
        name,
        bundler,
        output_directory,
        assets,
        metafile,
        detection,
    })
}

/// Which tier is available, and why the better ones are not.
fn detect(
    bundler: Option<Bundler>,
    output_directory: &Option<PathBuf>,
    assets: &[Asset],
    metafile: Option<&Path>,
) -> Detection {
    if output_directory.is_none() {
        let mut detection = Detection::new(Tier::AssetSizesOnly);
        if let Some(bundler) = bundler {
            detection = detection.rejecting(
                bundler.best_possible_tier(),
                "this project has not been built, so there is nothing on disk to attribute",
            );
        }
        return detection;
    }

    if metafile.is_some() {
        return Detection::new(Tier::EsbuildMetafile);
    }

    let mapped = assets.iter().any(|asset| asset.map.is_some());
    let mut detection =
        Detection::new(if mapped { Tier::SourceMaps } else { Tier::AssetSizesOnly });

    if let Some(bundler) = bundler {
        detection = detection.rejecting(
            bundler.best_possible_tier(),
            format!(
                "no build metadata was found beside the bundle — {}",
                bundler.how_to_emit_metadata()
            ),
        );
    }
    if !mapped {
        detection = detection.rejecting(
            Tier::SourceMaps,
            "no source maps were found beside the assets, so bytes cannot be traced to a file",
        );
    }
    detection
}

/// Every JavaScript asset in a directory, with its map where one exists.
///
/// One level deep only. A `dist` with nested route directories is common, but
/// so is a `dist/assets` full of images, and walking everything to find them
/// costs more than it returns for a first pass.
fn collect_assets(directory: &Path) -> Vec<Asset> {
    let Ok(entries) = std::fs::read_dir(directory) else { return Vec::new() };
    let mut assets: Vec<Asset> = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter(|path| {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            // `.map` is not an asset a user downloads, and `.d.ts` is not
            // shipped at all.
            (name.ends_with(".js") || name.ends_with(".mjs") || name.ends_with(".css"))
                && !name.ends_with(".d.ts")
        })
        .map(|path| {
            let map = path.with_extension(format!(
                "{}.map",
                path.extension().unwrap_or_default().to_string_lossy()
            ));
            Asset { map: map.is_file().then_some(map), path }
        })
        .collect();
    assets.sort_by(|left, right| left.path.cmp(&right.path));
    assets
}

/// esbuild's metafile, under whichever name it was written.
fn find_metafile(directory: &Path) -> Option<PathBuf> {
    ["meta.json", "metafile.json", "esbuild-meta.json"]
        .iter()
        .map(|name| directory.join(name))
        .find(|path| path.is_file())
}
