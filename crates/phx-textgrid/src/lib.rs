//! Praat TextGrid read (long and short text formats; UTF-8/UTF-16/Latin-1) and
//! write (long format, UTF-8 always).
//!
//! The reader accepts both text-format variants and reports the detected
//! [`Variant`] and [`Encoding`] alongside the document. A byte-order mark
//! selects UTF-8 or UTF-16; without one, a stream that is valid UTF-8 is read
//! as UTF-8, and anything else is decoded as Latin-1, which is how Praat wrote
//! files predating its Unicode support. A file from that era saved under Mac OS
//! Classic may instead carry MacRoman-encoded bytes, which are also valid
//! Latin-1 byte-for-byte and are not distinguished from it; Praat's own manual
//! describes the two encodings as ambiguous without external knowledge of a
//! file's origin.
//!
//! The writer emits one canonical shape: the long text format, UTF-8, `LF`
//! line endings, no byte-order mark. TextGrid carries no cross-tier relation
//! data, so every imported tier is [`TierRelation::Independent`]; tier
//! relations are a project-level concept that lives outside this format.
//!
//! The binary TextGrid variant is not read; [`read`] reports it as
//! [`TextGridError::BinaryUnsupported`].
//!
//! [`TierRelation::Independent`]: phx_annot::TierRelation::Independent
#![warn(missing_docs)]

mod encoding;
mod error;
mod reader;
mod writer;

pub use encoding::Encoding;
pub use error::TextGridError;
pub use writer::write;

use phx_annot::Annotation;

/// Text-format variant a TextGrid was written in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    /// Long (full) text format with field-name tags and item headers.
    Long,
    /// Short text format with bare values.
    Short,
}

/// Detected provenance of a read TextGrid.
///
/// The variant and encoding are reported for inspection; the writer does not
/// consult them, so a UTF-16 short-format source re-emerges as UTF-8 long
/// format on write.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceInfo {
    /// Text-format variant the file was written in.
    pub variant: Variant,
    /// Character encoding the file was decoded from.
    pub encoding: Encoding,
}

/// Reads a TextGrid from bytes, returning the document and its source provenance.
///
/// Encoding is detected by byte-order mark, then by UTF-8 validity, then by a
/// Latin-1 fallback. The text-format variant is detected from structure, not
/// from any filename. Malformed input yields a [`TextGridError`]; the reader
/// never panics.
pub fn read(bytes: &[u8]) -> Result<(Annotation, SourceInfo), TextGridError> {
    if bytes.starts_with(b"ooBinaryFile") {
        return Err(TextGridError::BinaryUnsupported);
    }
    let (text, encoding) = encoding::decode(bytes)?;
    reader::parse(&text, encoding)
}
