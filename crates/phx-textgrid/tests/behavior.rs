//! Behavior tests that do not depend on fixture files: quoted-quote escaping,
//! empty-label handling, and reader robustness against arbitrary bytes.

use phx_annot::{
    Annotation, BoundaryId, IntegrityIssue, Interval, IntervalId, IntervalTier, Point, PointId,
    PointTier, Tier, TierId, TierRelation, TierSlot,
};
use phx_textgrid::{TextGridError, read, read_lenient, write};

fn interval(id: u64, b0: u64, b1: u64, xmin: f64, xmax: f64, label: &str) -> Interval {
    Interval {
        id: IntervalId::new(id),
        start_boundary: BoundaryId::new(b0),
        end_boundary: BoundaryId::new(b1),
        xmin,
        xmax,
        label: label.to_owned(),
    }
}

#[test]
fn quoted_quotes_in_labels_survive_round_trip() {
    let tier = IntervalTier {
        name: "quote\"tier".to_owned(),
        xmin: 0.0,
        xmax: 1.0,
        intervals: vec![
            interval(1, 1, 2, 0.0, 0.5, "say \"hi\""),
            interval(2, 2, 3, 0.5, 1.0, ""),
        ],
    };
    let doc = Annotation::from_raw(
        0.0,
        1.0,
        vec![TierSlot {
            id: TierId::new(1),
            relation: TierRelation::Independent,
            tier: Tier::Interval(tier),
        }],
    )
    .expect("valid raw document");

    let bytes = write(&doc).expect("finite document writes");
    let text = std::str::from_utf8(&bytes).expect("written output is UTF-8");
    assert!(
        text.contains("\"say \"\"hi\"\"\""),
        "embedded quotes were not doubled: {text}"
    );

    let (reparsed, _) = read(&bytes).expect("written document reads back");
    assert_eq!(doc, reparsed);
    let Tier::Interval(back) = &reparsed.tiers()[0].tier else {
        panic!("expected an interval tier");
    };
    assert_eq!(back.intervals[0].label, "say \"hi\"");
    assert_eq!(back.name, "quote\"tier");
}

#[test]
fn point_tier_labels_round_trip() {
    let doc = Annotation::from_raw(
        0.0,
        2.0,
        vec![TierSlot {
            id: TierId::new(1),
            relation: TierRelation::Independent,
            tier: Tier::Point(PointTier {
                name: "marks".to_owned(),
                xmin: 0.0,
                xmax: 2.0,
                points: vec![
                    Point {
                        id: PointId::new(1),
                        time: 0.5,
                        label: "ˈa".to_owned(),
                    },
                    Point {
                        id: PointId::new(2),
                        time: 1.5,
                        label: String::new(),
                    },
                ],
            }),
        }],
    )
    .expect("valid raw document");

    let written = write(&doc).expect("finite document writes");
    let (reparsed, _) = read(&written).expect("point document reads back");
    assert_eq!(doc, reparsed);
}

#[test]
fn zero_tier_document_round_trips() {
    let doc = Annotation::from_raw(0.0, 2.0, Vec::new()).expect("valid raw document");
    let bytes = write(&doc).expect("finite document writes");
    let (reparsed, _) = read(&bytes).expect("zero-tier document reads back");
    assert_eq!(doc, reparsed);
    assert!(reparsed.tiers().is_empty());
}

#[test]
fn arbitrary_bytes_never_panic() {
    let samples: &[&[u8]] = &[
        b"",
        b"\x00\x01\x02\x03",
        b"File type = \"ooTextFile\"",
        b"File type = \"ooTextFile\"\nObject class = \"Sound\"\n",
        b"\xff\xfe\x00",
        b"\xfe\xff",
        b"\xef\xbb\xbf\xff",
        &[0xff; 64],
        b"File type = \"ooTextFile\"\nObject class = \"TextGrid\"\nxmin = notanumber\n",
        b"File type = \"ooTextFile\"\nObject class = \"TextGrid\"\n0\n1\n<exists>\n99999\n",
        b"ooBinaryFile",
        b"ooBinaryFile\x00",
        b"ooBinaryFile\x08TextGri", // class-name string truncated mid-payload
        b"ooBinaryFile\x08TextGrid",
        b"ooBinaryFile\x08NotAGrid",
        b"ooBinaryFile\x08TextGrid\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x02",
        b"ooBinaryFile\xffNotAsCII\x00",
    ];
    for sample in samples {
        // A malformed sample must yield an error, and reading it must not panic.
        let _ = read(sample);
    }

    // A longer pseudo-random sweep exercises the tokenizer and grammar guards.
    let mut state: u32 = 0x1234_5678;
    for _ in 0..2000 {
        let len = (state as usize % 96) + 1;
        let mut bytes = Vec::with_capacity(len);
        for _ in 0..len {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            bytes.push((state >> 16) as u8);
        }
        let _ = read(&bytes);
    }

    // Same sweep, but every sample opens with the binary magic so the fuzz
    // exercises the binary reader's cursor and text-field decoding instead of
    // falling through to the text/encoding path.
    let mut state: u32 = 0x4321_8765;
    for _ in 0..2000 {
        let len = (state as usize % 96) + 1;
        let mut bytes = b"ooBinaryFile".to_vec();
        for _ in 0..len {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            bytes.push((state >> 16) as u8);
        }
        let _ = read(&bytes);
    }
}

#[test]
fn read_rejects_a_reversed_document_domain() {
    let text = "\
File type = \"ooTextFile\"
Object class = \"TextGrid\"

xmin = 1
xmax = 0
tiers? <absent>
";
    let err = read(text.as_bytes()).expect_err("reversed domain fails strict read");
    let TextGridError::Invalid(issues) = err else {
        panic!("expected TextGridError::Invalid, got {err:?}");
    };
    assert!(
        issues
            .iter()
            .any(|issue| matches!(issue, IntegrityIssue::InvalidTimeDomain { .. }))
    );

    let (doc, _, issues) = read_lenient(text.as_bytes()).expect("lenient read still parses");
    assert_eq!(doc.xmin(), 1.0);
    assert_eq!(doc.xmax(), 0.0);
    assert!(
        issues
            .iter()
            .any(|issue| matches!(issue, IntegrityIssue::InvalidTimeDomain { .. }))
    );
}

#[test]
fn read_rejects_a_gapped_interval_tier() {
    let text = "\
File type = \"ooTextFile\"
Object class = \"TextGrid\"

xmin = 0
xmax = 1
tiers? <exists>
size = 1
item []:
    item [1]:
        class = \"IntervalTier\"
        name = \"w\"
        xmin = 0
        xmax = 1
        intervals: size = 2
        intervals [1]:
            xmin = 0
            xmax = 0.3
            text = \"a\"
        intervals [2]:
            xmin = 0.6
            xmax = 1
            text = \"b\"
";
    let err = read(text.as_bytes()).expect_err("gapped interval tier fails strict read");
    let TextGridError::Invalid(issues) = err else {
        panic!("expected TextGridError::Invalid, got {err:?}");
    };
    assert!(
        issues
            .iter()
            .any(|issue| matches!(issue, IntegrityIssue::IntervalGap { .. }))
    );

    let (doc, _, issues) = read_lenient(text.as_bytes()).expect("lenient read still parses");
    let Tier::Interval(tier) = &doc.tiers()[0].tier else {
        panic!("expected an interval tier");
    };
    assert_eq!(tier.intervals.len(), 2);
    assert!(
        issues
            .iter()
            .any(|issue| matches!(issue, IntegrityIssue::IntervalGap { .. }))
    );
}

#[test]
fn read_accepts_a_tier_domain_narrower_than_the_document() {
    // TierDomainMismatch is advisory, not a read failure: Praat itself
    // writes documents like this one.
    let text = "\
File type = \"ooTextFile\"
Object class = \"TextGrid\"

xmin = 0
xmax = 2
tiers? <exists>
size = 1
item []:
    item [1]:
        class = \"IntervalTier\"
        name = \"narrow\"
        xmin = 0.5
        xmax = 1.5
        intervals: size = 1
        intervals [1]:
            xmin = 0.5
            xmax = 1.5
            text = \"\"
";
    let (doc, _) = read(text.as_bytes()).expect("advisory-only document reads strictly");
    assert_eq!(doc.validate().len(), 1);
}

/// Assembles a minimal binary TextGrid with no tiers: the 12-byte magic, the
/// `"TextGrid"` class-name string, `xmin`/`xmax` as big-endian `f64`, and a
/// `0x00` tiers-exist flag. `crates/phx-textgrid/docs/binary-format.md`
/// documents this layout.
fn binary_zero_tier_document(xmin: f64, xmax: f64) -> Vec<u8> {
    let mut bytes = b"ooBinaryFile".to_vec();
    bytes.push(8); // class-name length
    bytes.extend_from_slice(b"TextGrid");
    bytes.extend_from_slice(&xmin.to_be_bytes());
    bytes.extend_from_slice(&xmax.to_be_bytes());
    bytes.push(0); // tiers-exist flag: no tiers follow
    bytes
}

#[test]
fn read_rejects_a_reversed_document_domain_in_the_binary_format() {
    let bytes = binary_zero_tier_document(1.0, 0.0);
    let err = read(&bytes).expect_err("reversed domain fails strict read");
    let TextGridError::Invalid(issues) = err else {
        panic!("expected TextGridError::Invalid, got {err:?}");
    };
    assert!(
        issues
            .iter()
            .any(|issue| matches!(issue, IntegrityIssue::InvalidTimeDomain { .. }))
    );

    let (doc, _, issues) = read_lenient(&bytes).expect("lenient read still parses");
    assert_eq!(doc.xmin(), 1.0);
    assert_eq!(doc.xmax(), 0.0);
    assert!(
        issues
            .iter()
            .any(|issue| matches!(issue, IntegrityIssue::InvalidTimeDomain { .. }))
    );
}
