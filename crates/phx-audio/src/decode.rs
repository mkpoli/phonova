//! AIFF and FLAC decoding through `symphonia`.
//!
//! `symphonia`'s per-sample-format conversion tables (`i16 -> f32` divides by
//! `32_768.0`, `i24 -> f32` by `8_388_608.0`, `i32 -> f32` by
//! `2_147_483_648.0`) use the same divisors as [`crate::read_integer_samples`]
//! for WAV, so a decoded AIFF (PCM) or FLAC (lossless) buffer equals its WAV
//! twin bit for bit.

use std::io::Cursor;

use symphonia::core::audio::GenericAudioBufferRef;
use symphonia::core::codecs::CodecParameters;
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::{MediaSource, MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;

use crate::AudioError;

/// The two symphonia-backed container formats this crate decodes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ContainerKind {
    Aiff,
    Flac,
}

impl ContainerKind {
    fn extension(self) -> &'static str {
        match self {
            Self::Aiff => "aiff",
            Self::Flac => "flac",
        }
    }

    fn malformed(self, reason: impl Into<String>) -> AudioError {
        match self {
            Self::Aiff => AudioError::MalformedAiff(reason.into()),
            Self::Flac => AudioError::MalformedFlac(reason.into()),
        }
    }

    fn unsupported(self, reason: impl Into<String>) -> AudioError {
        match self {
            Self::Aiff => AudioError::UnsupportedAiff {
                reason: reason.into(),
            },
            Self::Flac => AudioError::UnsupportedFlac {
                reason: reason.into(),
            },
        }
    }
}

/// Decodes `bytes` as `kind` into planar `f32` channels and the sample rate.
///
/// Every audio packet on the default track is decoded in stream order and
/// appended to its channel; the container's own frame accounting is never
/// trusted ahead of the decode; a decode failure surfaces as a typed
/// [`AudioError`] instead of a truncated buffer or a panic.
pub(crate) fn decode(
    bytes: &[u8],
    kind: ContainerKind,
) -> Result<(Vec<Vec<f32>>, f64), AudioError> {
    let source: Box<dyn MediaSource> = Box::new(Cursor::new(bytes.to_vec()));
    let mss = MediaSourceStream::new(source, MediaSourceStreamOptions::default());

    let mut hint = Hint::new();
    hint.with_extension(kind.extension());

    let mut format = symphonia::default::get_probe()
        .probe(
            &hint,
            mss,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|err| map_error(kind, err))?;

    let track = format
        .default_track(TrackType::Audio)
        .ok_or_else(|| kind.unsupported("no audio track found"))?;
    let track_id = track.id;
    let codec_params = match track.codec_params.clone() {
        Some(CodecParameters::Audio(params)) => params,
        _ => return Err(kind.unsupported("track has no audio codec parameters")),
    };

    let sample_rate = f64::from(
        codec_params
            .sample_rate
            .ok_or_else(|| kind.unsupported("missing sample rate"))?,
    );
    let declared_channels = codec_params
        .channels
        .as_ref()
        .map(|channels| channels.count());

    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(&codec_params, &AudioDecoderOptions::default())
        .map_err(|err| map_error(kind, err))?;

    let mut planar: Vec<Vec<f32>> = Vec::new();
    let mut scratch: Vec<Vec<f32>> = Vec::new();

    loop {
        let packet = match format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            Err(err) => return Err(map_error(kind, err)),
        };
        if packet.track_id != track_id {
            continue;
        }

        let decoded: GenericAudioBufferRef<'_> = decoder
            .decode(&packet)
            .map_err(|err| map_error(kind, err))?;
        decoded.copy_to_vecs_planar::<f32>(&mut scratch);

        if planar.is_empty() && !scratch.is_empty() {
            planar = vec![Vec::new(); scratch.len()];
        }
        if scratch.len() != planar.len() {
            return Err(kind.malformed("channel count changed mid-stream"));
        }
        for (dst, src) in planar.iter_mut().zip(&scratch) {
            dst.extend_from_slice(src);
        }
    }

    if planar.is_empty() {
        let channels = declared_channels
            .ok_or_else(|| kind.unsupported("stream has no decodable audio frames"))?;
        planar = vec![Vec::new(); channels];
    }

    Ok((planar, sample_rate))
}

fn map_error(kind: ContainerKind, err: SymphoniaError) -> AudioError {
    match err {
        SymphoniaError::IoError(io_err) => kind.malformed(io_err.to_string()),
        SymphoniaError::DecodeError(reason) => kind.malformed(reason.to_string()),
        SymphoniaError::Unsupported(reason) => kind.unsupported(reason.to_string()),
        SymphoniaError::LimitError(reason) => {
            kind.unsupported(format!("decode limit reached: {reason}"))
        }
        other => kind.malformed(other.to_string()),
    }
}
