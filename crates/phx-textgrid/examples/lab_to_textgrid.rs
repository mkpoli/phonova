//! Converts an HTK-style forced-alignment label file into a canonical
//! TextGrid.
//!
//! Reads lines of `<end_time> <state> <label>` (the format CMU ARCTIC
//! publishes under each voice's `lab/` directory) and writes a single
//! interval tier whose boundaries are exactly the file's cumulative end
//! times: the source is preserved verbatim. The whole-document `xmax` is the
//! caller-supplied audio duration, and the final interval's boundary is
//! extended to meet it, since a forced aligner's last labeled frame commonly
//! falls short of the file's true sample count by a few milliseconds of
//! untranscribed trailing silence. No boundary is invented, merged, or moved
//! beyond that single required extension.
//!
//! ```text
//! cargo run -p phx-textgrid --example lab_to_textgrid -- \
//!   input.lab 3.2350625 phones output.TextGrid
//! ```

use phx_annot::{Annotation, IntervalId, LabelTarget, Tier, TierRelation};
use std::env;
use std::fs;
use std::process::ExitCode;

fn parse_lab(text: &str) -> Vec<(f64, String)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line == "#" {
            continue;
        }
        let mut parts = line.split_whitespace();
        let end: f64 = parts
            .next()
            .expect("lab line has an end-time field")
            .parse()
            .expect("end-time field is a finite number");
        let _state = parts.next().expect("lab line has a state field");
        let label = parts
            .next()
            .expect("lab line has a label field")
            .to_string();
        out.push((end, label));
    }
    out
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let [_, lab_path, duration, tier_name, out_path] = args.as_slice() else {
        eprintln!(
            "usage: lab_to_textgrid <input.lab> <audio_duration_s> <tier_name> <output.TextGrid>"
        );
        return ExitCode::FAILURE;
    };
    let wav_duration: f64 = duration.parse().expect("audio_duration_s is a number");

    let lab_text = fs::read_to_string(lab_path).expect("read lab file");
    let mut segments = parse_lab(&lab_text);
    assert!(!segments.is_empty(), "lab file has no segments");
    let last_end = segments.last_mut().expect("checked non-empty above");
    assert!(
        wav_duration >= last_end.0,
        "audio duration {wav_duration} is shorter than the lab file's last boundary {}",
        last_end.0
    );
    last_end.0 = wav_duration;

    let mut doc = Annotation::new(0.0, wav_duration).expect("valid document domain");
    let tier = doc
        .add_interval_tier(tier_name, TierRelation::Independent)
        .expect("add tier");
    for (end, _) in &segments[..segments.len() - 1] {
        doc.insert_boundary(tier, *end).expect("insert boundary");
    }

    let slot = doc.tier(tier).expect("tier exists");
    let Tier::Interval(interval_tier) = &slot.tier else {
        unreachable!("add_interval_tier always creates an interval tier");
    };
    let ids: Vec<IntervalId> = interval_tier.intervals.iter().map(|iv| iv.id).collect();
    assert_eq!(ids.len(), segments.len(), "interval/segment count mismatch");
    for (id, (_, label)) in ids.iter().zip(segments.iter()) {
        doc.set_label(
            LabelTarget::Interval {
                tier,
                interval: *id,
            },
            label,
        )
        .expect("set label");
    }

    let bytes = phx_textgrid::write(&doc).expect("write textgrid");
    fs::write(out_path, bytes).expect("write output file");
    println!("wrote {out_path}");
    ExitCode::SUCCESS
}
