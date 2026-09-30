//! Attribution from source maps (`TOOLING-WEB §2`).
//!
//! The technique is `source-map-explorer`'s: walk every mapping segment, work
//! out the byte span of generated output each one covers, and attribute those
//! bytes to the original source it points at. It has no module identity and
//! cannot say why a module is in the bundle, but it needs nothing except a map
//! — which is why `§3.4` makes it the tier that always works.
//!
//! The decoding is `sourcemap`'s, not ours. `§2.1` is blunt about why: index
//! maps, relative resolution and one-versus-zero-based line conventions across
//! producers are exactly the edge cases a hand-rolled VLQ decoder gets wrong,
//! and getting them wrong produces confident nonsense rather than an error.
//!
//! What is ours is everything after the decode, and the traps live there:
//!
//! - A segment's span runs to the *next* segment, so the last segment on a line
//!   runs to the end of that line. Getting this wrong silently loses the tail
//!   of every line.
//! - Bytes covered by no segment are not attributed to the nearest source, they
//!   are attributed to nothing and reported as unattributed. `§2.3` says
//!   mappings are coarse after minification; inventing an owner for the gap
//!   would turn that coarseness into a false precision.
//! - Lines are counted in *bytes*, not characters, because the question is how
//!   many bytes an asset costs.

use crate::tier::Tier;
use binmap_core::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// What one original source costs in the generated output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceCost {
    /// The path as the map records it, resolved against `sourceRoot`.
    pub source: String,
    pub bytes: u64,
    /// How many mapping segments pointed here. A source with many small
    /// segments is scattered through the bundle; one with few large segments
    /// is contiguous, and that difference matters when deciding what to split.
    pub segments: u32,
}

/// Everything a map says about where an asset's bytes came from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceAttribution {
    /// Largest first.
    pub sources: Vec<SourceCost>,
    /// Bytes covered by no mapping segment.
    ///
    /// Never zero in practice: the bundler's own runtime, module wrappers and
    /// inter-module glue are generated rather than authored, so they map to
    /// nothing. Reporting it is the honest alternative to spreading it over
    /// the sources that happen to sit nearby.
    pub unattributed_bytes: u64,
    pub total_bytes: u64,
    /// Always `SourceMaps` from this module, carried so a caller that mixes
    /// tiers cannot lose track of which produced what.
    pub tier: Tier,
    /// Whether the map carried its own `sourcesContent`.
    ///
    /// `§2.3`: a finding based on source read from disk rather than from the
    /// map is weaker, because the file on disk may not be the file that was
    /// built. Say which was used.
    pub has_embedded_sources: bool,
}

impl SourceAttribution {
    /// What fraction of the asset was traced to a source.
    ///
    /// The headline honesty number for this tier: an attribution covering 40%
    /// of a bundle is a different thing from one covering 95%, and a reader
    /// deciding whether to act should see which they have.
    pub fn coverage(&self) -> f64 {
        if self.total_bytes == 0 {
            return 0.0;
        }
        (self.total_bytes - self.unattributed_bytes) as f64 / self.total_bytes as f64
    }

    /// The sentence shown beside the result.
    pub fn describe(&self) -> String {
        format!(
            "{} of {} bytes traced to {} sources ({:.1}% covered); {} bytes are bundler glue \
             that maps to no original file. Source text was {}.",
            self.total_bytes - self.unattributed_bytes,
            self.total_bytes,
            self.sources.len(),
            self.coverage() * 100.0,
            self.unattributed_bytes,
            if self.has_embedded_sources { "embedded in the map" } else { "not embedded" }
        )
    }
}

/// Attribute `generated` to its original sources using `map`.
///
/// `generated` is the built asset's bytes and `map` is its source map, as JSON.
pub fn attribute(generated: &[u8], map: &[u8]) -> Result<SourceAttribution> {
    // Checked before decoding, because `SourceMap::from_reader` accepts any
    // JSON object and hands back an empty map. A bundle attributed from
    // `{"nope": true}` then reports "nothing traced", which reads as a bundle
    // with no sources rather than as the wrong file being passed.
    validate(map)?;

    let decoded = sourcemap::SourceMap::from_reader(map).map_err(|error| {
        Error::Other(format!(
            "this is not a source map we can read: {error}. Index maps from multi-pass builds \
             are one known cause (TOOLING-WEB §2.3)."
        ))
    })?;

    // Byte offset of the start of each line, so a (line, column) can become an
    // absolute offset. Columns in a source map are counted in UTF-16 code
    // units by the spec, but every producer in practice emits ASCII-safe
    // minified output where that equals bytes; where it does not, the span is
    // still bounded by the line, so an error stays local rather than
    // cascading.
    let line_starts = line_starts(generated);
    let total_bytes = generated.len() as u64;

    // Collect every segment as an absolute offset, then sort: segments arrive
    // grouped by generated line, and the span of one runs to whichever comes
    // next in the file.
    let mut points: Vec<(usize, Option<u32>)> = Vec::new();
    for token in decoded.tokens() {
        let offset = offset_of(&line_starts, token.get_dst_line(), token.get_dst_col(), generated);
        // `has_source` rather than comparing against the `!0` sentinel: a
        // segment may legitimately map to no source, and treating u32::MAX as
        // a source index would invent one.
        points.push((offset, token.has_source().then(|| token.get_src_id())));
    }
    points.sort_by_key(|(offset, _)| *offset);
    points.dedup_by_key(|(offset, _)| *offset);

    let mut bytes_by_source: BTreeMap<u32, (u64, u32)> = BTreeMap::new();
    let mut attributed = 0u64;

    for (index, (offset, source)) in points.iter().enumerate() {
        // The span runs to the next segment, or to the end of the asset for
        // the last one. Stopping at the end of the line instead would silently
        // lose the tail of every line in a minified bundle, where one line can
        // be the whole file.
        let end = points.get(index + 1).map(|(next, _)| *next).unwrap_or(generated.len());
        let span = end.saturating_sub(*offset) as u64;

        // Bytes before the first segment belong to nobody, and so do bytes a
        // segment explicitly maps to no source.
        if let Some(source) = source {
            let entry = bytes_by_source.entry(*source).or_insert((0, 0));
            entry.0 += span;
            entry.1 += 1;
            attributed += span;
        }
    }

    let mut sources: Vec<SourceCost> = bytes_by_source
        .into_iter()
        .map(|(id, (bytes, segments))| SourceCost {
            source: decoded
                .get_source(id)
                .map(str::to_string)
                // A segment pointing at a source index the map does not have
                // is a broken map, not a source called "unknown". Naming it as
                // such keeps it visible rather than merging it into a real
                // file's total.
                .unwrap_or_else(|| format!("<source {id}, which this map does not name>")),
            bytes,
            segments,
        })
        .collect();
    sources.sort_by_key(|source| std::cmp::Reverse(source.bytes));

    Ok(SourceAttribution {
        sources,
        unattributed_bytes: total_bytes.saturating_sub(attributed),
        total_bytes,
        tier: Tier::SourceMaps,
        has_embedded_sources: (0..decoded.get_source_count())
            .any(|index| decoded.get_source_contents(index).is_some()),
    })
}

/// Refuse something that is not a source map.
///
/// Source Map v3 requires `version: 3` and either `mappings` (a flat map) or
/// `sections` (an index map from a multi-pass build). Anything else is a file
/// someone passed by mistake, and saying so beats reporting an empty
/// attribution as though it were an answer.
fn validate(map: &[u8]) -> Result<()> {
    let value: serde_json::Value = serde_json::from_slice(map).map_err(|error| {
        Error::Other(format!("this is not a source map we can read: it is not JSON ({error})"))
    })?;

    let version = value.get("version").and_then(serde_json::Value::as_u64);
    let has_mappings = value.get("mappings").is_some();
    let has_sections = value.get("sections").is_some();

    if version == Some(3) && (has_mappings || has_sections) {
        return Ok(());
    }

    Err(Error::Other(format!(
        "this is not a source map we can read: expected a version 3 map with `mappings`, found \
         version {} with {}. Index maps from multi-pass builds are one known cause \
         (TOOLING-WEB §2.3).",
        version.map(|v| v.to_string()).unwrap_or_else(|| "none".into()),
        if has_sections {
            "`sections`"
        } else if has_mappings {
            "`mappings`"
        } else {
            "neither `mappings` nor `sections`"
        }
    )))
}

/// Byte offset of the start of every line.
fn line_starts(bytes: &[u8]) -> Vec<usize> {
    let mut starts = vec![0usize];
    for (index, byte) in bytes.iter().enumerate() {
        if *byte == b'\n' {
            starts.push(index + 1);
        }
    }
    starts
}

/// Absolute byte offset of a (line, column) position.
///
/// Clamped to the line, because a column past the end of its line is a
/// producer bug (`§2.3`'s off-by-one trap) and letting it run into the next
/// line would attribute one source's bytes to another.
fn offset_of(line_starts: &[usize], line: u32, column: u32, generated: &[u8]) -> usize {
    let Some(start) = line_starts.get(line as usize) else {
        return generated.len();
    };
    let line_end = line_starts.get(line as usize + 1).copied().unwrap_or(generated.len());
    (start + column as usize).min(line_end)
}
