//! The one API both frontends and future bindings consume: commands with
//! explicit arguments, journaled unified undo, content-addressed analysis
//! cache.
//!
//! This crate currently implements the walking-skeleton slice of that
//! surface: an audio store keyed by [`AudioId`], a cached waveform
//! min/max pyramid, and dB→RGBA spectrogram tiles. Undo journaling and the
//! remaining analysis commands (pitch, formants, intensity, voice report)
//! arrive with their crates in later tasks.
#![warn(missing_docs)]

mod error;
mod pyramid;
mod store;

use phx_audio::Audio;
use phx_dsp::Window;

pub use error::EngineError;
pub use phx_render::{Colormap, DisplayMapping, Theme};
pub use phx_spectrogram::{SpectrogramParams, Tile, TileRequest};
pub use pyramid::MinMax;
pub use store::{AudioId, AudioInfo, AudioStore};

/// Session engine: the audio store plus the pure functions that read it.
///
/// Every method beyond store bookkeeping is stateless-by-arguments — the
/// same `(id, params)` pair always produces the same result, independent of
/// call order, viewport, or any other implicit state (rule 1,
/// `docs/plan/architecture.md`).
#[derive(Default)]
pub struct Engine {
    store: AudioStore,
}

impl Engine {
    /// Creates an engine with an empty audio store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Decodes RIFF/WAVE bytes and returns the id of the new store entry.
    ///
    /// # Errors
    /// Returns [`EngineError::Audio`] when the bytes are not a WAV file this
    /// crate can decode (see [`phx_audio::Audio::from_wav_bytes`]).
    pub fn import_wav_bytes(&mut self, bytes: &[u8]) -> Result<AudioId, EngineError> {
        let audio = Audio::from_wav_bytes(bytes)?;
        Ok(self.store.insert(audio))
    }

    /// Returns duration, sample rate, channel count, and name for `id`.
    ///
    /// # Errors
    /// Returns [`EngineError::UnknownAudioId`] when `id` does not name a
    /// live store entry.
    pub fn audio_info(&self, id: AudioId) -> Result<AudioInfo, EngineError> {
        let audio = self.store.audio(id)?;
        Ok(AudioInfo {
            duration: audio.duration(),
            sample_rate: audio.sample_rate(),
            channels: audio.channel_count(),
            name: audio.name().map(str::to_owned),
        })
    }

    /// Returns `px` [`MinMax`] buckets covering `[t0, t1)` seconds of `id`,
    /// read from its cached waveform pyramid.
    ///
    /// `t0`/`t1` may be given in either order and are clamped to the
    /// signal's duration; each bucket's min/max agrees exactly with a direct
    /// scan of the same underlying sample range (see the [`pyramid`] module
    /// doc for why the pyramid combine is exact, not approximate).
    ///
    /// # Errors
    /// Returns [`EngineError::UnknownAudioId`] when `id` does not name a
    /// live store entry, and [`EngineError::InvalidRequest`] when `t0`/`t1`
    /// are not finite.
    pub fn waveform_slice(
        &self,
        id: AudioId,
        t0: f64,
        t1: f64,
        px: u32,
    ) -> Result<Vec<MinMax>, EngineError> {
        if !t0.is_finite() || !t1.is_finite() {
            return Err(EngineError::InvalidRequest {
                reason: "waveform_slice t0/t1 must be finite".to_string(),
            });
        }
        let pyramid = self.store.pyramid(id)?;
        Ok(pyramid.slice(t0, t1, px))
    }

    /// Computes a spectrogram tile for `id` and colorizes it to RGBA bytes.
    ///
    /// Composes [`phx_spectrogram::compute_tile`] (raw PSD-derived dB,
    /// snapped to the object-level frame grid so adjacent tile requests
    /// share columns exactly) with [`phx_render::colorize`] (linear-in-dB
    /// clip against `display`, then a perceptual colormap lookup tuned for
    /// `theme`). The whole audio buffer is always passed to
    /// `compute_tile` — never just the `[t0, t1)` window `req` names — so
    /// the frame grid stays a function of the signal alone, not the
    /// viewport.
    ///
    /// Returns `4 * req.width_px * req.height_px` bytes, `R, G, B, A` per
    /// pixel, row 0 first.
    ///
    /// # Errors
    /// Returns [`EngineError::UnknownAudioId`] when `id` does not name a
    /// live store entry, and [`EngineError::InvalidRequest`] when `req`
    /// carries a non-finite bound or a non-positive analysis parameter, or
    /// when the audio is too short for the requested window to produce a
    /// single analysis frame.
    pub fn spectrogram_tile_rgba(
        &self,
        id: AudioId,
        req: &TileRequest,
        display: &DisplayMapping,
        colormap: Colormap,
        theme: Theme,
    ) -> Result<Vec<u8>, EngineError> {
        validate_tile_request(req)?;
        let audio = self.store.audio(id)?;
        let view = audio.slice_samples(0..audio.frames());
        let tile = phx_spectrogram::compute_tile(view, req);

        let expected_len = req.width_px as usize * req.height_px as usize;
        if tile.db.len() != expected_len {
            return Err(EngineError::InvalidRequest {
                reason: format!(
                    "tile produced {} values for a {}x{} request; the audio is likely too \
                     short, or the time/frequency range too narrow, to fit a single analysis \
                     frame",
                    tile.db.len(),
                    req.width_px,
                    req.height_px
                ),
            });
        }

        Ok(phx_render::colorize(
            &tile.db,
            req.width_px,
            req.height_px,
            display,
            colormap,
            theme,
        ))
    }
}

/// Validates a [`TileRequest`] before it reaches `phx_spectrogram`.
///
/// `phx_spectrogram::compute_tile` asserts these same properties and panics
/// on violation, which is the right contract for a pure math crate calling
/// itself internally with already-validated data. The engine is the
/// boundary that untrusted callers reach, so it re-checks the same
/// properties here and turns a would-be panic into a typed error.
fn validate_tile_request(req: &TileRequest) -> Result<(), EngineError> {
    let invalid = |reason: &str| {
        Err(EngineError::InvalidRequest {
            reason: reason.to_string(),
        })
    };

    if !req.t0.is_finite() || !req.t1.is_finite() {
        return invalid("t0/t1 must be finite");
    }
    if !req.f0.is_finite() || !req.f1.is_finite() {
        return invalid("f0/f1 must be finite");
    }
    let params = &req.params;
    if !(params.window_length.is_finite() && params.window_length > 0.0) {
        return invalid("params.window_length must be finite and positive");
    }
    if !(params.max_frequency.is_finite() && params.max_frequency >= 0.0) {
        return invalid("params.max_frequency must be finite and non-negative");
    }
    if !(params.time_step.is_finite() && params.time_step > 0.0) {
        return invalid("params.time_step must be finite and positive");
    }
    if !(params.frequency_step.is_finite() && params.frequency_step > 0.0) {
        return invalid("params.frequency_step must be finite and positive");
    }
    if let Window::Gaussian {
        effective_len_factor,
    } = params.window
        && !(effective_len_factor.is_finite() && effective_len_factor > 0.0)
    {
        return invalid("params.window Gaussian effective_len_factor must be finite and positive");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    const FIXTURE_WAV: &[u8] = include_bytes!("../../../tests/fixtures/audio/arctic_bdl_a0001.wav");

    fn sine_wav_bytes(sample_rate: u32, seconds: f64, frequency: f64) -> Vec<u8> {
        let frames = (sample_rate as f64 * seconds).round() as u32;
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut cursor = std::io::Cursor::new(Vec::new());
        {
            let mut writer = hound::WavWriter::new(&mut cursor, spec).unwrap();
            for i in 0..frames {
                let t = i as f64 / sample_rate as f64;
                let sample = (2.0 * PI * frequency * t).sin();
                writer.write_sample((sample * 32_000.0) as i16).unwrap();
            }
            writer.finalize().unwrap();
        }
        cursor.into_inner()
    }

    #[test]
    fn import_then_info_reports_the_decoded_buffer() {
        let mut engine = Engine::new();
        let bytes = sine_wav_bytes(16_000, 0.5, 440.0);
        let id = engine.import_wav_bytes(&bytes).unwrap();
        let info = engine.audio_info(id).unwrap();
        assert_eq!(info.sample_rate, 16_000.0);
        assert_eq!(info.channels, 1);
        assert!((info.duration - 0.5).abs() < 1.0e-9);
    }

    #[test]
    fn unknown_id_is_a_typed_error_everywhere() {
        let engine = Engine::new();
        let bogus = AudioId::from_u64(999);
        assert!(matches!(
            engine.audio_info(bogus),
            Err(EngineError::UnknownAudioId(_))
        ));
        assert!(matches!(
            engine.waveform_slice(bogus, 0.0, 1.0, 8),
            Err(EngineError::UnknownAudioId(_))
        ));
        assert!(matches!(
            engine.spectrogram_tile_rgba(
                bogus,
                &TileRequest {
                    t0: 0.0,
                    t1: 0.1,
                    f0: 0.0,
                    f1: 5000.0,
                    width_px: 4,
                    height_px: 4,
                    params: SpectrogramParams::default(),
                },
                &DisplayMapping::default(),
                Colormap::Viridis,
                Theme::Light,
            ),
            Err(EngineError::UnknownAudioId(_))
        ));
    }

    #[test]
    fn malformed_wav_bytes_are_a_typed_error_not_a_panic() {
        let mut engine = Engine::new();
        assert!(matches!(
            engine.import_wav_bytes(b"not a wav file"),
            Err(EngineError::Audio(_))
        ));
    }

    #[test]
    fn non_finite_tile_request_bounds_are_a_typed_error_not_a_panic() {
        let mut engine = Engine::new();
        let id = engine.import_wav_bytes(FIXTURE_WAV).unwrap();
        let req = TileRequest {
            t0: f64::NAN,
            t1: 0.1,
            f0: 0.0,
            f1: 5000.0,
            width_px: 4,
            height_px: 4,
            params: SpectrogramParams::default(),
        };
        assert!(matches!(
            engine.spectrogram_tile_rgba(
                id,
                &req,
                &DisplayMapping::default(),
                Colormap::Viridis,
                Theme::Light
            ),
            Err(EngineError::InvalidRequest { .. })
        ));
    }

    #[test]
    fn non_finite_waveform_bounds_are_a_typed_error_not_a_panic() {
        let mut engine = Engine::new();
        let id = engine.import_wav_bytes(FIXTURE_WAV).unwrap();
        assert!(matches!(
            engine.waveform_slice(id, f64::NAN, 1.0, 8),
            Err(EngineError::InvalidRequest { .. })
        ));
    }

    #[test]
    fn waveform_pyramid_agrees_with_direct_min_max_on_fixture_audio() {
        let mut engine = Engine::new();
        let id = engine.import_wav_bytes(FIXTURE_WAV).unwrap();
        let audio = Audio::from_wav_bytes(FIXTURE_WAV).unwrap();
        let mono = audio.mono_mix().into_owned();
        let sample_rate = audio.sample_rate();
        let duration = audio.duration();

        let px = 50;
        let t0 = duration * 0.1;
        let t1 = duration * 0.6;
        let slice = engine.waveform_slice(id, t0, t1, px).unwrap();
        assert_eq!(slice.len() as u32, px);

        for (i, bucket) in slice.iter().enumerate() {
            let frac0 = i as f64 / px as f64;
            let frac1 = (i + 1) as f64 / px as f64;
            let start = ((t0 + frac0 * (t1 - t0)) * sample_rate)
                .round()
                .clamp(0.0, mono.len() as f64) as usize;
            let mut end = ((t0 + frac1 * (t1 - t0)) * sample_rate)
                .round()
                .clamp(0.0, mono.len() as f64) as usize;
            end = end.max(start);
            if end == start && start < mono.len() {
                end = start + 1;
            }
            let expected_min = mono[start..end]
                .iter()
                .copied()
                .fold(f32::INFINITY, f32::min);
            let expected_max = mono[start..end]
                .iter()
                .copied()
                .fold(f32::NEG_INFINITY, f32::max);
            assert_eq!(
                bucket.min.to_bits(),
                expected_min.to_bits(),
                "bucket {i} min"
            );
            assert_eq!(
                bucket.max.to_bits(),
                expected_max.to_bits(),
                "bucket {i} max"
            );
        }
    }

    #[test]
    fn waveform_pyramid_agrees_with_direct_min_max_on_synthetic_audio() {
        let mut engine = Engine::new();
        let bytes = sine_wav_bytes(8_000, 1.0, 220.0);
        let id = engine.import_wav_bytes(&bytes).unwrap();
        let audio = Audio::from_wav_bytes(&bytes).unwrap();
        let mono = audio.mono_mix().into_owned();

        let px = 64;
        let slice = engine.waveform_slice(id, 0.0, 1.0, px).unwrap();
        for (i, bucket) in slice.iter().enumerate() {
            let start = (mono.len() * i / px as usize).min(mono.len());
            let end = (mono.len() * (i + 1) / px as usize).min(mono.len());
            if start == end {
                continue;
            }
            let expected_min = mono[start..end]
                .iter()
                .copied()
                .fold(f32::INFINITY, f32::min);
            let expected_max = mono[start..end]
                .iter()
                .copied()
                .fold(f32::NEG_INFINITY, f32::max);
            assert!(bucket.min <= expected_min + f32::EPSILON);
            assert!(bucket.max >= expected_max - f32::EPSILON);
        }
    }

    #[test]
    fn spectrogram_tile_has_expected_dimensions_and_is_deterministic() {
        let mut engine = Engine::new();
        let id = engine.import_wav_bytes(FIXTURE_WAV).unwrap();
        let req = TileRequest {
            t0: 0.05,
            t1: 0.35,
            f0: 0.0,
            f1: 5000.0,
            width_px: 40,
            height_px: 30,
            params: SpectrogramParams::default(),
        };
        let display = DisplayMapping::default();
        let first = engine
            .spectrogram_tile_rgba(id, &req, &display, Colormap::Viridis, Theme::Dark)
            .unwrap();
        let second = engine
            .spectrogram_tile_rgba(id, &req, &display, Colormap::Viridis, Theme::Dark)
            .unwrap();
        assert_eq!(first.len(), 40 * 30 * 4);
        assert_eq!(first, second);
    }

    #[test]
    fn tile_request_too_short_for_a_frame_is_a_typed_error() {
        let mut engine = Engine::new();
        let bytes = sine_wav_bytes(8_000, 0.001, 440.0);
        let id = engine.import_wav_bytes(&bytes).unwrap();
        let req = TileRequest {
            t0: 0.0,
            t1: 0.001,
            f0: 0.0,
            f1: 4000.0,
            width_px: 4,
            height_px: 4,
            params: SpectrogramParams::default(),
        };
        assert!(matches!(
            engine.spectrogram_tile_rgba(
                id,
                &req,
                &DisplayMapping::default(),
                Colormap::Viridis,
                Theme::Light
            ),
            Err(EngineError::InvalidRequest { .. })
        ));
    }
}
