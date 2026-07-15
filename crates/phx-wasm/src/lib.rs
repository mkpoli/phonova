//! wasm-bindgen bindings over phx-engine.
#![warn(missing_docs)]

use js_sys::{Float32Array, Float64Array, Uint8Array};
use phx_engine::{
    AudioId, Colormap as EngineColormap, DisplayMapping, Engine, FormantParams, IntensityParams,
    PitchParams, SpectrogramParams, Theme as EngineTheme, TileRequest,
};
use wasm_bindgen::prelude::*;

/// Perceptual colormap selection exposed to JavaScript.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WasmColormap {
    /// Perceptually uniform purple→teal→yellow ramp.
    Viridis,
    /// Perceptually uniform black→purple→orange→pale-yellow ramp.
    Magma,
    /// Achromatic ramp, tuned separately per theme.
    Grayscale,
}

impl From<WasmColormap> for EngineColormap {
    fn from(value: WasmColormap) -> Self {
        match value {
            WasmColormap::Viridis => EngineColormap::Viridis,
            WasmColormap::Magma => EngineColormap::Magma,
            WasmColormap::Grayscale => EngineColormap::Grayscale,
        }
    }
}

/// Light/dark UI theme exposed to JavaScript.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WasmTheme {
    /// Light application background.
    Light,
    /// Dark application background.
    Dark,
}

impl From<WasmTheme> for EngineTheme {
    fn from(value: WasmTheme) -> Self {
        match value {
            WasmTheme::Light => EngineTheme::Light,
            WasmTheme::Dark => EngineTheme::Dark,
        }
    }
}

/// Duration, sample rate, channel count, and name of a stored audio buffer.
#[wasm_bindgen]
pub struct WasmAudioInfo {
    duration: f64,
    sample_rate: f64,
    channels: u32,
    name: Option<String>,
}

#[wasm_bindgen]
impl WasmAudioInfo {
    /// Duration in seconds.
    #[wasm_bindgen(getter)]
    pub fn duration(&self) -> f64 {
        self.duration
    }

    /// Sample rate in hertz.
    #[wasm_bindgen(getter, js_name = sampleRate)]
    pub fn sample_rate(&self) -> f64 {
        self.sample_rate
    }

    /// Channel count.
    #[wasm_bindgen(getter)]
    pub fn channels(&self) -> u32 {
        self.channels
    }

    /// Buffer name, when the import source provided one.
    #[wasm_bindgen(getter)]
    pub fn name(&self) -> Option<String> {
        self.name.clone()
    }
}

/// A pitch contour crossing the boundary as two parallel `Float64Array`s.
///
/// `times` holds frame-centre times in seconds; `f0` holds the selected
/// fundamental in hertz, with `NaN` marking unvoiced frames so the overlay
/// can break the line across voiceless runs. `maxHz` is the highest voiced
/// value, for the inspector's ceiling-clipping badge.
#[wasm_bindgen]
pub struct WasmPitchTrack {
    times: Vec<f64>,
    f0: Vec<f64>,
    max_hz: f64,
}

#[wasm_bindgen]
impl WasmPitchTrack {
    /// Frame-centre times in seconds.
    #[wasm_bindgen(getter)]
    pub fn times(&self) -> Float64Array {
        Float64Array::from(self.times.as_slice())
    }

    /// Selected fundamental per frame in hertz; `NaN` on unvoiced frames.
    #[wasm_bindgen(getter)]
    pub fn f0(&self) -> Float64Array {
        Float64Array::from(self.f0.as_slice())
    }

    /// Highest voiced fundamental in hertz, or `0.0` when fully unvoiced.
    #[wasm_bindgen(getter, js_name = maxHz)]
    pub fn max_hz(&self) -> f64 {
        self.max_hz
    }
}

/// Formant candidates crossing the boundary as flat `[time, frequency,
/// bandwidth]` triples in `points`, ascending by frame then by frequency.
///
/// The flat layout suits speckle rendering: each triple is one speckle placed
/// at its time and frequency, sized by its bandwidth. `maxHz` is the highest
/// candidate frequency, for the inspector's ceiling-clipping badge.
#[wasm_bindgen]
pub struct WasmFormantTrack {
    points: Vec<f64>,
    max_hz: f64,
}

#[wasm_bindgen]
impl WasmFormantTrack {
    /// Flat `[time_s, frequency_hz, bandwidth_hz]` triples.
    #[wasm_bindgen(getter)]
    pub fn points(&self) -> Float64Array {
        Float64Array::from(self.points.as_slice())
    }

    /// Highest candidate frequency in hertz, or `0.0` when there are none.
    #[wasm_bindgen(getter, js_name = maxHz)]
    pub fn max_hz(&self) -> f64 {
        self.max_hz
    }
}

/// An intensity contour crossing the boundary as parallel `Float64Array`s:
/// `times` in seconds, `db` in dB SPL.
#[wasm_bindgen]
pub struct WasmIntensityTrack {
    times: Vec<f64>,
    db: Vec<f64>,
}

#[wasm_bindgen]
impl WasmIntensityTrack {
    /// Frame-centre times in seconds.
    #[wasm_bindgen(getter)]
    pub fn times(&self) -> Float64Array {
        Float64Array::from(self.times.as_slice())
    }

    /// Level per frame in dB SPL re 2×10⁻⁵ Pa.
    #[wasm_bindgen(getter)]
    pub fn db(&self) -> Float64Array {
        Float64Array::from(self.db.as_slice())
    }
}

/// Session engine surface exposed to JavaScript.
///
/// Wraps [`phx_engine::Engine`] one-to-one for the walking-skeleton surface:
/// import, audio metadata, waveform pyramid slices, and dB→RGBA spectrogram
/// tiles. An [`AudioId`] crosses the boundary as a plain `u64` (JavaScript
/// `BigInt`) — see [`phx_engine::AudioId::as_u64`] and
/// [`phx_engine::AudioId::from_u64`].
#[wasm_bindgen]
#[derive(Default)]
pub struct WasmEngine {
    inner: Engine,
}

#[wasm_bindgen]
impl WasmEngine {
    /// Creates an engine with an empty audio store.
    #[wasm_bindgen(constructor)]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Decodes RIFF/WAVE bytes and returns the id of the new store entry.
    ///
    /// `bytes` crosses the boundary as a borrowed slice: wasm-bindgen copies
    /// the JS `Uint8Array` once into wasm linear memory for the call, and
    /// decoding reads that copy directly with no further duplication.
    ///
    /// # Errors
    /// Rejects when `bytes` is not a WAV file `phx-audio` can decode.
    #[wasm_bindgen(js_name = importWavBytes)]
    pub fn import_wav_bytes(&mut self, bytes: &[u8]) -> Result<u64, JsError> {
        let id = self.inner.import_wav_bytes(bytes)?;
        Ok(id.as_u64())
    }

    /// Returns duration, sample rate, channel count, and name for `id`.
    ///
    /// # Errors
    /// Rejects when `id` does not name a live store entry.
    #[wasm_bindgen(js_name = audioInfo)]
    pub fn audio_info(&self, id: u64) -> Result<WasmAudioInfo, JsError> {
        let info = self.inner.audio_info(AudioId::from_u64(id))?;
        Ok(WasmAudioInfo {
            duration: info.duration,
            sample_rate: info.sample_rate,
            channels: info.channels as u32,
            name: info.name,
        })
    }

    /// Returns `px` min/max buckets covering `[t0, t1)` seconds of `id`, as
    /// an interleaved `[min0, max0, min1, max1, …]` `Float32Array` of length
    /// `2 * px`.
    ///
    /// Building the typed array from one owned `Vec<f32>` — rather than a
    /// `js_sys::Array` of boxed numbers — copies the buffer exactly once
    /// across the boundary.
    ///
    /// # Errors
    /// Rejects when `id` does not name a live store entry, or when `t0`/`t1`
    /// are not finite.
    #[wasm_bindgen(js_name = waveformSlice)]
    pub fn waveform_slice(
        &self,
        id: u64,
        t0: f64,
        t1: f64,
        px: u32,
    ) -> Result<Float32Array, JsError> {
        let slice = self
            .inner
            .waveform_slice(AudioId::from_u64(id), t0, t1, px)?;
        let mut interleaved = Vec::with_capacity(slice.len() * 2);
        for bucket in slice {
            interleaved.push(bucket.min);
            interleaved.push(bucket.max);
        }
        Ok(Float32Array::from(interleaved.as_slice()))
    }

    /// Computes a spectrogram tile for `id` and colorizes it to RGBA bytes.
    ///
    /// `t0`/`t1`/`f0`/`f1` bound the requested time/frequency window;
    /// `width_px`/`height_px` set the tile size; `window_length`,
    /// `max_frequency`, `time_step`, and `frequency_step` are
    /// [`phx_engine::SpectrogramParams`] fields — the analysis window shape
    /// itself stays Praat's default Gaussian (`SpectrogramParams::default()`
    /// provenance); this walking-skeleton surface does not yet expose the
    /// Hanning/Kaiser alternatives over the JS boundary. `dynamic_range_db`
    /// and `max_db` are [`phx_engine::DisplayMapping`] fields (`max_db =
    /// undefined` autoscales).
    ///
    /// Returns `4 * width_px * height_px` bytes, `R, G, B, A` per pixel, row
    /// 0 first, as a `Uint8Array` built from the owned result buffer in one
    /// copy.
    ///
    /// # Errors
    /// Rejects when `id` does not name a live store entry, when a
    /// time/frequency bound is not finite, when an analysis parameter is
    /// not positive, or when the audio is too short for the requested
    /// window to produce a single analysis frame.
    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(js_name = spectrogramTileRgba)]
    pub fn spectrogram_tile_rgba(
        &self,
        id: u64,
        t0: f64,
        t1: f64,
        f0: f64,
        f1: f64,
        width_px: u32,
        height_px: u32,
        window_length: f64,
        max_frequency: f64,
        time_step: f64,
        frequency_step: f64,
        dynamic_range_db: f64,
        max_db: Option<f64>,
        colormap: WasmColormap,
        theme: WasmTheme,
    ) -> Result<Uint8Array, JsError> {
        let req = TileRequest {
            t0,
            t1,
            f0,
            f1,
            width_px,
            height_px,
            params: SpectrogramParams {
                window_length,
                max_frequency,
                time_step,
                frequency_step,
                ..SpectrogramParams::default()
            },
        };
        let display = DisplayMapping {
            dynamic_range_db,
            max_db,
        };
        let rgba = self.inner.spectrogram_tile_rgba(
            AudioId::from_u64(id),
            &req,
            &display,
            colormap.into(),
            theme.into(),
        )?;
        Ok(Uint8Array::from(rgba.as_slice()))
    }

    /// Computes the pitch contour of `id` over its whole signal.
    ///
    /// `floor_hz`/`ceiling_hz` are the two [`phx_engine::PitchParams`] fields
    /// the inspector edits; the remaining fields keep their documented Praat
    /// defaults. The whole-signal frame grid keeps a value queried at a given
    /// time independent of the viewport.
    ///
    /// # Errors
    /// Rejects when `id` does not name a live store entry.
    #[wasm_bindgen(js_name = pitchTrack)]
    pub fn pitch_track(
        &self,
        id: u64,
        floor_hz: f64,
        ceiling_hz: f64,
    ) -> Result<WasmPitchTrack, JsError> {
        let params = PitchParams {
            floor_hz,
            ceiling_hz,
            ..PitchParams::default()
        };
        let track = self.inner.pitch_track(AudioId::from_u64(id), &params)?;
        let mut times = Vec::with_capacity(track.frames().len());
        let mut f0 = Vec::with_capacity(track.frames().len());
        let mut max_hz = 0.0_f64;
        for frame in track.frames() {
            times.push(frame.time);
            match frame.f0 {
                Some(hz) => {
                    max_hz = max_hz.max(hz);
                    f0.push(hz);
                }
                None => f0.push(f64::NAN),
            }
        }
        Ok(WasmPitchTrack { times, f0, max_hz })
    }

    /// Computes a fast pitch preview over just the samples spanning `[t0, t1)`
    /// seconds, with frame times already placed on the absolute timeline.
    ///
    /// A live ceiling edit renders this first, then swaps in the whole-signal
    /// [`WasmEngine::pitch_track`] result. `max_hz` here covers only the
    /// window, so the inspector's clipping badge stays driven by the full
    /// track rather than the preview.
    ///
    /// # Errors
    /// Rejects when `id` does not name a live store entry, or when `t0`/`t1`
    /// are not finite.
    #[wasm_bindgen(js_name = pitchTrackSpan)]
    pub fn pitch_track_span(
        &self,
        id: u64,
        floor_hz: f64,
        ceiling_hz: f64,
        t0: f64,
        t1: f64,
    ) -> Result<WasmPitchTrack, JsError> {
        let params = PitchParams {
            floor_hz,
            ceiling_hz,
            ..PitchParams::default()
        };
        let (track, start_time) =
            self.inner
                .pitch_track_span(AudioId::from_u64(id), &params, t0, t1)?;
        let mut times = Vec::with_capacity(track.frames().len());
        let mut f0 = Vec::with_capacity(track.frames().len());
        let mut max_hz = 0.0_f64;
        for frame in track.frames() {
            times.push(start_time + frame.time);
            match frame.f0 {
                Some(hz) => {
                    max_hz = max_hz.max(hz);
                    f0.push(hz);
                }
                None => f0.push(f64::NAN),
            }
        }
        Ok(WasmPitchTrack { times, f0, max_hz })
    }

    /// Computes the formant candidates of `id` over its whole signal.
    ///
    /// `ceiling_hz`/`max_formants` are the two [`phx_engine::FormantParams`]
    /// fields the inspector edits; the remaining fields keep their documented
    /// Praat defaults. When `smoothed` is set the Xia–Espy-Wilson tracker
    /// runs — a view the UI marks provisional while the tracking weights are
    /// unvalidated (`docs/plan/tasks/phase-4.md`); otherwise the raw Burg
    /// candidates are returned.
    ///
    /// # Errors
    /// Rejects when `id` does not name a live store entry, or when a formant
    /// parameter is outside the range the analysis accepts.
    #[wasm_bindgen(js_name = formantTrack)]
    pub fn formant_track(
        &self,
        id: u64,
        ceiling_hz: f64,
        max_formants: usize,
        smoothed: bool,
    ) -> Result<WasmFormantTrack, JsError> {
        let params = FormantParams {
            ceiling_hz,
            max_formants,
            ..FormantParams::default()
        };
        let audio_id = AudioId::from_u64(id);
        let track = if smoothed {
            self.inner.formant_track_smoothed(audio_id, &params)?
        } else {
            self.inner.formant_track(audio_id, &params)?
        };
        let mut points = Vec::new();
        let mut max_hz = 0.0_f64;
        for frame in &track.frames {
            for formant in &frame.formants {
                points.push(frame.time);
                points.push(formant.frequency);
                points.push(formant.bandwidth);
                max_hz = max_hz.max(formant.frequency);
            }
        }
        Ok(WasmFormantTrack { points, max_hz })
    }

    /// Computes the intensity contour of `id` over its whole signal.
    ///
    /// `pitch_floor_hz` is the [`phx_engine::IntensityParams`] field the
    /// inspector edits; it sets the analysis window's effective duration and,
    /// with the automatic step, the frame hop. Remaining fields keep their
    /// documented Praat defaults.
    ///
    /// # Errors
    /// Rejects when `id` does not name a live store entry, or when
    /// `pitch_floor_hz` is not finite and positive.
    #[wasm_bindgen(js_name = intensityTrack)]
    pub fn intensity_track(
        &self,
        id: u64,
        pitch_floor_hz: f64,
    ) -> Result<WasmIntensityTrack, JsError> {
        let params = IntensityParams {
            pitch_floor_hz,
            ..IntensityParams::default()
        };
        let track = self.inner.intensity_track(AudioId::from_u64(id), &params)?;
        let (times, db) = track.iter().unzip();
        Ok(WasmIntensityTrack { times, db })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::wasm_bindgen_test;

    const FIXTURE_WAV: &[u8] = include_bytes!("../../../tests/fixtures/audio/arctic_bdl_a0001.wav");

    #[wasm_bindgen_test]
    fn import_then_info_then_tile_round_trip() {
        let mut engine = WasmEngine::new();
        let id = engine.import_wav_bytes(FIXTURE_WAV).unwrap();
        let info = engine.audio_info(id).unwrap();
        assert!(info.duration() > 0.0);
        assert!(info.sample_rate() > 0.0);
        assert_eq!(info.channels(), 1);

        let waveform = engine.waveform_slice(id, 0.0, info.duration(), 32).unwrap();
        assert_eq!(waveform.length(), 64);

        let default_params = SpectrogramParams::default();
        let rgba = engine
            .spectrogram_tile_rgba(
                id,
                0.05,
                0.35,
                0.0,
                5000.0,
                16,
                12,
                default_params.window_length,
                default_params.max_frequency,
                default_params.time_step,
                default_params.frequency_step,
                50.0,
                None,
                WasmColormap::Viridis,
                WasmTheme::Dark,
            )
            .unwrap();
        assert_eq!(rgba.length(), 16 * 12 * 4);

        let pitch = engine.pitch_track(id, 75.0, 600.0).unwrap();
        assert_eq!(pitch.times().length(), pitch.f0().length());
        assert!(pitch.times().length() > 0);
        assert!(pitch.max_hz() > 0.0);

        let formants = engine.formant_track(id, 5000.0, 5, false).unwrap();
        assert_eq!(formants.points().length() % 3, 0);
        assert!(formants.max_hz() > 0.0);
        // Smoothing keeps the same triple layout.
        let smoothed = engine.formant_track(id, 5000.0, 5, true).unwrap();
        assert_eq!(smoothed.points().length() % 3, 0);

        let intensity = engine.intensity_track(id, 100.0).unwrap();
        assert_eq!(intensity.times().length(), intensity.db().length());
        assert!(intensity.times().length() > 0);
    }

    #[wasm_bindgen_test]
    fn unknown_id_rejects_instead_of_panicking() {
        let engine = WasmEngine::new();
        assert!(engine.audio_info(999).is_err());
    }
}
