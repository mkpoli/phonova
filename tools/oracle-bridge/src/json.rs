//! Minimal JSON value model and writer mirroring
//! `tools/oracle/src/oracle/jsonio.py`: object keys sorted lexicographically
//! regardless of insertion order, floats rounded to six decimal places
//! (`jsonio.FLOAT_DECIMALS`), non-finite floats never reach the page (they
//! become `null`, mirroring `oracle.measures._clean`'s NaN/Inf-to-`None`
//! conversion, rather than `jsonio._round`'s `ValueError` -- this bridge
//! never has raw Praat "no value" markers to reject, only its own optional
//! measurements).
//!
//! No external JSON crate is used: the formatting rules above are narrow and
//! specific enough that a small hand-written writer is easier to keep exactly
//! in sync with `jsonio.py` than configuring a general-purpose serializer.

use std::fmt::Write as _;

/// A JSON value restricted to what this bridge needs to emit.
#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    /// JSON `null`.
    Null,
    /// JSON boolean.
    Bool(bool),
    /// JSON number, always finite (construct via [`Json::number`]).
    Number(f64),
    /// JSON string.
    String(String),
    /// JSON array, serialized in the given order.
    Array(Vec<Json>),
    /// JSON object, serialized with keys sorted lexicographically.
    Object(Vec<(String, Json)>),
}

impl Json {
    /// Builds a number, mapping non-finite values to `null`.
    ///
    /// Mirrors `oracle.measures._clean`, which turns Praat's NaN/Inf
    /// "no value at this frame" markers into JSON `null` before the payload
    /// ever reaches `jsonio` (which rejects non-finite floats outright,
    /// since a *reference* file must never have needed that conversion).
    #[must_use]
    pub fn number(value: f64) -> Self {
        if value.is_finite() {
            Json::Number(value)
        } else {
            Json::Null
        }
    }

    /// Builds an object from `(key, value)` pairs.
    ///
    /// Insertion order does not matter: [`Json::to_pretty_string`] sorts keys
    /// before serializing, matching `jsonio._normalize`'s
    /// `sorted(obj.items())`.
    #[must_use]
    pub fn object(entries: Vec<(&str, Json)>) -> Self {
        Json::Object(
            entries
                .into_iter()
                .map(|(key, value)| (key.to_string(), value))
                .collect(),
        )
    }

    /// Serializes with two-space indentation, sorted object keys, and a
    /// trailing newline -- mirrors `jsonio.dumps`'s
    /// `json.dumps(normalized, indent=2, sort_keys=True) + "\n"`.
    #[must_use]
    pub fn to_pretty_string(&self) -> String {
        let mut out = String::new();
        write_value(self, 0, &mut out);
        out.push('\n');
        out
    }
}

const INDENT_UNIT: &str = "  ";

fn write_value(value: &Json, indent: usize, out: &mut String) {
    match value {
        Json::Null => out.push_str("null"),
        Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Json::Number(n) => out.push_str(&format_number(*n)),
        Json::String(s) => write_string(s, out),
        Json::Array(items) => write_array(items, indent, out),
        Json::Object(entries) => write_object(entries, indent, out),
    }
}

fn push_indent(level: usize, out: &mut String) {
    for _ in 0..level {
        out.push_str(INDENT_UNIT);
    }
}

fn write_array(items: &[Json], indent: usize, out: &mut String) {
    if items.is_empty() {
        out.push_str("[]");
        return;
    }
    out.push_str("[\n");
    for (i, item) in items.iter().enumerate() {
        push_indent(indent + 1, out);
        write_value(item, indent + 1, out);
        if i + 1 < items.len() {
            out.push(',');
        }
        out.push('\n');
    }
    push_indent(indent, out);
    out.push(']');
}

fn write_object(entries: &[(String, Json)], indent: usize, out: &mut String) {
    if entries.is_empty() {
        out.push_str("{}");
        return;
    }
    let mut sorted: Vec<&(String, Json)> = entries.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    out.push_str("{\n");
    for (i, (key, val)) in sorted.iter().enumerate() {
        push_indent(indent + 1, out);
        write_string(key, out);
        out.push_str(": ");
        write_value(val, indent + 1, out);
        if i + 1 < sorted.len() {
            out.push(',');
        }
        out.push('\n');
    }
    push_indent(indent, out);
    out.push('}');
}

fn write_string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Rounds to six decimal places and formats without a trailing run of zeros.
///
/// Mirrors `jsonio.FLOAT_DECIMALS = 6` combined with Python's `repr`-style
/// shortest-decimal float formatting. Rust's `{:.6}` rounds half-to-even like
/// IEEE 754 recommends, while Python's `round` is also round-half-to-even;
/// the two coincide for computed (non-literal-halfway) floats, which is all
/// this bridge ever formats.
fn format_number(value: f64) -> String {
    let mut s = format!("{value:.6}");
    while s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.pop();
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_keys_serialize_sorted_regardless_of_insertion_order() {
        let value = Json::object(vec![("b", Json::number(1.0)), ("a", Json::number(2.0))]);
        let text = value.to_pretty_string();
        let a_pos = text.find("\"a\"").expect("key a present");
        let b_pos = text.find("\"b\"").expect("key b present");
        assert!(
            a_pos < b_pos,
            "keys must serialize in sorted order:\n{text}"
        );
    }

    #[test]
    fn numbers_round_to_six_decimal_places_and_trim_trailing_zeros() {
        assert_eq!(format_number(110.618_212_999_9), "110.618213");
        assert_eq!(format_number(1.0), "1");
        assert_eq!(format_number(0.5), "0.5");
        assert_eq!(format_number(-0.000_000_4), "-0");
    }

    #[test]
    fn non_finite_numbers_become_null() {
        assert_eq!(Json::number(f64::NAN), Json::Null);
        assert_eq!(Json::number(f64::INFINITY), Json::Null);
        assert_eq!(Json::number(f64::NEG_INFINITY), Json::Null);
    }

    #[test]
    fn matches_the_measured_json_schema_diff_py_expects() {
        // `tools/oracle/src/oracle/diff.py`'s module doc comment: both sides
        // share `{case, measure, audio, params, frames}`.
        let payload = Json::object(vec![
            ("case", Json::String("pitch-defaults".to_string())),
            ("measure", Json::String("pitch".to_string())),
            (
                "audio",
                Json::String("tests/fixtures/audio/synth_vowel_a.wav".to_string()),
            ),
            (
                "params",
                Json::object(vec![("floor_hz", Json::number(75.0))]),
            ),
            (
                "frames",
                Json::Array(vec![Json::object(vec![
                    ("time", Json::number(0.02)),
                    ("voiced", Json::Bool(true)),
                    ("f0_hz", Json::number(110.003_054)),
                    ("strength", Json::number(0.851_017)),
                ])]),
            ),
        ]);
        let text = payload.to_pretty_string();
        for key in ["case", "measure", "audio", "params", "frames"] {
            assert!(
                text.contains(&format!("\"{key}\"")),
                "missing key {key}:\n{text}"
            );
        }
        assert!(
            text.ends_with('\n'),
            "output must end with a trailing newline"
        );
    }
}
