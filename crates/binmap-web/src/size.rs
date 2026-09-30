//! Three numbers, not one (`TOOLING-WEB §4`).
//!
//! Raw bytes are nearly irrelevant to a web user: what costs them latency is
//! what crosses the network, and that is compressed. So every size here is a
//! triple, and each one records *how* it was compressed — because the settings
//! usually do not match the defaults, and "a size claim under brotli 11 that a
//! user's CDN serves at quality 4 is a wrong number delivered confidently".
//!
//! The reason this is worth building rather than bolting on: **compressed size
//! is not proportional to raw size**. Removing raw bytes can *increase* the
//! gzipped total by breaking compression locality — deduplicating a repeated
//! string literal that was compressing almost for free, or reordering modules
//! so similar code no longer sits inside the compressor's window. Across a
//! sweep that means the ranking by raw bytes and the ranking by transfer bytes
//! can disagree, and `§4.1` says no existing tool shows that. Showing it is the
//! point, so `Ranking` exists to name the disagreement rather than leave a
//! reader to notice it.

use serde::{Deserialize, Serialize};
use std::io::Write;

/// How a measurement was compressed.
///
/// Carried with every number rather than assumed, because the settings decide
/// the answer and the defaults are usually wrong for what actually ships.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompressionSettings {
    /// gzip level. Most CDNs use 6.
    pub gzip_level: u32,
    /// brotli quality. 11 for precompressed static assets, 4–5 on the fly —
    /// and the gap between those two is large enough to change a decision.
    pub brotli_quality: u32,
    /// brotli window bits.
    pub brotli_window: u32,
}

impl Default for CompressionSettings {
    /// What a CDN serving precompressed assets typically does.
    ///
    /// A default, and labelled as an assumption wherever it is shown. `§4`
    /// requires this to be exposed as a setting precisely because the default
    /// is a guess about someone else's infrastructure.
    fn default() -> Self {
        Self { gzip_level: 6, brotli_quality: 11, brotli_window: 22 }
    }
}

impl CompressionSettings {
    /// What a server compressing on the fly typically does.
    ///
    /// Offered as a named alternative because the difference between this and
    /// the default is the single most common way a transfer-size claim turns
    /// out to be wrong.
    pub fn on_the_fly() -> Self {
        Self { gzip_level: 6, brotli_quality: 4, brotli_window: 22 }
    }

    /// The sentence shown beside every number measured with these.
    pub fn describe(&self) -> String {
        format!(
            "gzip level {}, brotli quality {} window {}",
            self.gzip_level, self.brotli_quality, self.brotli_window
        )
    }
}

/// What one asset costs, three ways.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferSize {
    pub raw: u64,
    pub gzip: u64,
    pub brotli: u64,
    pub settings: CompressionSettings,
}

impl TransferSize {
    /// The number a web user actually pays.
    ///
    /// Brotli where it is smaller, which it nearly always is. Named rather
    /// than left to the caller so that "the size" means one thing across the
    /// product.
    pub fn transfer(&self) -> u64 {
        self.gzip.min(self.brotli)
    }

    /// How much of the raw size survives compression, as a fraction.
    ///
    /// Useful on its own: an asset that barely compresses is usually already
    /// compressed — an inlined image or a WASM blob — and shrinking it needs a
    /// different move than shrinking code.
    pub fn compression_ratio(&self) -> f64 {
        if self.raw == 0 {
            return 1.0;
        }
        self.transfer() as f64 / self.raw as f64
    }

    /// Whether this asset looks like it is already compressed.
    ///
    /// A heuristic and named as one. Text compresses to roughly a third or
    /// less; something that only loses a tenth was not text.
    pub fn looks_incompressible(&self) -> bool {
        self.raw > 1024 && self.compression_ratio() > 0.9
    }
}

/// Measure one asset, three ways.
pub fn measure(bytes: &[u8], settings: CompressionSettings) -> std::io::Result<TransferSize> {
    Ok(TransferSize {
        raw: bytes.len() as u64,
        gzip: gzip_size(bytes, settings.gzip_level)?,
        brotli: brotli_size(bytes, settings.brotli_quality, settings.brotli_window),
        settings,
    })
}

fn gzip_size(bytes: &[u8], level: u32) -> std::io::Result<u64> {
    let mut encoder =
        flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::new(level.min(9)));
    encoder.write_all(bytes)?;
    Ok(encoder.finish()?.len() as u64)
}

fn brotli_size(bytes: &[u8], quality: u32, window: u32) -> u64 {
    let mut out = Vec::new();
    {
        let mut writer =
            brotli::CompressorWriter::new(&mut out, 4096, quality.min(11), window.clamp(10, 24));
        // Writing to a `Vec` does not fail, and neither does the flush on drop.
        let _ = writer.write_all(bytes);
    }
    out.len() as u64
}

/// Whether two measurements agree about which is smaller (`§4.1`).
///
/// The whole reason for measuring three numbers. A configuration that removes
/// raw bytes can add transfer bytes, and a reader shown only raw sizes would
/// ship the worse one believing it was better.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ranking {
    /// Both say the same thing.
    Agree,
    /// Raw says smaller, transfer says larger. The trap.
    RawSmallerTransferLarger,
    /// Raw says larger, transfer says smaller. Also worth surfacing: it is a
    /// win a raw-bytes tool would have discarded.
    RawLargerTransferSmaller,
}

impl Ranking {
    /// Whether this disagreement is worth telling someone about.
    pub fn is_a_disagreement(self) -> bool {
        !matches!(self, Ranking::Agree)
    }

    /// The sentence shown when the rankings disagree.
    pub fn describe(self) -> &'static str {
        match self {
            Ranking::Agree => "raw and transfer size agree about which is smaller",
            Ranking::RawSmallerTransferLarger => {
                "this is smaller on disk but larger over the network — removing raw bytes broke \
                 compression locality, and the user pays the transfer size"
            }
            Ranking::RawLargerTransferSmaller => {
                "this is larger on disk but smaller over the network, which is the number the \
                 user actually pays"
            }
        }
    }
}

/// Compare a candidate against a baseline on both axes.
pub fn rank(baseline: &TransferSize, candidate: &TransferSize) -> Ranking {
    let raw_smaller = candidate.raw < baseline.raw;
    let transfer_smaller = candidate.transfer() < baseline.transfer();

    match (raw_smaller, transfer_smaller) {
        (true, false) if candidate.transfer() != baseline.transfer() => {
            Ranking::RawSmallerTransferLarger
        }
        (false, true) if candidate.raw != baseline.raw => Ranking::RawLargerTransferSmaller,
        _ => Ranking::Agree,
    }
}
