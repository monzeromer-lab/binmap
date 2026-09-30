//! The web configuration matrix (`TOOLING-WEB §5`).
//!
//! Phase 0's machinery ports directly, and so does its central discipline:
//! **never mutate the user's config files.** On the native side that meant
//! driving cargo through `CARGO_PROFILE_*` environment variables rather than
//! editing `Cargo.toml`; here it means CLI flags, because a bundler accepts
//! them and the working tree stays untouched. A sweep that edited
//! `vite.config.ts` and crashed would leave someone's project broken.
//!
//! `target` is the axis to take seriously. `§5` calls browserslist "the web
//! analogue of `opt-level`: one setting with an enormous, poorly understood
//! effect that nobody measures on their own code", and down-levelling is
//! exactly that — the difference between `esnext` and `es2015` is polyfills
//! and transpiled syntax across every file, and almost nobody has measured it
//! on their own bundle.

use serde::{Deserialize, Serialize};

/// How far down the language is transpiled.
///
/// Ordered oldest first, so `>` means "assumes a newer browser and therefore
/// emits less".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Target {
    Es2015,
    Es2017,
    Es2020,
    Es2022,
    EsNext,
}

impl Target {
    pub fn flag(self) -> &'static str {
        match self {
            Target::Es2015 => "es2015",
            Target::Es2017 => "es2017",
            Target::Es2020 => "es2020",
            Target::Es2022 => "es2022",
            Target::EsNext => "esnext",
        }
    }

    /// What choosing this costs the user, in browsers rather than in bytes.
    ///
    /// The trade a size tool must not hide: the smallest bundle here is the
    /// one that excludes the most people, and that is a product decision
    /// rather than a build setting.
    pub fn audience(self) -> &'static str {
        match self {
            Target::Es2015 => "works on very old browsers, including IE11-era engines",
            Target::Es2017 => "works on browsers from about 2017 onward",
            Target::Es2020 => "works on browsers from about 2020 onward",
            Target::Es2022 => "works on browsers from about 2022 onward",
            Target::EsNext => "assumes a current browser and transpiles nothing",
        }
    }

    pub const ALL: [Target; 5] =
        [Target::Es2015, Target::Es2017, Target::Es2020, Target::Es2022, Target::EsNext];
}

/// One point in the matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebConfiguration {
    pub target: Target,
    pub minify: bool,
    /// Off is worth measuring rather than assumed: a wrong `sideEffects: false`
    /// in a dependency is a correctness bug, and the difference between the two
    /// builds is how much that setting is actually buying.
    pub tree_shaking: bool,
    pub splitting: bool,
    /// `NODE_ENV=production`, which enables dead-code elimination inside many
    /// dependencies and is frequently the largest single lever in the matrix.
    pub production: bool,
}

impl Default for WebConfiguration {
    /// What a normal production build does.
    ///
    /// The baseline every candidate is compared against, and — as on the
    /// native side — it asserts nothing the bundler would not have done
    /// anyway. A baseline that quietly turned something off would make every
    /// number in the product flattering.
    fn default() -> Self {
        Self {
            target: Target::EsNext,
            minify: true,
            tree_shaking: true,
            splitting: false,
            production: true,
        }
    }
}

impl WebConfiguration {
    /// The esbuild flags this configuration becomes.
    ///
    /// Flags rather than a generated config file, so the working tree is never
    /// touched (`§5`).
    pub fn flags(&self) -> Vec<String> {
        let mut flags = vec![format!("--target={}", self.target.flag())];
        if self.minify {
            flags.push("--minify".into());
        }
        // esbuild tree-shakes by default, so only the negative needs saying.
        if !self.tree_shaking {
            flags.push("--tree-shaking=false".into());
        }
        if self.splitting {
            flags.push("--splitting".into());
        }
        flags.push(format!(
            "--define:process.env.NODE_ENV=\"{}\"",
            if self.production { "production" } else { "development" }
        ));
        flags
    }

    /// A stable, readable name.
    ///
    /// Used as a directory name and a session key, so it has to be filesystem
    /// safe and stable across runs.
    pub fn name(&self) -> String {
        format!(
            "{}-{}-{}-{}-{}",
            self.target.flag(),
            if self.minify { "min" } else { "nomin" },
            if self.tree_shaking { "shake" } else { "noshake" },
            if self.splitting { "split" } else { "nosplit" },
            if self.production { "prod" } else { "dev" }
        )
    }

    /// The sentence shown beside the measurement.
    pub fn describe(&self) -> String {
        format!(
            "target={} minify={} tree-shaking={} splitting={} NODE_ENV={}",
            self.target.flag(),
            self.minify,
            self.tree_shaking,
            self.splitting,
            if self.production { "production" } else { "development" }
        )
    }

    /// Whether this configuration is one anybody would actually ship.
    ///
    /// The matrix contains combinations that are only useful as measurements:
    /// an unminified development build is not a candidate, it is a control.
    /// Ranking it against real candidates would put a 400 KB bundle at the
    /// bottom of a list of shippable options and make the list untrustworthy.
    pub fn is_shippable(&self) -> bool {
        self.minify && self.production
    }
}

/// Which axes to sweep.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebMatrix {
    pub targets: Vec<Target>,
    pub minify: Vec<bool>,
    pub tree_shaking: Vec<bool>,
    pub splitting: Vec<bool>,
    pub production: Vec<bool>,
}

impl Default for WebMatrix {
    /// The axes worth sweeping by default.
    ///
    /// Every target, because that is the axis nobody measures and the one most
    /// likely to surprise. The rest are held at what a real build uses, with
    /// tree shaking swept because knowing what it buys is worth one extra
    /// build. Unminified and development builds are *not* in the default
    /// matrix: they are controls, and sweeping them doubles the cost to learn
    /// something everyone already knows.
    fn default() -> Self {
        Self {
            targets: Target::ALL.to_vec(),
            minify: vec![true],
            tree_shaking: vec![true, false],
            splitting: vec![false, true],
            production: vec![true],
        }
    }
}

impl WebMatrix {
    /// Every configuration, in a stable order.
    pub fn configurations(&self) -> Vec<WebConfiguration> {
        let mut all = Vec::new();
        for target in &self.targets {
            for minify in &self.minify {
                for tree_shaking in &self.tree_shaking {
                    for splitting in &self.splitting {
                        for production in &self.production {
                            all.push(WebConfiguration {
                                target: *target,
                                minify: *minify,
                                tree_shaking: *tree_shaking,
                                splitting: *splitting,
                                production: *production,
                            });
                        }
                    }
                }
            }
        }
        all
    }

    /// How many builds this asks for.
    pub fn cardinality(&self) -> usize {
        self.targets.len().max(1)
            * self.minify.len().max(1)
            * self.tree_shaking.len().max(1)
            * self.splitting.len().max(1)
            * self.production.len().max(1)
    }

    /// Just the targets, for the one-axis sweep that answers the question most
    /// people actually have.
    pub fn targets_only() -> Self {
        Self {
            targets: Target::ALL.to_vec(),
            minify: vec![true],
            tree_shaking: vec![true],
            splitting: vec![false],
            production: vec![true],
        }
    }
}
