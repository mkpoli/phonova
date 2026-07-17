//! Serialization to the long TextGrid text format.

use phx_annot::{Annotation, Tier};
use std::fmt::Write as _;

/// Serializes an annotation as a long-format TextGrid.
///
/// The output is UTF-8 with `LF` line endings and no byte-order mark, matching
/// fixed design rule 4 (legacy encodings are read, never produced). Each tier's
/// own `xmin`/`xmax` is written as the document time domain, which is what Praat
/// emits for tiers that span the whole recording.
pub fn write(annotation: &Annotation) -> Vec<u8> {
    let xmin = number(annotation.xmin());
    let xmax = number(annotation.xmax());

    let mut out = String::new();
    out.push_str("File type = \"ooTextFile\"\n");
    out.push_str("Object class = \"TextGrid\"\n\n");
    let _ = writeln!(out, "xmin = {xmin}");
    let _ = writeln!(out, "xmax = {xmax}");

    if annotation.tiers().is_empty() {
        out.push_str("tiers? <absent>\n");
        return out.into_bytes();
    }

    out.push_str("tiers? <exists>\n");
    let _ = writeln!(out, "size = {}", annotation.tiers().len());
    out.push_str("item []:\n");

    for (index, slot) in annotation.tiers().iter().enumerate() {
        let _ = writeln!(out, "    item [{}]:", index + 1);
        match &slot.tier {
            Tier::Interval(tier) => {
                let _ = writeln!(out, "        class = \"IntervalTier\"");
                let _ = writeln!(out, "        name = \"{}\"", escape(&tier.name));
                let _ = writeln!(out, "        xmin = {xmin}");
                let _ = writeln!(out, "        xmax = {xmax}");
                let _ = writeln!(out, "        intervals: size = {}", tier.intervals.len());
                for (entry, interval) in tier.intervals.iter().enumerate() {
                    let _ = writeln!(out, "        intervals [{}]:", entry + 1);
                    let _ = writeln!(out, "            xmin = {}", number(interval.xmin));
                    let _ = writeln!(out, "            xmax = {}", number(interval.xmax));
                    let _ = writeln!(out, "            text = \"{}\"", escape(&interval.label));
                }
            }
            Tier::Point(tier) => {
                let _ = writeln!(out, "        class = \"TextTier\"");
                let _ = writeln!(out, "        name = \"{}\"", escape(&tier.name));
                let _ = writeln!(out, "        xmin = {xmin}");
                let _ = writeln!(out, "        xmax = {xmax}");
                let _ = writeln!(out, "        points: size = {}", tier.points.len());
                for (entry, point) in tier.points.iter().enumerate() {
                    let _ = writeln!(out, "        points [{}]:", entry + 1);
                    let _ = writeln!(out, "            number = {}", number(point.time));
                    let _ = writeln!(out, "            mark = \"{}\"", escape(&point.label));
                }
            }
        }
    }

    out.into_bytes()
}

/// Doubles every quote so the value survives Praat's quoted-string reading.
fn escape(text: &str) -> String {
    text.replace('"', "\"\"")
}

/// Formats a time as the shortest decimal that reparses to the same value,
/// which keeps a written file byte-stable across a further round-trip. Praat's
/// reader accepts this decimal serialization.
fn number(value: f64) -> String {
    if value.is_finite() {
        format!("{value}")
    } else {
        "0".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integers_lose_their_fraction() {
        assert_eq!(number(0.0), "0");
        assert_eq!(number(1.0), "1");
        assert_eq!(number(5.0), "5");
    }

    #[test]
    fn fractions_use_shortest_decimal() {
        assert_eq!(number(0.35), "0.35");
        assert_eq!(number(3.235063), "3.235063");
        assert_eq!(number(0.05), "0.05");
    }

    #[test]
    fn non_finite_times_are_neutralized() {
        assert_eq!(number(f64::NAN), "0");
        assert_eq!(number(f64::INFINITY), "0");
    }

    #[test]
    fn quotes_are_doubled() {
        assert_eq!(escape("a\"b"), "a\"\"b");
        assert_eq!(escape(""), "");
        assert_eq!(escape("plain"), "plain");
    }

    #[test]
    fn zero_tier_document_writes_the_absent_flag_and_stops() {
        let doc = Annotation::from_raw(0.0, 1.0, Vec::new()).expect("valid raw document");
        let bytes = write(&doc);
        let text = std::str::from_utf8(&bytes).expect("written output is UTF-8");
        assert_eq!(
            text,
            "File type = \"ooTextFile\"\nObject class = \"TextGrid\"\n\nxmin = 0\nxmax = 1\ntiers? <absent>\n"
        );
    }
}
