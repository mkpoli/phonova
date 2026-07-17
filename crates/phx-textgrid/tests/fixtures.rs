//! Fixture-driven coverage: round-trip every well-formed TextGrid and confirm
//! every malformed one is rejected without a panic.
//!
//! The fixture directory is globbed rather than hard-coded so that files landing
//! from the fixture-expansion lane are exercised automatically. The `malformed/`
//! subdirectory is skipped by the round-trip tests and drives the error paths.

use phx_annot::Tier;
use phx_textgrid::{Encoding, TextGridError, Variant, read, write};
use std::fs;
use std::path::{Path, PathBuf};

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/textgrids")
}

fn is_binary(path: &Path) -> bool {
    fs::read(path)
        .map(|bytes| bytes.starts_with(b"ooBinaryFile"))
        .unwrap_or(false)
}

fn text_grid_files() -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(fixture_dir())
        .expect("fixture directory is readable")
        .map(|entry| entry.expect("directory entry is readable").path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("TextGrid"))
        })
        .collect();
    files.sort();
    files
}

/// Text-format fixtures the reader supports (binary-format fixtures excluded).
fn well_formed_fixtures() -> Vec<PathBuf> {
    let files: Vec<PathBuf> = text_grid_files()
        .into_iter()
        .filter(|path| !is_binary(path))
        .collect();
    assert!(
        !files.is_empty(),
        "expected at least one text-format TextGrid fixture"
    );
    files
}

fn binary_fixtures() -> Vec<PathBuf> {
    text_grid_files()
        .into_iter()
        .filter(|p| is_binary(p))
        .collect()
}

fn malformed_fixtures() -> Vec<PathBuf> {
    let dir = fixture_dir().join("malformed");
    if !dir.is_dir() {
        return Vec::new();
    }
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .expect("malformed directory is readable")
        .map(|entry| entry.expect("directory entry is readable").path())
        .filter(|path| path.is_file())
        .collect();
    files.sort();
    files
}

#[test]
fn every_fixture_round_trips_to_structural_equality() {
    for path in well_formed_fixtures() {
        let bytes = fs::read(&path).expect("fixture bytes are readable");
        let (doc, _info) = read(&bytes).unwrap_or_else(|err| {
            panic!("reading {} failed: {err}", path.display());
        });

        let written = write(&doc).expect("finite document writes");
        let (reparsed, _) = read(&written).unwrap_or_else(|err| {
            panic!("re-reading written {} failed: {err}", path.display());
        });
        assert_eq!(
            doc,
            reparsed,
            "structural round-trip mismatch for {}",
            path.display()
        );

        let rewritten = write(&reparsed).expect("finite document writes");
        assert_eq!(
            written,
            rewritten,
            "write output is not byte-stable for {}",
            path.display()
        );
    }
}

#[test]
fn written_output_is_utf8_without_bom() {
    for path in well_formed_fixtures() {
        let bytes = fs::read(&path).expect("fixture bytes are readable");
        let (doc, _) = read(&bytes).expect("fixture reads");
        let written = write(&doc).expect("finite document writes");
        assert!(
            std::str::from_utf8(&written).is_ok(),
            "written {} is not valid UTF-8",
            path.display()
        );
        assert!(
            !written.starts_with(&[0xEF, 0xBB, 0xBF]),
            "written {} carries a byte-order mark",
            path.display()
        );
    }
}

#[test]
fn utf16_source_reemerges_as_utf8() {
    let path = fixture_dir().join("librispeech_2277-149896-0005_long_utf16.TextGrid");
    let bytes = fs::read(&path).expect("utf16 fixture is readable");
    let (doc, info) = read(&bytes).expect("utf16 fixture reads");
    assert_eq!(info.encoding, Encoding::Utf16Le);
    assert_eq!(info.variant, Variant::Long);

    let written = write(&doc).expect("finite document writes");
    assert!(std::str::from_utf8(&written).is_ok());
    let (reparsed, second) = read(&written).expect("written utf16 source reads");
    assert_eq!(second.encoding, Encoding::Utf8);
    assert_eq!(doc, reparsed);
}

#[test]
fn latin1_legacy_source_decodes() {
    let path = fixture_dir().join("latin1_legacy.TextGrid");
    if !path.is_file() {
        return;
    }
    let bytes = fs::read(&path).expect("latin1 fixture is readable");
    let (doc, info) = read(&bytes).expect("latin1 fixture reads");
    assert_eq!(info.encoding, Encoding::Latin1);
    let written = write(&doc).expect("finite document writes");
    let (reparsed, _) = read(&written).expect("written latin1 source reads");
    assert_eq!(doc, reparsed);
}

#[test]
fn tier_domain_narrower_than_the_document_round_trips() {
    let path = fixture_dir().join("narrow_tier_domain_long_utf8.TextGrid");
    let bytes = fs::read(&path).expect("fixture is readable");
    let (doc, _) = read(&bytes).expect("fixture reads");

    let Tier::Interval(tier) = &doc.tiers()[0].tier else {
        panic!("expected an interval tier");
    };
    assert_eq!(doc.xmin(), 0.0);
    assert_eq!(doc.xmax(), 2.0);
    assert_eq!(tier.xmin, 0.5);
    assert_eq!(tier.xmax, 1.5);

    let written = write(&doc).expect("finite document writes");
    let text = std::str::from_utf8(&written).expect("written output is UTF-8");
    assert!(
        text.contains("xmin = 0.5") && text.contains("xmax = 1.5"),
        "written output did not preserve the tier's own domain: {text}"
    );

    let (reparsed, _) = read(&written).expect("written document reads back");
    assert_eq!(doc, reparsed);
}

#[test]
fn short_and_long_variants_are_detected() {
    let long = fixture_dir().join("arctic_bdl_a0001_long_utf8.TextGrid");
    let (_, info) = read(&fs::read(long).expect("readable")).expect("reads");
    assert_eq!(info.variant, Variant::Long);
    assert_eq!(info.encoding, Encoding::Utf8);

    let short = fixture_dir().join("arctic_slt_a0001_short_utf8.TextGrid");
    let (_, info) = read(&fs::read(short).expect("readable")).expect("reads");
    assert_eq!(info.variant, Variant::Short);
    assert_eq!(info.encoding, Encoding::Utf8);
}

#[test]
fn binary_fixtures_are_rejected_with_a_clear_error() {
    for path in binary_fixtures() {
        let bytes = fs::read(&path).expect("binary fixture is readable");
        assert_eq!(
            read(&bytes),
            Err(TextGridError::BinaryUnsupported),
            "binary fixture {} should be reported as unsupported",
            path.display()
        );
    }
}

#[test]
fn malformed_fixtures_return_errors_without_panicking() {
    let files = malformed_fixtures();
    for path in &files {
        let bytes = fs::read(path).expect("malformed fixture is readable");
        let result = read(&bytes);
        assert!(
            result.is_err(),
            "expected {} to be rejected, but it parsed",
            path.display()
        );
    }
}
