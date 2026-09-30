//! The JavaScript and TypeScript backend (Phase 1.5).
//!
//! `TOOLING-WEB §1` is the useful frame: everything here is the web equivalent
//! of something in the native backend, and what makes this backend cheap is
//! what is *absent*. There is no disassembly, no unwinding, no core dumps and
//! no record-and-replay — the entire hard half of the native backend does not
//! apply. Source maps do DWARF's job, a bundler metafile does the symbol
//! table's, and size is three numbers because a web user pays for transfer
//! rather than for disk.
//!
//! The capability model carries that honestly rather than pretending: a web
//! target simply does not offer `Disassembly`, and the interface says so
//! instead of offering a button that fails.

pub mod configuration;
pub mod metafile;
pub mod project;
pub mod size;
pub mod sourcemap;
pub mod sweep;
pub mod tier;

pub use size::{CompressionSettings, Ranking, TransferSize, measure, rank};
pub use tier::{Detection, Tier};
