//! Praat TextGrid read (long text, short text, and binary formats; UTF-8,
//! UTF-16, and Latin-1 for text) and write (long text format, UTF-8 always).
//!
//! The reader accepts every format Praat writes and reports the detected
//! [`Variant`] and [`Encoding`] for a text-format source. A byte-order mark
//! selects UTF-8 or UTF-16; without one, a stream that is valid UTF-8 is read
//! as UTF-8, and anything else is decoded as Latin-1, which is how Praat wrote
//! files predating its Unicode support. A file from that era saved under Mac OS
//! Classic may instead carry MacRoman-encoded bytes, which are also valid
//! Latin-1 byte-for-byte and are not distinguished from it; Praat's own manual
//! describes the two encodings as ambiguous without external knowledge of a
//! file's origin. A stream opening with the `ooBinaryFile` magic is read as
//! Praat's binary format instead, an undocumented format this crate derives
//! from oracle-generated sample pairs rather than from Praat's source; see
//! `docs/binary-format.md`.
//!
//! The writer emits one canonical shape: the long text format, UTF-8, `LF`
//! line endings, no byte-order mark. TextGrid carries no cross-tier relation
//! data, so every imported tier is [`TierRelation::Independent`]; tier
//! relations are a project-level concept that lives outside this format.
//! Writing the binary format is out of scope; [`write`] always emits text.
//!
//! [`TierRelation::Independent`]: phx_annot::TierRelation::Independent
#![warn(missing_docs)]

mod binary;
mod encoding;
mod error;
mod reader;
mod tier_build;
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
/// Reported for inspection; the writer does not consult it, so any source
/// re-emerges as UTF-8 long text format on write.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceInfo {
    /// A text-format source, with its detected variant and encoding.
    Text {
        /// Text-format variant the file was written in.
        variant: Variant,
        /// Character encoding the file was decoded from.
        encoding: Encoding,
    },
    /// A binary-format source.
    Binary,
}

/// Reads a TextGrid from bytes, returning the document and its source
/// provenance.
///
/// A stream opening with the `ooBinaryFile` magic is read as Praat's binary
/// format. Otherwise, encoding is detected by byte-order mark, then by UTF-8
/// validity, then by a Latin-1 fallback, and the text-format variant is
/// detected from structure, not from any filename. The parsed document must
/// also pass [`phx_annot::Annotation::validate`] apart from an advisory tier
/// domain (see [`phx_annot::IntegrityIssue::is_advisory`]): a reversed
/// document domain, a gap or overlap between intervals, or a duplicate id
/// reports [`TextGridError::Invalid`] instead of silently handing back a
/// structurally broken document. [`read_lenient`] returns the same cases
/// alongside the document instead of rejecting them. Malformed or truncated
/// input yields a [`TextGridError`] either way; the reader never panics.
pub fn read(bytes: &[u8]) -> Result<(Annotation, SourceInfo), TextGridError> {
    let (annotation, source, issues) = read_lenient(bytes)?;
    let blocking: Vec<_> = issues
        .into_iter()
        .filter(|issue| !issue.is_advisory())
        .collect();
    if blocking.is_empty() {
        Ok((annotation, source))
    } else {
        Err(TextGridError::Invalid(blocking))
    }
}

/// Reads a TextGrid from bytes without rejecting a structurally invalid
/// document, returning the document, its source provenance, and every
/// integrity issue [`phx_annot::Annotation::validate`] found.
///
/// Recovery and import tooling that wants to show a user what is wrong with a
/// TextGrid — rather than simply refusing to open it — reads with this
/// function instead of [`read`]. A malformed or truncated byte stream that
/// cannot be parsed into tiers at all still yields a [`TextGridError`]; only
/// a structurally questionable *result* of a successful parse is returned
/// rather than rejected.
pub fn read_lenient(
    bytes: &[u8],
) -> Result<(Annotation, SourceInfo, Vec<phx_annot::IntegrityIssue>), TextGridError> {
    if bytes.starts_with(b"ooBinaryFile") {
        let annotation = binary::parse(bytes)?;
        let issues = annotation.validate();
        return Ok((annotation, SourceInfo::Binary, issues));
    }
    let (text, encoding) = encoding::decode(bytes)?;
    let (annotation, variant) = reader::parse(&text)?;
    let issues = annotation.validate();
    Ok((annotation, SourceInfo::Text { variant, encoding }, issues))
}
