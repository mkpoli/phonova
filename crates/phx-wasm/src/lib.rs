//! wasm-bindgen bindings over phx-engine.
#![warn(missing_docs)]

use js_sys::{Float32Array, Uint8Array};
use phx_engine::{
    AudioId, Colormap as EngineColormap, DisplayMapping, Engine, SpectrogramParams,
    Theme as EngineTheme, TileRequest,
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
    }

    #[wasm_bindgen_test]
    fn unknown_id_rejects_instead_of_panicking() {
        let engine = WasmEngine::new();
        assert!(engine.audio_info(999).is_err());
    }
}
