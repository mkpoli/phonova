//! Reader for Praat's binary TextGrid format.
//!
//! The format is undocumented by Praat itself; every field below was derived
//! clean-room from `tools/oracle`-generated sample pairs (a TextGrid saved as
//! both text and binary by Praat, via parselmouth, out-of-process) by
//! hexdumping the binary bytes and matching them against the already-known
//! text-format values. `docs/binary-format.md` records the full derivation,
//! with each claim cited to the fixture and byte offset that demonstrates it.

use crate::error::TextGridError;
use crate::tier_build::{IdMinter, build_interval_tier, build_point_tier};
use phx_annot::{Annotation, Tier, TierId, TierRelation, TierSlot};

/// Cursor over a binary TextGrid byte stream.
///
/// Every read is bounds-checked against the remaining bytes; running out of
/// input reports [`TextGridError::UnexpectedEnd`] rather than panicking, so a
/// truncated or arbitrary byte stream never crashes the reader.
struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], TextGridError> {
        let end = self
            .pos
            .checked_add(n)
            .ok_or(TextGridError::UnexpectedEnd)?;
        let slice = self
            .bytes
            .get(self.pos..end)
            .ok_or(TextGridError::UnexpectedEnd)?;
        self.pos = end;
        Ok(slice)
    }

    fn u8(&mut self) -> Result<u8, TextGridError> {
        Ok(self.take(1)?[0])
    }

    fn i16(&mut self) -> Result<i16, TextGridError> {
        let b = self.take(2)?;
        Ok(i16::from_be_bytes([b[0], b[1]]))
    }

    fn u16(&mut self) -> Result<u16, TextGridError> {
        let b = self.take(2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }

    fn i32(&mut self) -> Result<i32, TextGridError> {
        let b = self.take(4)?;
        Ok(i32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn f64(&mut self) -> Result<f64, TextGridError> {
        let b = self.take(8)?;
        Ok(f64::from_be_bytes(
            b.try_into().expect("`take(8)` returns 8 bytes"),
        ))
    }

    /// Reads a non-negative element count.
    fn count(&mut self) -> Result<usize, TextGridError> {
        let value = self.i32()?;
        if value < 0 {
            return Err(TextGridError::NegativeCount { value });
        }
        Ok(value as usize)
    }

    /// Reads a Praat "class name" string: a 1-byte unsigned length followed
    /// by that many bytes, one byte per character.
    ///
    /// Used only for the object-class and tier-class fields, which are drawn
    /// from Praat's small fixed vocabulary of type names (`"TextGrid"`,
    /// `"IntervalTier"`, `"TextTier"`) and are plain ASCII in every sample
    /// this reader was derived from (`docs/binary-format.md` §Header, §Tier
    /// record). A byte at or above `0x80` is decoded the same way the text
    /// reader's Latin-1 fallback treats legacy bytes, since no sample
    /// exercises one here and the class name is checked against a fixed
    /// string immediately after, so any such byte simply fails that check
    /// rather than reaching further parsing.
    fn class_name(&mut self) -> Result<String, TextGridError> {
        let len = self.u8()? as usize;
        let bytes = self.take(len)?;
        Ok(bytes.iter().map(|&b| b as char).collect())
    }

    /// Reads a Praat "text" field: a tier name, or an interval/point label.
    ///
    /// A 2-byte signed length `n` selects the encoding
    /// (`docs/binary-format.md` §Text field encoding):
    /// - `n == -1` flags a UTF-16BE string: a 2-byte unsigned character count
    ///   follows, then that many `char`s are decoded from the following
    ///   UTF-16BE code units (2 bytes each, 4 for a surrogate pair).
    /// - `n >= 0` is `n` literal bytes, one Latin-1 byte per character; every
    ///   sample this reader was derived from uses this form only for pure
    ///   ASCII text (`docs/binary-format.md` §Narrow strings are ASCII-only
    ///   in practice), but a byte at or above `0x80` is still decoded rather
    ///   than rejected, matching the text reader's own Latin-1 fallback for
    ///   legacy content.
    /// - Any other negative `n` is not attested by any sample and is reported
    ///   as [`TextGridError::InvalidTextLength`].
    fn text(&mut self) -> Result<String, TextGridError> {
        let n = self.i16()?;
        if n == -1 {
            let count = self.u16()? as usize;
            let mut text = String::with_capacity(count);
            for _ in 0..count {
                let unit = self.u16()?;
                let c = if (0xD800..=0xDBFF).contains(&unit) {
                    let low = self.u16()?;
                    if !(0xDC00..=0xDFFF).contains(&low) {
                        return Err(TextGridError::InvalidUtf16);
                    }
                    let scalar =
                        0x10000 + ((u32::from(unit) - 0xD800) << 10) + (u32::from(low) - 0xDC00);
                    char::from_u32(scalar).ok_or(TextGridError::InvalidUtf16)?
                } else if (0xDC00..=0xDFFF).contains(&unit) {
                    return Err(TextGridError::InvalidUtf16);
                } else {
                    char::from_u32(u32::from(unit)).ok_or(TextGridError::InvalidUtf16)?
                };
                text.push(c);
            }
            Ok(text)
        } else if n >= 0 {
            let bytes = self.take(n as usize)?;
            Ok(bytes.iter().map(|&b| b as char).collect())
        } else {
            Err(TextGridError::InvalidTextLength { length: n })
        }
    }
}

/// Parses a binary-format TextGrid.
///
/// `bytes` must start with the `ooBinaryFile` magic; [`crate::read`] checks
/// this before dispatching here and this function re-consumes those 12 bytes
/// without re-checking them.
pub(crate) fn parse(bytes: &[u8]) -> Result<Annotation, TextGridError> {
    let mut cursor = Cursor::new(bytes);
    cursor.take(12)?; // "ooBinaryFile" magic, already matched by the caller

    let class = cursor.class_name()?;
    if class != "TextGrid" {
        return Err(TextGridError::UnsupportedObjectClass { found: class });
    }

    let xmin = cursor.f64()?;
    let xmax = cursor.f64()?;

    let tiers_exist = match cursor.u8()? {
        0 => false,
        1 => true,
        found => return Err(TextGridError::InvalidTiersFlag { found }),
    };

    let mut ids = IdMinter::default();
    let mut slots = Vec::new();
    if tiers_exist {
        let tier_count = cursor.count()?;
        for _ in 0..tier_count {
            let tier_class = cursor.class_name()?;
            let name = cursor.text()?;
            let tier_xmin = cursor.f64()?;
            let tier_xmax = cursor.f64()?;
            let entry_count = cursor.count()?;

            let tier = match tier_class.as_str() {
                "IntervalTier" => {
                    let mut raw = Vec::new();
                    for _ in 0..entry_count {
                        let xmin = cursor.f64()?;
                        let xmax = cursor.f64()?;
                        let label = cursor.text()?;
                        raw.push((xmin, xmax, label));
                    }
                    Tier::Interval(build_interval_tier(
                        name, tier_xmin, tier_xmax, raw, &mut ids,
                    ))
                }
                "TextTier" => {
                    let mut raw = Vec::new();
                    for _ in 0..entry_count {
                        let time = cursor.f64()?;
                        let label = cursor.text()?;
                        raw.push((time, label));
                    }
                    Tier::Point(build_point_tier(name, tier_xmin, tier_xmax, raw, &mut ids))
                }
                _ => return Err(TextGridError::UnknownTierClass { found: tier_class }),
            };
            slots.push(TierSlot {
                id: TierId::new(ids.next_tier()),
                relation: TierRelation::Independent,
                tier,
            });
        }
    }

    Ok(Annotation::from_raw(xmin, xmax, slots)?)
}
