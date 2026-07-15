//! External media references and content-addressed re-linking.
//!
//! A project never embeds audio. Each recording is referenced by a path
//! relative to the project file plus the BLAKE3 hash of the file's bytes. When
//! a referenced file is absent, a matching file is found by hashing candidates
//! in sibling directories, so a moved recording re-links without user input.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Stable identifier for a referenced recording within one project.
#[derive(Clone, Copy, Debug, Hash, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct MediaId(u64);

impl MediaId {
    /// Wraps a raw value as a media identifier.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the numeric value carried by this identifier.
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for MediaId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A BLAKE3 content hash of a media file's bytes.
///
/// Serialized as a 64-character lowercase hex string so the manifest stays
/// readable and third-party tools can verify it without this crate.
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
pub struct ContentHash([u8; 32]);

impl ContentHash {
    /// Hashes the given bytes with BLAKE3.
    pub fn of(bytes: &[u8]) -> Self {
        Self(*blake3::hash(bytes).as_bytes())
    }

    /// Returns the raw 32-byte digest.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Formats the digest as a 64-character lowercase hex string.
    pub fn to_hex(self) -> String {
        let mut s = String::with_capacity(64);
        for byte in self.0 {
            s.push(hex_digit(byte >> 4));
            s.push(hex_digit(byte & 0x0f));
        }
        s
    }

    /// Parses a 64-character hex string back into a digest.
    pub fn from_hex(text: &str) -> Result<Self, HashParseError> {
        let bytes = text.as_bytes();
        if bytes.len() != 64 {
            return Err(HashParseError::Length(bytes.len()));
        }
        let mut out = [0u8; 32];
        for (i, out_byte) in out.iter_mut().enumerate() {
            let hi = hex_value(bytes[i * 2]).ok_or(HashParseError::Digit)?;
            let lo = hex_value(bytes[i * 2 + 1]).ok_or(HashParseError::Digit)?;
            *out_byte = (hi << 4) | lo;
        }
        Ok(Self(out))
    }
}

impl fmt::Display for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl Serialize for ContentHash {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for ContentHash {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        ContentHash::from_hex(&text).map_err(serde::de::Error::custom)
    }
}

/// Failure to parse a hex-encoded content hash.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HashParseError {
    /// The string did not contain exactly 64 hex characters.
    Length(usize),
    /// A character outside `0-9a-fA-F` was present.
    Digit,
}

impl fmt::Display for HashParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length(n) => write!(f, "content hash must be 64 hex characters, found {n}"),
            Self::Digit => write!(f, "content hash contains a non-hex character"),
        }
    }
}

impl std::error::Error for HashParseError {}

const fn hex_digit(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        _ => (b'a' + (nibble - 10)) as char,
    }
}

const fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// A reference to one external recording.
///
/// `relative_path` is resolved against the directory that holds the project
/// file. `duration`, `sample_rate`, and `channels` are recorded so a project
/// can list its recordings without opening the media, and so re-linking can
/// reject a file whose shape no longer matches.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MediaRef {
    /// Identifier used by annotations and profiles to refer to this recording.
    pub id: MediaId,
    /// Location of the media file relative to the project file's directory.
    pub relative_path: String,
    /// BLAKE3 hash of the media file's bytes.
    pub hash: ContentHash,
    /// Duration in seconds.
    pub duration: f64,
    /// Sample rate in hertz.
    pub sample_rate: f64,
    /// Channel count.
    pub channels: usize,
}

impl MediaRef {
    /// Builds a reference by hashing WAV bytes and decoding their shape.
    ///
    /// The hash covers the file bytes exactly as stored, so a byte-identical
    /// copy in another directory re-links by hash. Decoding reads only the
    /// header shape needed for the manifest.
    pub fn from_wav_bytes(
        id: MediaId,
        relative_path: impl Into<String>,
        bytes: &[u8],
    ) -> Result<Self, phx_audio::AudioError> {
        let audio = phx_audio::Audio::from_wav_bytes(bytes)?;
        Ok(Self {
            id,
            relative_path: relative_path.into(),
            hash: ContentHash::of(bytes),
            duration: audio.duration(),
            sample_rate: audio.sample_rate(),
            channels: audio.channel_count(),
        })
    }
}

/// A file offered as a re-link match for a missing recording.
#[derive(Clone, Debug, PartialEq)]
pub struct MediaCandidate {
    /// Path of the candidate relative to the project file's directory.
    pub relative_path: String,
    /// Content hash of the candidate.
    pub hash: ContentHash,
}

/// Outcome of resolving one media reference against the filesystem.
#[derive(Clone, Debug, PartialEq)]
pub enum MediaResolution {
    /// The referenced file is present and its hash matches.
    Present(MediaId),
    /// A byte-identical file was found elsewhere and the reference now points to it.
    Relinked {
        /// The recording that moved.
        media: MediaId,
        /// The path the reference previously held.
        from: String,
        /// The path the reference now holds.
        to: String,
    },
}

/// A recording whose file could not be resolved automatically.
///
/// `candidates` lists files whose hash matches the expected content but which
/// could not be chosen without the user, because more than one matched. An
/// empty list means no matching file was found in the searched directories.
#[derive(Clone, Debug, PartialEq)]
pub struct MediaGap {
    /// The unresolved recording.
    pub media: MediaId,
    /// The path the reference held.
    pub original_path: String,
    /// The content hash the recording must match.
    pub expected_hash: ContentHash,
    /// Hash-matching files the user may pick from.
    pub candidates: Vec<MediaCandidate>,
}

impl fmt::Display for MediaGap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "media {} ({}) missing; {} matching candidate(s)",
            self.media,
            self.original_path,
            self.candidates.len()
        )
    }
}
