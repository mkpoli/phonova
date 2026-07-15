//! Audio buffers, WAV I/O, and sample-rate conversion.
//!
//! The crate stores audio as planar `f32` channels with an `f64` sample rate.
//! WAV reading and writing uses `hound`. Resampling uses `rubato`'s
//! windowed-sinc interpolator.

#![warn(missing_docs)]

use std::borrow::Cow;
use std::error::Error;
use std::fmt;
use std::io::Cursor;
use std::ops::Range;

use rubato::audioadapter_buffers::owned::{InterleavedOwned, SequentialOwned};
use rubato::{
    Async, FixedAsync, Resampler, SincInterpolationParameters, SincInterpolationType,
    WindowFunction,
};

const MAX_PLANAR_ALLOCATION_BYTES: usize = 2 * 1024 * 1024 * 1024;

/// Planar `f32` audio samples with an `f64` sample rate.
#[derive(Clone, Debug, PartialEq)]
pub struct Audio {
    channels: Vec<Vec<f32>>,
    sample_rate: f64,
    name: Option<String>,
}

impl Audio {
    /// Builds an audio buffer from planar channels.
    ///
    /// Every channel must have the same number of samples. The total planar
    /// `f32` storage is rejected before allocation when it exceeds 2 GiB.
    pub fn new(channels: Vec<Vec<f32>>, sample_rate: f64) -> Result<Self, AudioError> {
        validate_sample_rate(sample_rate)?;
        validate_channels(&channels)?;
        let frames = channels.first().map_or(0, Vec::len);
        check_planar_allocation(channels.len(), frames)?;
        Ok(Self {
            channels,
            sample_rate,
            name: None,
        })
    }

    /// Reads a RIFF/WAVE byte buffer into planar normalized `f32` samples.
    ///
    /// Integer PCM at 8, 16, 24, and 32 bits is normalized to approximately
    /// `[-1.0, 1.0]`. IEEE 32-bit float WAV samples are read as stored.
    pub fn from_wav_bytes(bytes: &[u8]) -> Result<Self, AudioError> {
        let cursor = Cursor::new(bytes);
        let mut reader = hound::WavReader::new(cursor).map_err(map_hound_read_error)?;
        let spec = reader.spec();
        let channels = usize::from(spec.channels);

        if channels == 0 {
            return Err(AudioError::InvalidChannelCount { channels });
        }

        let frames = usize::try_from(reader.duration()).map_err(|_| {
            AudioError::MalformedWav("WAV duration cannot fit in memory size".to_string())
        })?;
        check_planar_allocation(channels, frames)?;

        let mut planar = (0..channels)
            .map(|_| Vec::with_capacity(frames))
            .collect::<Vec<_>>();

        match (spec.sample_format, spec.bits_per_sample) {
            (hound::SampleFormat::Int, 8) => {
                read_integer_samples::<i32>(&mut reader, &mut planar, 128.0)?
            }
            (hound::SampleFormat::Int, 16) => {
                read_integer_samples::<i32>(&mut reader, &mut planar, 32_768.0)?
            }
            (hound::SampleFormat::Int, 24) => {
                read_integer_samples::<i32>(&mut reader, &mut planar, 8_388_608.0)?
            }
            (hound::SampleFormat::Int, 32) => {
                read_integer_samples::<i32>(&mut reader, &mut planar, 2_147_483_648.0)?
            }
            (hound::SampleFormat::Float, 32) => read_float_samples(&mut reader, &mut planar)?,
            (format, bits) => {
                return Err(AudioError::UnsupportedWav {
                    reason: format!("unsupported WAV sample format {format:?} at {bits} bits"),
                });
            }
        }

        Ok(Self {
            channels: planar,
            sample_rate: f64::from(spec.sample_rate),
            name: None,
        })
    }

    /// Encodes the buffer as RIFF/WAVE bytes at the requested sample format.
    ///
    /// PCM output is clipped to `[-1.0, 1.0]` before quantization.
    pub fn to_wav_bytes(&self, bits: BitDepth) -> Result<Vec<u8>, AudioError> {
        let channels =
            u16::try_from(self.channels.len()).map_err(|_| AudioError::UnsupportedWav {
                reason: "WAV channel count exceeds u16".to_string(),
            })?;
        let sample_rate = wav_sample_rate(self.sample_rate)?;
        let spec = hound::WavSpec {
            channels,
            sample_rate,
            bits_per_sample: bits.bits_per_sample(),
            sample_format: bits.sample_format(),
        };

        let mut cursor = Cursor::new(Vec::new());
        {
            let mut writer =
                hound::WavWriter::new(&mut cursor, spec).map_err(map_hound_write_error)?;

            for frame in 0..self.frames() {
                for channel in &self.channels {
                    let sample = channel[frame];
                    match bits {
                        BitDepth::Pcm8 => writer
                            .write_sample(quantize_pcm(sample, 8)? as i8)
                            .map_err(map_hound_write_error)?,
                        BitDepth::Pcm16 => writer
                            .write_sample(quantize_pcm(sample, 16)? as i16)
                            .map_err(map_hound_write_error)?,
                        BitDepth::Pcm24 => writer
                            .write_sample(quantize_pcm(sample, 24)?)
                            .map_err(map_hound_write_error)?,
                        BitDepth::Pcm32 => writer
                            .write_sample(quantize_pcm(sample, 32)?)
                            .map_err(map_hound_write_error)?,
                        BitDepth::Float32 => {
                            writer.write_sample(sample).map_err(map_hound_write_error)?
                        }
                    }
                }
            }

            writer.finalize().map_err(map_hound_write_error)?;
        }

        Ok(cursor.into_inner())
    }

    /// Returns a borrowed channel by index.
    ///
    /// Panics when `i` is outside `0..self.channel_count()`.
    pub fn channel(&self, i: usize) -> &[f32] {
        &self.channels[i]
    }

    /// Returns a mono view or a freshly mixed mono buffer.
    ///
    /// Single-channel audio is borrowed. Multi-channel audio is mixed by taking
    /// the arithmetic mean of all channels at each sample index.
    pub fn mono_mix(&self) -> Cow<'_, [f32]> {
        match self.channels.as_slice() {
            [] => Cow::Borrowed(&[]),
            [single] => Cow::Borrowed(single),
            channels => {
                let mut mixed = vec![0.0; self.frames()];
                for channel in channels {
                    for (dst, sample) in mixed.iter_mut().zip(channel) {
                        *dst += *sample;
                    }
                }
                let scale = 1.0 / channels.len() as f32;
                for sample in &mut mixed {
                    *sample *= scale;
                }
                Cow::Owned(mixed)
            }
        }
    }

    /// Returns the duration in seconds.
    pub fn duration(&self) -> f64 {
        self.frames() as f64 / self.sample_rate
    }

    /// Returns a resampled copy at `target_hz`.
    ///
    /// The implementation uses `rubato::Async::new_sinc` with the selected
    /// windowed-sinc quality preset.
    pub fn resampled(&self, target_hz: f64, quality: ResampleQuality) -> Result<Self, AudioError> {
        validate_sample_rate(target_hz)?;
        if self.channels.is_empty() || self.frames() == 0 {
            return Ok(Self {
                channels: self.channels.clone(),
                sample_rate: target_hz,
                name: self.name.clone(),
            });
        }

        if (target_hz - self.sample_rate).abs() <= f64::EPSILON {
            let mut cloned = self.clone();
            cloned.sample_rate = target_hz;
            return Ok(cloned);
        }

        let output_frames = (self.frames() as f64 * target_hz / self.sample_rate).ceil() as usize;
        check_planar_allocation(self.channels.len(), output_frames)?;

        let input = SequentialOwned::new_from(
            self.channels.iter().flatten().copied().collect::<Vec<_>>(),
            self.channels.len(),
            self.frames(),
        )
        .map_err(|_| AudioError::ChannelCountMismatch {
            expected: self.channels.len() * self.frames(),
            actual: self.channels.iter().map(Vec::len).sum(),
        })?;

        let params = quality.sinc_parameters();
        let mut resampler = Async::<f32>::new_sinc(
            target_hz / self.sample_rate,
            1.0,
            &params,
            self.frames().clamp(1, 4096),
            self.channels.len(),
            FixedAsync::Input,
        )
        .map_err(|err| AudioError::Resample(err.to_string()))?;

        let output = resampler
            .process_all(&input, self.frames(), None)
            .map_err(|err| AudioError::Resample(err.to_string()))?;

        let actual_frames = output_frames_from_interleaved(&output);
        check_planar_allocation(self.channels.len(), actual_frames)?;
        let interleaved = output.take_data();
        let mut channels = (0..self.channels.len())
            .map(|_| Vec::with_capacity(actual_frames))
            .collect::<Vec<_>>();
        for frame in interleaved.chunks_exact(self.channels.len()) {
            for (channel, sample) in channels.iter_mut().zip(frame) {
                channel.push(*sample);
            }
        }

        Ok(Self {
            channels,
            sample_rate: target_hz,
            name: self.name.clone(),
        })
    }

    /// Returns a borrowed sample range without copying sample data.
    ///
    /// Panics when the range lies outside `0..self.frames()`.
    pub fn slice_samples(&self, range: Range<usize>) -> AudioView<'_> {
        assert!(range.start <= range.end);
        assert!(range.end <= self.frames());
        AudioView { audio: self, range }
    }

    /// Returns the number of channels.
    pub fn channel_count(&self) -> usize {
        self.channels.len()
    }

    /// Returns the number of samples in each channel.
    pub fn frames(&self) -> usize {
        self.channels.first().map_or(0, Vec::len)
    }

    /// Returns the sample rate in hertz.
    pub fn sample_rate(&self) -> f64 {
        self.sample_rate
    }

    /// Returns the optional buffer name.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Sets the optional buffer name.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
}

/// A borrowed sample-range view into an [`Audio`] buffer.
#[derive(Clone, Debug)]
pub struct AudioView<'a> {
    audio: &'a Audio,
    range: Range<usize>,
}

impl<'a> AudioView<'a> {
    /// Returns a borrowed channel slice by index.
    ///
    /// Panics when `i` is outside `0..self.channel_count()`.
    pub fn channel(&self, i: usize) -> &'a [f32] {
        &self.audio.channels[i][self.range.clone()]
    }

    /// Returns a mono view or a freshly mixed mono buffer for the view range.
    pub fn mono_mix(&self) -> Cow<'a, [f32]> {
        match self.audio.channels.as_slice() {
            [] => Cow::Borrowed(&[]),
            [single] => Cow::Borrowed(&single[self.range.clone()]),
            channels => {
                let mut mixed = vec![0.0; self.frames()];
                for channel in channels {
                    for (dst, sample) in mixed.iter_mut().zip(&channel[self.range.clone()]) {
                        *dst += *sample;
                    }
                }
                let scale = 1.0 / channels.len() as f32;
                for sample in &mut mixed {
                    *sample *= scale;
                }
                Cow::Owned(mixed)
            }
        }
    }

    /// Returns the view duration in seconds.
    pub fn duration(&self) -> f64 {
        self.frames() as f64 / self.sample_rate()
    }

    /// Returns the number of channels.
    pub fn channel_count(&self) -> usize {
        self.audio.channel_count()
    }

    /// Returns the number of samples in each channel of the view.
    pub fn frames(&self) -> usize {
        self.range.end - self.range.start
    }

    /// Returns the sample rate in hertz.
    pub fn sample_rate(&self) -> f64 {
        self.audio.sample_rate
    }

    /// Returns the optional buffer name from the source audio.
    pub fn name(&self) -> Option<&'a str> {
        self.audio.name()
    }
}

/// WAV output bit depth and sample format.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BitDepth {
    /// 8-bit unsigned PCM in the WAV stream, represented as signed PCM through `hound`.
    Pcm8,
    /// 16-bit signed PCM.
    Pcm16,
    /// 24-bit signed PCM.
    Pcm24,
    /// 32-bit signed PCM.
    Pcm32,
    /// 32-bit IEEE float.
    Float32,
}

impl BitDepth {
    fn bits_per_sample(self) -> u16 {
        match self {
            Self::Pcm8 => 8,
            Self::Pcm16 => 16,
            Self::Pcm24 => 24,
            Self::Pcm32 | Self::Float32 => 32,
        }
    }

    fn sample_format(self) -> hound::SampleFormat {
        match self {
            Self::Float32 => hound::SampleFormat::Float,
            Self::Pcm8 | Self::Pcm16 | Self::Pcm24 | Self::Pcm32 => hound::SampleFormat::Int,
        }
    }
}

/// Resampling quality preset.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ResampleQuality {
    /// Offline windowed-sinc resampling with a Blackman-Harris window.
    ///
    /// `sinc_len = 256`, `oversampling_factor = 128`, cubic interpolation, and
    /// an anti-alias cutoff at the destination Nyquist (`f_cutoff = 1.0`, the
    /// fraction of the lower of the source and destination Nyquist frequencies).
    ///
    /// The cutoff is placed at the destination Nyquist so the anti-alias filter
    /// keeps the full band below it and removes only content above it, the
    /// textbook anti-aliasing rule (Oppenheim & Schafer, multirate chapter).
    /// rubato's automatic cutoff instead backs off below that Nyquist to widen
    /// the window's stopband margin, discarding a band of valid signal just
    /// under the destination Nyquist; for spectral analysis that band is
    /// wanted, so the cutoff is pinned to the Nyquist itself.
    #[default]
    Best,
}

impl ResampleQuality {
    fn sinc_parameters(self) -> SincInterpolationParameters {
        match self {
            Self::Best => SincInterpolationParameters::new(256, WindowFunction::BlackmanHarris2)
                .oversampling_factor(128)
                .interpolation(SincInterpolationType::Cubic)
                .f_cutoff(1.0),
        }
    }
}

/// Errors produced by audio decoding, encoding, buffer validation, and resampling.
#[derive(Debug, Clone, PartialEq)]
pub enum AudioError {
    /// WAV data is malformed.
    MalformedWav(String),
    /// WAV data uses a format outside this crate's supported sample formats.
    UnsupportedWav {
        /// Human-readable reason.
        reason: String,
    },
    /// Channel count or channel length validation failed.
    ChannelCountMismatch {
        /// Expected count.
        expected: usize,
        /// Actual count.
        actual: usize,
    },
    /// Channel count is invalid for an audio buffer.
    InvalidChannelCount {
        /// Invalid channel count.
        channels: usize,
    },
    /// Sample rate is outside the supported finite positive range.
    InvalidSampleRate {
        /// Invalid sample rate.
        sample_rate: f64,
    },
    /// The requested planar `f32` sample storage exceeds the crate limit.
    OversizedAllocation {
        /// Requested bytes.
        requested_bytes: usize,
        /// Maximum accepted bytes.
        limit_bytes: usize,
    },
    /// WAV writing failed.
    WavWrite(String),
    /// Resampling failed.
    Resample(String),
}

impl fmt::Display for AudioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedWav(reason) => write!(f, "malformed WAV data: {reason}"),
            Self::UnsupportedWav { reason } => write!(f, "unsupported WAV data: {reason}"),
            Self::ChannelCountMismatch { expected, actual } => {
                write!(
                    f,
                    "channel count mismatch: expected {expected}, got {actual}"
                )
            }
            Self::InvalidChannelCount { channels } => {
                write!(f, "invalid channel count: {channels}")
            }
            Self::InvalidSampleRate { sample_rate } => {
                write!(f, "invalid sample rate: {sample_rate}")
            }
            Self::OversizedAllocation {
                requested_bytes,
                limit_bytes,
            } => write!(
                f,
                "audio allocation would request {requested_bytes} bytes, above the {limit_bytes} byte limit"
            ),
            Self::WavWrite(reason) => write!(f, "WAV writing failed: {reason}"),
            Self::Resample(reason) => write!(f, "resampling failed: {reason}"),
        }
    }
}

impl Error for AudioError {}

fn validate_sample_rate(sample_rate: f64) -> Result<(), AudioError> {
    if sample_rate.is_finite() && sample_rate > 0.0 {
        Ok(())
    } else {
        Err(AudioError::InvalidSampleRate { sample_rate })
    }
}

fn wav_sample_rate(sample_rate: f64) -> Result<u32, AudioError> {
    validate_sample_rate(sample_rate)?;
    let rounded = sample_rate.round();
    if (sample_rate - rounded).abs() <= f64::EPSILON
        && rounded >= 1.0
        && rounded <= f64::from(u32::MAX)
    {
        Ok(rounded as u32)
    } else {
        Err(AudioError::InvalidSampleRate { sample_rate })
    }
}

fn validate_channels(channels: &[Vec<f32>]) -> Result<(), AudioError> {
    let Some(first) = channels.first() else {
        return Err(AudioError::InvalidChannelCount { channels: 0 });
    };
    let expected = first.len();
    for channel in channels {
        if channel.len() != expected {
            return Err(AudioError::ChannelCountMismatch {
                expected,
                actual: channel.len(),
            });
        }
    }
    Ok(())
}

fn check_planar_allocation(channels: usize, frames: usize) -> Result<(), AudioError> {
    let requested_bytes = channels
        .checked_mul(frames)
        .and_then(|samples| samples.checked_mul(size_of::<f32>()))
        .ok_or(AudioError::OversizedAllocation {
            requested_bytes: usize::MAX,
            limit_bytes: MAX_PLANAR_ALLOCATION_BYTES,
        })?;

    if requested_bytes > MAX_PLANAR_ALLOCATION_BYTES {
        Err(AudioError::OversizedAllocation {
            requested_bytes,
            limit_bytes: MAX_PLANAR_ALLOCATION_BYTES,
        })
    } else {
        Ok(())
    }
}

fn read_integer_samples<S>(
    reader: &mut hound::WavReader<Cursor<&[u8]>>,
    planar: &mut [Vec<f32>],
    divisor: f32,
) -> Result<(), AudioError>
where
    S: hound::Sample + Into<i32>,
{
    let channels = planar.len();
    for (index, sample) in reader.samples::<S>().enumerate() {
        let sample = sample.map_err(map_hound_read_error)?;
        planar[index % channels].push(sample.into() as f32 / divisor);
    }
    Ok(())
}

fn read_float_samples(
    reader: &mut hound::WavReader<Cursor<&[u8]>>,
    planar: &mut [Vec<f32>],
) -> Result<(), AudioError> {
    let channels = planar.len();
    for (index, sample) in reader.samples::<f32>().enumerate() {
        let sample = sample.map_err(map_hound_read_error)?;
        planar[index % channels].push(sample);
    }
    Ok(())
}

fn quantize_pcm(sample: f32, bits: u16) -> Result<i32, AudioError> {
    let sample = sample.clamp(-1.0, 1.0) as f64;
    let min = -(1_i64 << (bits - 1));
    let max = (1_i64 << (bits - 1)) - 1;
    let quantized = if sample <= -1.0 {
        min
    } else {
        (sample * max as f64).round() as i64
    };
    i32::try_from(quantized).map_err(|_| AudioError::UnsupportedWav {
        reason: format!("{bits}-bit PCM sample cannot fit i32"),
    })
}

fn output_frames_from_interleaved(buffer: &InterleavedOwned<f32>) -> usize {
    use rubato::audioadapter::Adapter;

    buffer.frames()
}

fn map_hound_read_error(err: hound::Error) -> AudioError {
    match err {
        hound::Error::Unsupported | hound::Error::InvalidSampleFormat | hound::Error::TooWide => {
            AudioError::UnsupportedWav {
                reason: err.to_string(),
            }
        }
        hound::Error::IoError(_)
        | hound::Error::FormatError(_)
        | hound::Error::UnfinishedSample => AudioError::MalformedWav(err.to_string()),
    }
}

fn map_hound_write_error(err: hound::Error) -> AudioError {
    AudioError::WavWrite(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    fn synthetic_audio(channels: usize) -> Audio {
        let frames = 257;
        let mut planar = Vec::new();
        for channel in 0..channels {
            let mut samples = Vec::new();
            for frame in 0..frames {
                let phase = frame as f32 * 0.037 + channel as f32 * 0.19;
                samples.push((phase.sin() * 0.8).clamp(-1.0, 1.0));
            }
            planar.push(samples);
        }
        Audio::new(planar, 48_000.0).unwrap()
    }

    fn assert_round_trip(bit_depth: BitDepth, channels: usize, tolerance: f32) {
        let audio = synthetic_audio(channels);
        let bytes = audio.to_wav_bytes(bit_depth).unwrap();
        let decoded = Audio::from_wav_bytes(&bytes).unwrap();
        assert_eq!(decoded.channel_count(), channels);
        assert_eq!(decoded.frames(), audio.frames());
        assert_eq!(decoded.sample_rate(), audio.sample_rate());
        for channel in 0..channels {
            for (actual, expected) in decoded.channel(channel).iter().zip(audio.channel(channel)) {
                assert!(
                    (actual - expected).abs() <= tolerance,
                    "{actual} differs from {expected} by more than {tolerance}"
                );
            }
        }
    }

    #[test]
    fn wav_round_trip_pcm16_mono_and_stereo() {
        assert_round_trip(BitDepth::Pcm16, 1, 1.5 / 32_768.0);
        assert_round_trip(BitDepth::Pcm16, 2, 1.5 / 32_768.0);
    }

    #[test]
    fn wav_round_trip_pcm24_mono_and_stereo() {
        assert_round_trip(BitDepth::Pcm24, 1, 1.0 / 8_388_608.0);
        assert_round_trip(BitDepth::Pcm24, 2, 1.0 / 8_388_608.0);
    }

    #[test]
    fn wav_round_trip_pcm32_mono_and_stereo() {
        assert_round_trip(BitDepth::Pcm32, 1, 1.0 / 2_147_483_648.0);
        assert_round_trip(BitDepth::Pcm32, 2, 1.0 / 2_147_483_648.0);
    }

    #[test]
    fn wav_round_trip_float32_reads_samples_unchanged() {
        assert_round_trip(BitDepth::Float32, 1, 0.0);
        assert_round_trip(BitDepth::Float32, 2, 0.0);
    }

    #[test]
    fn mono_mix_borrows_single_channel() {
        let audio = synthetic_audio(1);
        assert!(matches!(audio.mono_mix(), Cow::Borrowed(_)));
    }

    #[test]
    fn slice_samples_borrows_range() {
        let audio = synthetic_audio(2);
        let view = audio.slice_samples(10..20);
        assert_eq!(view.frames(), 10);
        assert_eq!(view.channel(1), &audio.channel(1)[10..20]);
    }

    #[test]
    fn resample_preserves_tone_frequency() {
        let source_hz = 44_100.0;
        let target_hz = 16_000.0;
        let tone_hz = 440.0;
        let frames = source_hz as usize;
        let samples = (0..frames)
            .map(|frame| {
                let phase = 2.0 * PI * tone_hz as f32 * frame as f32 / source_hz as f32;
                phase.sin() * 0.5
            })
            .collect::<Vec<_>>();
        let audio = Audio::new(vec![samples], source_hz).unwrap();

        let resampled = audio.resampled(target_hz, ResampleQuality::Best).unwrap();
        assert_eq!(resampled.sample_rate(), target_hz);
        assert_eq!(resampled.frames(), target_hz as usize);

        let peak_bin = peak_bin_near(resampled.channel(0), target_hz, 430, 450);
        let peak_frequency = peak_bin as f64 * target_hz / resampled.frames() as f64;
        assert!(
            (peak_frequency - tone_hz).abs() <= 0.1,
            "peak frequency {peak_frequency} Hz"
        );
    }

    #[test]
    fn oversized_planar_allocation_is_rejected() {
        let max_samples = MAX_PLANAR_ALLOCATION_BYTES / size_of::<f32>();
        let err = check_planar_allocation(2, max_samples / 2 + 1).unwrap_err();
        assert!(matches!(err, AudioError::OversizedAllocation { .. }));
    }

    fn peak_bin_near(
        samples: &[f32],
        sample_rate: f64,
        first_bin: usize,
        last_bin: usize,
    ) -> usize {
        let _ = sample_rate;
        (first_bin..=last_bin)
            .max_by(|left, right| {
                let left_mag = dft_magnitude(samples, *left);
                let right_mag = dft_magnitude(samples, *right);
                left_mag.total_cmp(&right_mag)
            })
            .unwrap()
    }

    fn dft_magnitude(samples: &[f32], bin: usize) -> f64 {
        let len = samples.len() as f64;
        let mut real = 0.0;
        let mut imag = 0.0;
        for (index, sample) in samples.iter().enumerate() {
            let angle = -2.0 * std::f64::consts::PI * bin as f64 * index as f64 / len;
            real += f64::from(*sample) * angle.cos();
            imag += f64::from(*sample) * angle.sin();
        }
        real.hypot(imag)
    }
}
