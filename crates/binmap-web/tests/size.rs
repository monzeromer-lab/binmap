//! Transfer size (`TOOLING-WEB §4`).

use binmap_web::size::{CompressionSettings, Ranking, measure, rank};
use proptest::prelude::*;

/// Text that compresses well, as real JavaScript does.
fn some_javascript(repeats: usize) -> Vec<u8> {
    "export function renderTheThing(value) { return value.toString(); }\n"
        .repeat(repeats)
        .into_bytes()
}

#[test]
fn text_compresses_and_both_encoders_agree_it_is_smaller() {
    let bytes = some_javascript(200);
    let size = measure(&bytes, CompressionSettings::default()).expect("measurable");

    assert_eq!(size.raw, bytes.len() as u64);
    assert!(size.gzip < size.raw, "gzip {} vs raw {}", size.gzip, size.raw);
    assert!(size.brotli < size.raw, "brotli {} vs raw {}", size.brotli, size.raw);
    assert_eq!(size.transfer(), size.gzip.min(size.brotli));
}

#[test]
fn the_settings_travel_with_the_number() {
    // A number without its settings is a number that cannot be checked, and
    // §4 is explicit that the settings decide the answer.
    let size = measure(b"const x = 1;", CompressionSettings::default()).unwrap();
    assert_eq!(size.settings, CompressionSettings::default());
    assert!(size.settings.describe().contains("brotli quality 11"), "{}", size.settings.describe());
}

#[test]
fn brotli_quality_changes_the_answer_enough_to_matter() {
    // The reason §4 requires the settings to be exposed: a claim measured at
    // quality 11 and served at quality 4 is wrong, and not by a rounding
    // error.
    let bytes = some_javascript(400);
    let precompressed = measure(&bytes, CompressionSettings::default()).unwrap();
    let on_the_fly = measure(&bytes, CompressionSettings::on_the_fly()).unwrap();

    assert_eq!(precompressed.raw, on_the_fly.raw, "raw size does not depend on settings");
    assert!(
        on_the_fly.brotli >= precompressed.brotli,
        "quality 4 should not beat quality 11: {} vs {}",
        on_the_fly.brotli,
        precompressed.brotli
    );
}

#[test]
fn an_empty_asset_is_not_a_division_by_zero() {
    let size = measure(b"", CompressionSettings::default()).unwrap();
    assert_eq!(size.raw, 0);
    assert_eq!(size.compression_ratio(), 1.0, "nothing compresses to nothing, not to NaN");
    assert!(!size.looks_incompressible(), "an empty asset is not an image");
}

#[test]
fn already_compressed_data_is_recognised_as_such() {
    // An inlined image or a WASM blob barely compresses, and shrinking it
    // needs a different move than shrinking code. Saying so saves the reader
    // from trying the wrong thing.
    // xorshift64, because a multiply-and-shift leaves enough structure that
    // gzip found it and compressed to 3% — the first version of this test
    // asserted the opposite of what its own data did.
    let mut state = 0x2545_F491_4F6C_DD1Du64;
    let incompressible: Vec<u8> = (0..20_000)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state as u8
        })
        .collect();
    let size = measure(&incompressible, CompressionSettings::default()).unwrap();
    assert!(
        size.looks_incompressible(),
        "ratio was {:.3}, which should read as already compressed",
        size.compression_ratio()
    );

    let text = measure(&some_javascript(200), CompressionSettings::default()).unwrap();
    assert!(
        !text.looks_incompressible(),
        "javascript compresses, ratio {:.3}",
        text.compression_ratio()
    );
}

#[test]
fn a_tiny_asset_is_never_called_incompressible() {
    // Compression headers dominate at small sizes, so a short string can
    // "grow". Reporting that as an already-compressed asset would be noise.
    let size = measure(b"const a=1;", CompressionSettings::default()).unwrap();
    assert!(!size.looks_incompressible());
}

// --- the disagreement that justifies measuring three numbers (§4.1) ---------

fn sized(raw: u64, gzip: u64, brotli: u64) -> binmap_web::TransferSize {
    binmap_web::TransferSize { raw, gzip, brotli, settings: CompressionSettings::default() }
}

#[test]
fn smaller_on_disk_and_larger_over_the_network_is_named_as_the_trap_it_is() {
    // The counterintuitive result §4.1 says no existing tool measures. A
    // reader shown only raw sizes would ship the worse one believing it
    // better.
    let baseline = sized(100_000, 30_000, 25_000);
    let candidate = sized(95_000, 32_000, 27_000);

    let ranking = rank(&baseline, &candidate);
    assert_eq!(ranking, Ranking::RawSmallerTransferLarger);
    assert!(ranking.is_a_disagreement());
    assert!(ranking.describe().contains("compression locality"), "{}", ranking.describe());
}

#[test]
fn larger_on_disk_and_smaller_over_the_network_is_surfaced_too() {
    // A win that a raw-bytes tool would have discarded.
    let baseline = sized(100_000, 30_000, 25_000);
    let candidate = sized(105_000, 28_000, 23_000);

    let ranking = rank(&baseline, &candidate);
    assert_eq!(ranking, Ranking::RawLargerTransferSmaller);
    assert!(ranking.describe().contains("the number the user actually pays"));
}

#[test]
fn agreeing_measurements_are_not_reported_as_a_disagreement() {
    // Flagging every comparison would make the flag meaningless.
    let baseline = sized(100_000, 30_000, 25_000);
    for candidate in [sized(90_000, 27_000, 22_000), sized(110_000, 33_000, 28_000)] {
        let ranking = rank(&baseline, &candidate);
        assert_eq!(ranking, Ranking::Agree, "candidate {candidate:?}");
        assert!(!ranking.is_a_disagreement());
    }
}

#[test]
fn an_identical_measurement_is_not_a_disagreement() {
    let baseline = sized(100_000, 30_000, 25_000);
    assert_eq!(rank(&baseline, &baseline), Ranking::Agree);
}

#[test]
fn a_change_on_only_one_axis_is_not_a_disagreement() {
    // Same transfer size, smaller raw: nothing to warn about, because nobody
    // pays more.
    let baseline = sized(100_000, 30_000, 25_000);
    let same_transfer = sized(95_000, 30_000, 25_000);
    assert_eq!(rank(&baseline, &same_transfer), Ranking::Agree);
}

proptest! {
    /// Measuring never panics and never claims compression grew the raw size.
    #[test]
    fn measuring_arbitrary_bytes_is_well_behaved(bytes in proptest::collection::vec(any::<u8>(), 0..8192)) {
        let size = measure(&bytes, CompressionSettings::default()).expect("measurable");
        prop_assert_eq!(size.raw, bytes.len() as u64);
        prop_assert!(size.transfer() > 0 || bytes.is_empty());
        prop_assert!((0.0..=f64::MAX).contains(&size.compression_ratio()));
    }

    /// Any quality is accepted, including ones brotli would reject.
    ///
    /// The settings come from a user-editable field, so a quality of 99 must
    /// clamp rather than panic deep inside the encoder.
    #[test]
    fn out_of_range_settings_clamp_rather_than_panic(
        gzip_level in 0u32..64,
        brotli_quality in 0u32..64,
        brotli_window in 0u32..64,
    ) {
        let settings = CompressionSettings { gzip_level, brotli_quality, brotli_window };
        let size = measure(b"export const value = 42;\n", settings).expect("measurable");
        prop_assert!(size.gzip > 0);
        prop_assert!(size.brotli > 0);
    }
}
