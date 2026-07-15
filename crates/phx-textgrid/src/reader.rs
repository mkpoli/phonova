//! Tokenizer and grammar for the long and short TextGrid text formats.
//!
//! Both formats carry the same values in the same order; the long format adds
//! field-name tags, `item`/`intervals`/`points` headers, and an `<exists>`
//! flag, while the short format lists the bare values. A single token stream
//! drives both: numeric fields skip decorative words until a number, and string
//! fields skip decorative words until a quoted value, so the tags and headers
//! present only in the long format are consumed the same way in either variant.

use crate::error::TextGridError;
use crate::{Encoding, SourceInfo, Variant};
use phx_annot::{
    Annotation, BoundaryId, Interval, IntervalId, IntervalTier, Point, PointId, PointTier, Tier,
    TierId, TierRelation, TierSlot,
};

/// One lexical token: a quoted string (already unescaped) or a bare word.
enum Token {
    Word(String),
    Str(String),
}

fn tokenize(text: &str) -> Result<Vec<Token>, TextGridError> {
    let mut tokens = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(&ch) = chars.peek() {
        if ch.is_whitespace() {
            chars.next();
        } else if ch == '"' {
            chars.next();
            let mut value = String::new();
            loop {
                match chars.next() {
                    None => return Err(TextGridError::UnterminatedString),
                    Some('"') => {
                        if chars.peek() == Some(&'"') {
                            chars.next();
                            value.push('"');
                        } else {
                            break;
                        }
                    }
                    Some(other) => value.push(other),
                }
            }
            tokens.push(Token::Str(value));
        } else {
            let mut word = String::new();
            while let Some(&next) = chars.peek() {
                if next.is_whitespace() || next == '"' {
                    break;
                }
                word.push(next);
                chars.next();
            }
            tokens.push(Token::Word(word));
        }
    }
    Ok(tokens)
}

struct Cursor<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(tokens: &'a [Token]) -> Self {
        Self { tokens, pos: 0 }
    }

    /// Reads the next numeric value, skipping decorative words (tags, `=`,
    /// item and entry headers, and the `<exists>` flag).
    fn read_number(&mut self) -> Result<f64, TextGridError> {
        while let Some(token) = self.tokens.get(self.pos) {
            self.pos += 1;
            match token {
                Token::Str(_) => return Err(TextGridError::ExpectedNumber),
                Token::Word(word) => {
                    if let Ok(value) = word.parse::<f64>() {
                        if value.is_finite() {
                            return Ok(value);
                        }
                        return Err(TextGridError::InvalidNumber {
                            token: word.clone(),
                        });
                    }
                }
            }
        }
        Err(TextGridError::UnexpectedEnd)
    }

    /// Reads the next quoted value, skipping decorative words.
    fn read_string(&mut self) -> Result<String, TextGridError> {
        while let Some(token) = self.tokens.get(self.pos) {
            self.pos += 1;
            if let Token::Str(value) = token {
                return Ok(value.clone());
            }
        }
        Err(TextGridError::UnexpectedEnd)
    }

    /// Reads a non-negative count and returns it as a loop bound.
    fn read_count(&mut self) -> Result<usize, TextGridError> {
        let value = self.read_number()?;
        if value < 0.0 || value.fract() != 0.0 {
            return Err(TextGridError::InvalidNumber {
                token: value.to_string(),
            });
        }
        Ok(value as usize)
    }
}

/// Reads a decoded TextGrid string into an annotation and its source variant.
pub fn parse(text: &str, encoding: Encoding) -> Result<(Annotation, SourceInfo), TextGridError> {
    let tokens = tokenize(text)?;
    let variant = detect_variant(&tokens);
    let mut cursor = Cursor::new(&tokens);

    if cursor.read_string()? != "ooTextFile" {
        return Err(TextGridError::NotATextGrid);
    }
    let class = cursor.read_string()?;
    if class != "TextGrid" {
        return Err(TextGridError::UnsupportedObjectClass { found: class });
    }

    let xmin = cursor.read_number()?;
    let xmax = cursor.read_number()?;
    let tier_count = cursor.read_count()?;

    let mut ids = IdMinter::default();
    let mut slots = Vec::new();
    for _ in 0..tier_count {
        let tier_class = cursor.read_string()?;
        let name = cursor.read_string()?;
        let _tier_xmin = cursor.read_number()?;
        let _tier_xmax = cursor.read_number()?;
        let entry_count = cursor.read_count()?;

        let tier = match tier_class.as_str() {
            "IntervalTier" => Tier::Interval(read_interval_tier(
                &mut cursor,
                name,
                entry_count,
                &mut ids,
            )?),
            "TextTier" => Tier::Point(read_point_tier(&mut cursor, name, entry_count, &mut ids)?),
            _ => return Err(TextGridError::UnknownTierClass { found: tier_class }),
        };
        slots.push(TierSlot {
            id: TierId::new(ids.next_tier()),
            relation: TierRelation::Independent,
            tier,
        });
    }

    let annotation = Annotation::from_raw(xmin, xmax, slots);
    Ok((annotation, SourceInfo { variant, encoding }))
}

fn read_interval_tier(
    cursor: &mut Cursor<'_>,
    name: String,
    count: usize,
    ids: &mut IdMinter,
) -> Result<IntervalTier, TextGridError> {
    let mut intervals = Vec::new();
    let mut shared_boundary: Option<u64> = None;
    for _ in 0..count {
        let xmin = cursor.read_number()?;
        let xmax = cursor.read_number()?;
        let label = cursor.read_string()?;
        let start = shared_boundary.unwrap_or_else(|| ids.next_boundary());
        let end = ids.next_boundary();
        shared_boundary = Some(end);
        intervals.push(Interval {
            id: IntervalId::new(ids.next_interval()),
            start_boundary: BoundaryId::new(start),
            end_boundary: BoundaryId::new(end),
            xmin,
            xmax,
            label,
        });
    }
    Ok(IntervalTier { name, intervals })
}

fn read_point_tier(
    cursor: &mut Cursor<'_>,
    name: String,
    count: usize,
    ids: &mut IdMinter,
) -> Result<PointTier, TextGridError> {
    let mut points = Vec::new();
    for _ in 0..count {
        let time = cursor.read_number()?;
        let label = cursor.read_string()?;
        points.push(Point {
            id: PointId::new(ids.next_point()),
            time,
            label,
        });
    }
    Ok(PointTier { name, points })
}

/// The long format is the only variant that carries a bare `xmin` field tag;
/// the short format lists the value alone. A label containing `xmin` is quoted,
/// so it is a string token and does not trigger this check.
fn detect_variant(tokens: &[Token]) -> Variant {
    let is_long = tokens
        .iter()
        .any(|token| matches!(token, Token::Word(word) if word == "xmin"));
    if is_long {
        Variant::Long
    } else {
        Variant::Short
    }
}

/// Mints document-wide unique identifiers so that boundary, interval, and point
/// identifiers never collide across tiers, which `Annotation::validate` checks.
#[derive(Default)]
struct IdMinter {
    tier: u64,
    boundary: u64,
    interval: u64,
    point: u64,
}

impl IdMinter {
    fn next_tier(&mut self) -> u64 {
        self.tier += 1;
        self.tier
    }

    fn next_boundary(&mut self) -> u64 {
        self.boundary += 1;
        self.boundary
    }

    fn next_interval(&mut self) -> u64 {
        self.interval += 1;
        self.interval
    }

    fn next_point(&mut self) -> u64 {
        self.point += 1;
        self.point
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escaped_quotes_tokenize_to_one_quote() {
        let tokens = tokenize("\"say \"\"hi\"\"\"").expect("tokenizes");
        assert_eq!(tokens.len(), 1);
        let Token::Str(value) = &tokens[0] else {
            panic!("expected a string token");
        };
        assert_eq!(value, "say \"hi\"");
    }

    #[test]
    fn unterminated_final_string_errors() {
        assert!(matches!(
            tokenize("\"open"),
            Err(TextGridError::UnterminatedString)
        ));
    }

    #[test]
    fn read_number_skips_long_format_tags() {
        let tokens = tokenize("intervals [1]:\n            xmin = 0.35").expect("tokenizes");
        let mut cursor = Cursor::new(&tokens);
        assert_eq!(cursor.read_number().expect("reads"), 0.35);
    }

    #[test]
    fn long_and_short_snippets_parse_equal() {
        let long = "\
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
        intervals: size = 1
        intervals [1]:
            xmin = 0
            xmax = 1
            text = \"hi\"
";
        let short = "\
File type = \"ooTextFile\"
Object class = \"TextGrid\"
0
1
<exists>
1
\"IntervalTier\"
\"w\"
0
1
1
0
1
\"hi\"
";
        let (long_doc, long_info) = parse(long, Encoding::Utf8).expect("long parses");
        let (short_doc, short_info) = parse(short, Encoding::Utf8).expect("short parses");
        assert_eq!(long_info.variant, Variant::Long);
        assert_eq!(short_info.variant, Variant::Short);
        assert_eq!(long_doc, short_doc);
    }
}
