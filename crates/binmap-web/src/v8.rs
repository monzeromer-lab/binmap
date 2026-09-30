//! V8 CPU profiles, mapped back to TypeScript (`F3.2` for web targets).
//!
//! A `.cpuprofile` is a tree of call frames plus a list of sample indices into
//! it — the same idea as a native profile, shaped differently. What makes it
//! useful here is that its frames carry a **url, line and column** in the
//! *generated* bundle, which is exactly what a source map translates. So the
//! Phase 1.5 attribution layer does for a profile what it already does for
//! bytes.
//!
//! Three things the format gets wrong if read naively, and all three are in a
//! real profile from `node --cpu-prof`:
//!
//! - **`hitCount` is self time, and most nodes have none.** A tree where 60 of
//!   67 nodes have zero hits is normal; ranking by hit count without summing
//!   children gives a list of leaves and no structure.
//! - **Line and column are zero-based**, while every stack trace a JavaScript
//!   developer has ever read is one-based for lines. Off by one in the
//!   direction that looks plausible.
//! - **`(root)`, `(program)`, `(idle)` and `(garbage collector)` are not
//!   functions.** They are V8's own accounting, and leaving them in the
//!   ranking puts "(program)" at the top of a profile of someone's code.

use binmap_core::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One frame, as V8 records it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
struct CallFrame {
    #[serde(rename = "functionName", default)]
    function_name: String,
    #[serde(default)]
    url: String,
    /// Zero-based, and `-1` for V8's synthetic frames.
    #[serde(rename = "lineNumber", default)]
    line_number: i64,
    #[serde(rename = "columnNumber", default)]
    column_number: i64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
struct RawNode {
    id: u32,
    #[serde(rename = "callFrame")]
    call_frame: CallFrame,
    #[serde(rename = "hitCount", default)]
    hit_count: u64,
    #[serde(default)]
    children: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
struct RawProfile {
    nodes: Vec<RawNode>,
    #[serde(default)]
    samples: Vec<u32>,
}

/// V8's own accounting entries, which are not functions anyone wrote.
///
/// Left in the ranking they put "(program)" at the top of a profile of
/// someone's code, which is true and useless — it is V8 telling you it was
/// busy.
const SYNTHETIC: [&str; 5] =
    ["(root)", "(program)", "(idle)", "(garbage collector)", "(anonymous)"];

fn is_synthetic(name: &str) -> bool {
    SYNTHETIC.contains(&name) || name.is_empty()
}

/// One function's share of a V8 profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hot {
    /// V8's name, or `(anonymous)` where the function has none.
    pub function: String,
    /// Where it is in the *generated* bundle.
    pub generated: Option<String>,
    /// Where it is in the original source, once a map has been applied.
    ///
    /// `None` until mapping, and `None` after it for a frame the map does not
    /// cover — node's own internals, for instance, which have no source map
    /// and are not the reader's code anyway.
    pub original: Option<String>,
    /// One-based, as every JavaScript stack trace a developer has read.
    pub line: Option<u32>,
    pub self_samples: u64,
    pub total_samples: u64,
}

impl Hot {
    pub fn self_share(&self, total: u64) -> f64 {
        if total == 0 {
            return 0.0;
        }
        self.self_samples as f64 / total as f64
    }

    /// Whether this is the reader's own code rather than node's or a
    /// dependency's.
    ///
    /// `§3` of the plan: "your-code-only on by default", because a profile of
    /// a node program is mostly node.
    pub fn is_yours(&self) -> bool {
        let Some(location) = self.original.as_deref().or(self.generated.as_deref()) else {
            return false;
        };
        !location.starts_with("node:")
            && !location.contains("node_modules/")
            && !location.starts_with("internal/")
    }

    pub fn describe(&self, total: u64) -> String {
        let location =
            self.original.as_deref().or(self.generated.as_deref()).unwrap_or("<unknown>");
        let line = self.line.map(|line| format!(":{line}")).unwrap_or_default();
        format!(
            "{} at {location}{line} — {:.1}% self, {} samples",
            self.function,
            self.self_share(total) * 100.0,
            self.self_samples
        )
    }
}

/// A V8 profile, read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct V8Profile {
    /// Hottest by self time first.
    pub hot: Vec<Hot>,
    pub total_samples: u64,
    /// Samples V8 attributed to its own accounting rather than to a function.
    ///
    /// Reported rather than dropped: a profile that hides them shows
    /// percentages that do not add up and gives no clue why.
    pub synthetic_samples: u64,
}

impl V8Profile {
    /// Only the reader's own code.
    pub fn yours(&self) -> Vec<&Hot> {
        self.hot.iter().filter(|hot| hot.is_yours()).collect()
    }

    pub fn describe(&self) -> String {
        format!(
            "{} samples across {} functions; {} in V8's own accounting",
            self.total_samples,
            self.hot.len(),
            self.synthetic_samples
        )
    }
}

/// Read a `.cpuprofile`.
pub fn read(profile: &[u8]) -> Result<V8Profile> {
    let raw: RawProfile = serde_json::from_slice(profile).map_err(|error| {
        Error::Other(format!(
            "this is not a V8 CPU profile: {error}. Produce one with `node --cpu-prof`."
        ))
    })?;

    if raw.nodes.is_empty() {
        return Err(Error::Other(
            "this profile has no nodes in it, so there is nothing to attribute.".into(),
        ));
    }

    let by_id: BTreeMap<u32, &RawNode> = raw.nodes.iter().map(|node| (node.id, node)).collect();

    // `hitCount` is self time. Where `samples` is present it is the authority —
    // it is the actual sequence, and a profile can carry both.
    let mut self_counts: BTreeMap<u32, u64> = BTreeMap::new();
    if raw.samples.is_empty() {
        for node in &raw.nodes {
            if node.hit_count > 0 {
                self_counts.insert(node.id, node.hit_count);
            }
        }
    } else {
        for id in &raw.samples {
            *self_counts.entry(*id).or_insert(0) += 1;
        }
    }

    let total: u64 = self_counts.values().sum();

    // Total time: a node's own hits plus every descendant's. Computed by
    // walking down from each node rather than up, because the tree gives
    // children and not parents.
    fn subtree(
        id: u32,
        by_id: &BTreeMap<u32, &RawNode>,
        self_counts: &BTreeMap<u32, u64>,
        seen: &mut std::collections::BTreeSet<u32>,
    ) -> u64 {
        // A malformed profile can point a child at an ancestor. Without this
        // the walk never returns.
        if !seen.insert(id) {
            return 0;
        }
        let Some(node) = by_id.get(&id) else { return 0 };
        let mut total = self_counts.get(&id).copied().unwrap_or(0);
        for child in &node.children {
            total += subtree(*child, by_id, self_counts, seen);
        }
        total
    }

    // Functions, not nodes: one function appears at many nodes, once per call
    // path, and ranking nodes ranks call paths.
    let mut by_function: BTreeMap<(String, String, i64), (u64, u64)> = BTreeMap::new();
    let mut synthetic = 0u64;

    for node in &raw.nodes {
        let frame = &node.call_frame;
        let self_samples = self_counts.get(&node.id).copied().unwrap_or(0);

        if is_synthetic(&frame.function_name) && frame.url.is_empty() {
            synthetic += self_samples;
            continue;
        }

        let mut seen = std::collections::BTreeSet::new();
        let total_samples = subtree(node.id, &by_id, &self_counts, &mut seen);

        let key = (frame.function_name.clone(), frame.url.clone(), frame.line_number);
        let entry = by_function.entry(key).or_insert((0, 0));
        entry.0 += self_samples;
        entry.1 = entry.1.max(total_samples);
    }

    let mut hot: Vec<Hot> = by_function
        .into_iter()
        .map(|((function, url, line), (self_samples, total_samples))| Hot {
            function: if function.is_empty() { "(anonymous)".into() } else { function },
            generated: (!url.is_empty()).then(|| url.clone()),
            original: None,
            // V8 counts lines from zero and every JavaScript stack trace a
            // developer has read counts from one. Off by one in the direction
            // that looks entirely plausible.
            line: (line >= 0).then(|| line as u32 + 1),
            self_samples,
            total_samples,
        })
        .collect();

    hot.sort_by(|left, right| {
        right
            .self_samples
            .cmp(&left.self_samples)
            .then(right.total_samples.cmp(&left.total_samples))
            .then(left.function.cmp(&right.function))
    });

    Ok(V8Profile { hot, total_samples: total, synthetic_samples: synthetic })
}

/// Map a profile's frames back through a source map.
///
/// `lookup` takes a generated (line, column), both zero-based as V8 records
/// them, and returns the original file and line. Injected so this is testable
/// without a bundle, and so a caller with several bundles picks the right map
/// per frame.
pub fn map_through<F>(profile: &mut V8Profile, mut lookup: F)
where
    F: FnMut(&str, u32, u32) -> Option<(String, u32)>,
{
    for hot in &mut profile.hot {
        let Some(url) = hot.generated.clone() else { continue };
        // Back to zero-based for the lookup: the map speaks V8's numbering,
        // not the reader's.
        let Some(line) = hot.line.map(|line| line.saturating_sub(1)) else { continue };
        if let Some((file, original_line)) = lookup(&url, line, 0) {
            hot.original = Some(file);
            hot.line = Some(original_line);
        }
    }
}
