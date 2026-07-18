//! Fixture-driven coverage: round-trip every well-formed TextGrid, confirm
//! every binary sample parses structurally equal to its text-format twin, and
//! confirm every malformed one is rejected without a panic.
//!
//! The fixture directory is globbed rather than hard-coded so that files landing
//! from the fixture-expansion lane are exercised automatically. The `malformed/`
//! subdirectory is skipped by the round-trip tests and drives the error paths.

use phx_annot::Tier;
use phx_textgrid::{Encoding, SourceInfo, TextGridError, Variant, read, write};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Binary fixture filename paired with the text-format fixture that carries
/// the same content, per `tests/fixtures/MANIFEST.md`. Every `*_binary.TextGrid`
/// (and `synthetic_binary_complex.TextGrid`) fixture must appear here exactly
/// once; `every_binary_fixture_has_a_declared_text_twin` enforces that.
const BINARY_TEXT_TWINS: &[(&str, &str)] = &[
    (
        "ipa_diacritics_binary.TextGrid",
        "ipa_diacritics_long_utf8.TextGrid",
    ),
    (
        "adjacent_empty_intervals_binary.TextGrid",
        "adjacent_empty_intervals_long_utf8.TextGrid",
    ),
    (
        "narrow_tier_domain_binary.TextGrid",
        "narrow_tier_domain_long_utf8.TextGrid",
    ),
    (
        "points_only_binary.TextGrid",
        "points_only_long_utf8.TextGrid",
    ),
    (
        "mixed_multitier_binary.TextGrid",
        "mixed_multitier_short_utf8.TextGrid",
    ),
    ("latin1_legacy_binary.TextGrid", "latin1_legacy.TextGrid"),
    (
        "synthetic_binary_complex.TextGrid",
        "synthetic_complex_long_utf8.TextGrid",
    ),
];

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

/// Every non-malformed fixture, text- or binary-format alike; both round-trip
/// through `read` and `write` (which always emits canonical text).
fn well_formed_fixtures() -> Vec<PathBuf> {
    let files = text_grid_files();
    assert!(!files.is_empty(), "expected at least one TextGrid fixture");
    files
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
    assert_eq!(
        info,
        SourceInfo::Text {
            variant: Variant::Long,
            encoding: Encoding::Utf16Le,
        }
    );

    let written = write(&doc).expect("finite document writes");
    assert!(std::str::from_utf8(&written).is_ok());
    let (reparsed, second) = read(&written).expect("written utf16 source reads");
    assert_eq!(
        second,
        SourceInfo::Text {
            variant: Variant::Long,
            encoding: Encoding::Utf8,
        }
    );
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
    assert!(matches!(
        info,
        SourceInfo::Text {
            encoding: Encoding::Latin1,
            ..
        }
    ));
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
    assert_eq!(
        info,
        SourceInfo::Text {
            variant: Variant::Long,
            encoding: Encoding::Utf8,
        }
    );

    let short = fixture_dir().join("arctic_slt_a0001_short_utf8.TextGrid");
    let (_, info) = read(&fs::read(short).expect("readable")).expect("reads");
    assert_eq!(
        info,
        SourceInfo::Text {
            variant: Variant::Short,
            encoding: Encoding::Utf8,
        }
    );
}

#[test]
fn every_binary_fixture_has_a_declared_text_twin() {
    let declared: HashSet<&str> = BINARY_TEXT_TWINS
        .iter()
        .map(|(binary, _)| *binary)
        .collect();
    for path in text_grid_files() {
        if !is_binary(&path) {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("fixture has a UTF-8 filename");
        assert!(
            declared.contains(name),
            "binary fixture {name} has no declared text twin in BINARY_TEXT_TWINS"
        );
    }
}

#[test]
fn every_binary_fixture_parses_structurally_equal_to_its_text_twin() {
    for (binary_name, text_name) in BINARY_TEXT_TWINS {
        let binary_bytes =
            fs::read(fixture_dir().join(binary_name)).expect("binary fixture is readable");
        let text_bytes =
            fs::read(fixture_dir().join(text_name)).expect("text-twin fixture is readable");

        let (binary_doc, binary_info) =
            read(&binary_bytes).unwrap_or_else(|err| panic!("reading {binary_name} failed: {err}"));
        let (text_doc, text_info) =
            read(&text_bytes).unwrap_or_else(|err| panic!("reading {text_name} failed: {err}"));

        assert_eq!(
            binary_info,
            SourceInfo::Binary,
            "{binary_name} should be detected as binary"
        );
        assert!(
            matches!(text_info, SourceInfo::Text { .. }),
            "{text_name} should be detected as text"
        );
        assert_eq!(
            binary_doc, text_doc,
            "{binary_name} did not parse structurally equal to its text twin {text_name}"
        );
    }
}

#[test]
fn binary_fixtures_never_panic_when_truncated_or_bit_flipped() {
    for (binary_name, _) in BINARY_TEXT_TWINS {
        let original =
            fs::read(fixture_dir().join(binary_name)).expect("binary fixture is readable");

        for cut in 0..=original.len() {
            let _ = read(&original[..cut]);
        }

        let mut state: u32 = 0x9e37_79b9 ^ original.len() as u32;
        for _ in 0..200 {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            if original.is_empty() {
                break;
            }
            let index = (state as usize) % original.len();
            let mut flipped = original.clone();
            flipped[index] ^= 0xFF;
            let _ = read(&flipped);
        }
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

type FaultCheck = fn(&TextGridError) -> bool;

#[test]
fn malformed_fixtures_error_matches_the_documented_fault() {
    let cases: &[(&str, FaultCheck)] = &[
        ("binary_truncated_mid_header.TextGrid", |err| {
            matches!(err, TextGridError::UnexpectedEnd)
        }),
        ("binary_truncated_mid_interval.TextGrid", |err| {
            matches!(err, TextGridError::UnexpectedEnd)
        }),
        ("binary_bad_tier_class.TextGrid", |err| {
            matches!(err, TextGridError::UnknownTierClass { .. })
        }),
    ];
    for (name, matches_fault) in cases {
        let path = fixture_dir().join("malformed").join(name);
        if !path.is_file() {
            continue;
        }
        let bytes = fs::read(&path).expect("malformed fixture is readable");
        let err = read(&bytes).expect_err("expected the fixture to be rejected");
        assert!(
            matches_fault(&err),
            "{name} produced an unexpected error: {err}"
        );
    }
}
