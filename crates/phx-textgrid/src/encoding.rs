//! Byte-order-mark sniffing and decoding of the encodings Praat writes.

use crate::error::TextGridError;

/// Character encoding detected on a TextGrid byte stream.
///
/// Praat writes UTF-8 by default and UTF-16 with a byte-order mark for text that
/// needs it; legacy files predating Unicode output are read as Latin-1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding {
    /// UTF-8 without a byte-order mark.
    Utf8,
    /// UTF-8 preceded by an `EF BB BF` byte-order mark.
    Utf8Bom,
    /// UTF-16 little-endian, introduced by an `FF FE` byte-order mark.
    Utf16Le,
    /// UTF-16 big-endian, introduced by a `FE FF` byte-order mark.
    Utf16Be,
    /// ISO-8859-1 fallback for legacy files with no byte-order mark.
    Latin1,
}

/// Detects the encoding of `bytes` and decodes them to a `String`.
///
/// A byte-order mark selects UTF-8, UTF-16LE, or UTF-16BE. Without a mark, a
/// stream that is valid UTF-8 is read as UTF-8; anything else falls back to
/// Latin-1, which maps every byte to the code point of the same value.
pub fn decode(bytes: &[u8]) -> Result<(String, Encoding), TextGridError> {
    if bytes.is_empty() {
        return Err(TextGridError::Empty);
    }
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        let text = std::str::from_utf8(rest).map_err(|_| TextGridError::InvalidUtf8)?;
        return Ok((text.to_owned(), Encoding::Utf8Bom));
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        return Ok((decode_utf16(rest, Endian::Little)?, Encoding::Utf16Le));
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        return Ok((decode_utf16(rest, Endian::Big)?, Encoding::Utf16Be));
    }
    match std::str::from_utf8(bytes) {
        Ok(text) => Ok((text.to_owned(), Encoding::Utf8)),
        Err(_) => Ok((bytes.iter().map(|&b| b as char).collect(), Encoding::Latin1)),
    }
}

#[derive(Clone, Copy)]
enum Endian {
    Little,
    Big,
}

fn decode_utf16(bytes: &[u8], endian: Endian) -> Result<String, TextGridError> {
    if !bytes.len().is_multiple_of(2) {
        return Err(TextGridError::OddUtf16Length);
    }
    let units = bytes.chunks_exact(2).map(|pair| match endian {
        Endian::Little => u16::from_le_bytes([pair[0], pair[1]]),
        Endian::Big => u16::from_be_bytes([pair[0], pair[1]]),
    });
    char::decode_utf16(units)
        .collect::<Result<String, _>>()
        .map_err(|_| TextGridError::InvalidUtf16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_errors() {
        assert_eq!(decode(b""), Err(TextGridError::Empty));
    }

    #[test]
    fn plain_ascii_is_utf8() {
        let (text, enc) = decode(b"xmin = 0").expect("decodes");
        assert_eq!(text, "xmin = 0");
        assert_eq!(enc, Encoding::Utf8);
    }

    #[test]
    fn utf8_bom_is_stripped() {
        let (text, enc) = decode(b"\xEF\xBB\xBFabc").expect("decodes");
        assert_eq!(text, "abc");
        assert_eq!(enc, Encoding::Utf8Bom);
    }

    #[test]
    fn utf16_le_and_be_decode() {
        let le = [0xFF, 0xFE, b'h', 0x00, b'i', 0x00];
        let (text, enc) = decode(&le).expect("decodes");
        assert_eq!(text, "hi");
        assert_eq!(enc, Encoding::Utf16Le);

        let be = [0xFE, 0xFF, 0x00, b'h', 0x00, b'i'];
        let (text, enc) = decode(&be).expect("decodes");
        assert_eq!(text, "hi");
        assert_eq!(enc, Encoding::Utf16Be);
    }

    #[test]
    fn odd_utf16_length_errors() {
        assert_eq!(
            decode(&[0xFF, 0xFE, 0x00]),
            Err(TextGridError::OddUtf16Length)
        );
    }

    #[test]
    fn invalid_utf8_without_bom_falls_back_to_latin1() {
        // 0xDF is ß in Latin-1 and an invalid lone lead byte in UTF-8.
        let (text, enc) = decode(b"Stra\xDFe").expect("decodes");
        assert_eq!(text, "Straße");
        assert_eq!(enc, Encoding::Latin1);
    }
}
