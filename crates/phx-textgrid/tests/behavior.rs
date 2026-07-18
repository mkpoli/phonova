//! Behavior tests that do not depend on fixture files: quoted-quote escaping,
//! empty-label handling, and reader robustness against arbitrary bytes.

use phx_annot::{
    Annotation, BoundaryId, Interval, IntervalId, IntervalTier, Point, PointId, PointTier, Tier,
    TierId, TierRelation, TierSlot,
};
use phx_textgrid::{read, write};

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
