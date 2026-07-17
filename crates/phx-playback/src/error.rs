//! Typed playback errors.

use std::error::Error;
use std::fmt;

/// What can go wrong opening or feeding an output stream.
///
/// [`PlaybackError::NoOutputDevice`] is the signal a caller uses to fall back
/// to another transport (the desktop shell drops to WebAudio when the native
/// host has no default output, as on a headless machine).
#[derive(Debug)]
pub enum PlaybackError {
    /// The host exposes no default output device.
    NoOutputDevice,
    /// The device's supported-config query failed.
    DeviceConfig(String),
    /// The device offers no output configuration this backend can drive.
    UnsupportedConfig(String),
    /// Building the cpal output stream failed.
    BuildStream(String),
    /// Starting the cpal output stream failed.
    StartStream(String),
    /// Resampling the source to the device rate failed.
    Resample(String),
    /// A control call arrived before any audio was loaded.
    NoAudioLoaded,
}

impl fmt::Display for PlaybackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoOutputDevice => write!(f, "no default audio output device"),
            Self::DeviceConfig(msg) => write!(f, "output device configuration error: {msg}"),
            Self::UnsupportedConfig(msg) => write!(f, "unsupported output configuration: {msg}"),
            Self::BuildStream(msg) => write!(f, "could not build the output stream: {msg}"),
            Self::StartStream(msg) => write!(f, "could not start the output stream: {msg}"),
            Self::Resample(msg) => write!(f, "could not resample to the device rate: {msg}"),
            Self::NoAudioLoaded => write!(f, "no audio is loaded"),
        }
    }
}

impl Error for PlaybackError {}
