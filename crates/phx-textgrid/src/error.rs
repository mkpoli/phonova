//! Typed errors reported when a byte stream cannot be read as a TextGrid.

use std::fmt;

/// Reason a byte stream could not be read as a Praat TextGrid.
///
/// Every reader failure path returns one of these variants; the reader never
/// panics on malformed, truncated, or arbitrary input.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum TextGridError {
    /// The input held no bytes.
    Empty,
    /// A UTF-16 stream had an odd number of bytes after its byte-order mark.
    OddUtf16Length,
    /// A UTF-16 code unit sequence contained an unpaired surrogate.
    InvalidUtf16,
    /// A byte-order-marked UTF-8 stream held an invalid UTF-8 sequence.
    InvalidUtf8,
    /// A quoted label ran to the end of the stream without a closing quote.
    UnterminatedString,
    /// The `File type` header was absent or was not `"ooTextFile"`.
    NotATextGrid,
    /// The object class header named a class other than `TextGrid`.
    UnsupportedObjectClass {
        /// Class name found in the header.
        found: String,
    },
    /// A tier declared a class other than `IntervalTier` or `TextTier`.
    UnknownTierClass {
        /// Class name found on the tier.
        found: String,
    },
    /// A numeric field held a token that did not parse as a finite number.
    InvalidNumber {
        /// Token found where a number was required.
        token: String,
    },
    /// A numeric field was required but a quoted string was found.
    ExpectedNumber,
    /// The stream ended while a field was still being read.
    UnexpectedEnd,
    /// A binary tier or interval/point count was negative.
    NegativeCount {
        /// The negative count found.
        value: i32,
    },
    /// A binary text field's length prefix was negative but not the `-1`
    /// sentinel that flags a UTF-16 string.
    InvalidTextLength {
        /// The invalid length found.
        length: i16,
    },
    /// A binary document's tiers-exist flag byte was neither `0` nor `1`.
    InvalidTiersFlag {
        /// The byte found where the flag was expected.
        found: u8,
    },
    /// The parsed tiers failed to build a valid annotation, such as a raw
    /// identifier that leaves no room to allocate the next one, or a
    /// non-finite time value.
    Annotation(phx_annot::AnnotationError),
    /// The writer was asked to serialize a NaN or infinite time value.
    NonFiniteTime {
        /// The non-finite value found.
        value: f64,
    },
}

impl fmt::Display for TextGridError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "empty input"),
            Self::OddUtf16Length => write!(f, "UTF-16 stream has an odd byte length"),
            Self::InvalidUtf16 => write!(f, "UTF-16 stream contains an unpaired surrogate"),
            Self::InvalidUtf8 => write!(f, "byte-order-marked stream is not valid UTF-8"),
            Self::UnterminatedString => write!(f, "quoted label has no closing quote"),
            Self::NotATextGrid => write!(f, "missing or invalid ooTextFile header"),
            Self::UnsupportedObjectClass { found } => {
                write!(f, "object class {found:?} is not a TextGrid")
            }
            Self::UnknownTierClass { found } => write!(f, "unknown tier class {found:?}"),
            Self::InvalidNumber { token } => write!(f, "expected a number, found {token:?}"),
            Self::ExpectedNumber => write!(f, "expected a number, found a quoted string"),
            Self::UnexpectedEnd => write!(f, "input ended while reading a field"),
            Self::NegativeCount { value } => {
                write!(f, "expected a non-negative count, found {value}")
            }
            Self::InvalidTextLength { length } => {
                write!(f, "expected a text length of -1 or greater, found {length}")
            }
            Self::InvalidTiersFlag { found } => {
                write!(
                    f,
                    "expected a tiers-exist flag byte of 0 or 1, found {found}"
                )
            }
            Self::Annotation(err) => write!(f, "invalid annotation: {err}"),
            Self::NonFiniteTime { value } => {
                write!(f, "cannot write non-finite time value {value}")
            }
        }
    }
}

impl std::error::Error for TextGridError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Annotation(err) => Some(err),
            _ => None,
        }
    }
}

impl From<phx_annot::AnnotationError> for TextGridError {
    fn from(err: phx_annot::AnnotationError) -> Self {
        Self::Annotation(err)
    }
}
