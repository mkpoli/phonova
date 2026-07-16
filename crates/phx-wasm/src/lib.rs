//! wasm-bindgen bindings over phx-engine.
#![warn(missing_docs)]

use js_sys::{Float32Array, Float64Array, Uint8Array};
use phx_engine::{
    AlignMode, Annotation, AnnotationId, Applied, AudioId, BoundaryId, Colormap as EngineColormap,
    Command, DisplayMapping, Engine, ExportBundle, Figure, FigureFormat, FigureRequest,
    FormantParams, IntensityParams, IntervalId, LabelPattern, LabelQuery, LabelTarget, PitchParams,
    PointId, RecordingId, SpectrogramParams, Theme as EngineTheme, Tier, TierId, TierRelation,
    TileRequest, export_figure as engine_export_figure, figure_to_svg,
};
use phx_project::{ContentHash, MediaId, MediaRef, Project};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use wasm_bindgen::prelude::*;

/// Perceptual colormap selection exposed to JavaScript.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WasmColormap {
    /// Perceptually uniform purple→teal→yellow ramp.
    Viridis,
    /// Perceptually uniform black→purple→orange→pale-yellow ramp.
    Magma,
    /// Perceptually uniform black→purple→orange→pale-yellow ramp.
    Inferno,
    /// Perceptually uniform dark-blue→purple→orange→yellow ramp.
    Plasma,
    /// Perceptually uniform dark-blue→gray→yellow ramp for color-vision
    /// deficiency.
    Cividis,
    /// Achromatic ramp, tuned separately per theme.
    Grayscale,
}

impl From<WasmColormap> for EngineColormap {
    fn from(value: WasmColormap) -> Self {
        match value {
            WasmColormap::Viridis => EngineColormap::Viridis,
            WasmColormap::Magma => EngineColormap::Magma,
            WasmColormap::Inferno => EngineColormap::Inferno,
            WasmColormap::Plasma => EngineColormap::Plasma,
            WasmColormap::Cividis => EngineColormap::Cividis,
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

/// Cross-tier integrity relation selector exposed to JavaScript.
///
/// `AlignedBoundaries` and `ChildOf` name a second tier in the same document;
/// that tier's id is passed alongside as `relationTier`. `Independent` ignores
/// `relationTier`.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WasmTierRelation {
    /// No cross-tier integrity relation.
    Independent,
    /// Boundaries stay aligned with the tier named by `relationTier`.
    AlignedBoundaries,
    /// Every interval nests inside a span of the parent tier `relationTier`.
    ChildOf,
}

/// Builds an engine [`TierRelation`] from the selector and the optional target
/// tier id, rejecting a missing target for the two relating variants.
fn tier_relation_of(
    relation: WasmTierRelation,
    relation_tier: Option<u64>,
) -> Result<TierRelation, JsError> {
    let target = || {
        relation_tier
            .map(TierId::new)
            .ok_or_else(|| JsError::new("relationTier is required for this relation"))
    };
    match relation {
        WasmTierRelation::Independent => Ok(TierRelation::Independent),
        WasmTierRelation::AlignedBoundaries => {
            Ok(TierRelation::AlignedBoundaries { with: target()? })
        }
        WasmTierRelation::ChildOf => Ok(TierRelation::ChildOf { parent: target()? }),
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

/// A finished recording crossing the boundary: the new store id and the take
/// encoded as WAV bytes for the host to persist.
#[wasm_bindgen]
pub struct WasmFinishedRecording {
    audio: u64,
    wav: Vec<u8>,
}

#[wasm_bindgen]
impl WasmFinishedRecording {
    /// Id of the audio buffer the take became in the store.
    #[wasm_bindgen(getter)]
    pub fn audio(&self) -> u64 {
        self.audio
    }

    /// The take as RIFF/WAVE bytes (24-bit PCM), copied once across the boundary.
    #[wasm_bindgen(getter)]
    pub fn wav(&self) -> Uint8Array {
        Uint8Array::from(self.wav.as_slice())
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

/// What a successful command, undo, or redo changed, flattened for JavaScript.
///
/// `kind` names the effect (`"boundaryInserted"`, `"labelSet"`, …); the id
/// getters return the affected ids that variant carries, or `undefined` when it
/// carries none. A frontend switches on `kind` and reads the ids it needs to
/// patch its view.
#[wasm_bindgen]
pub struct WasmApplied {
    kind: String,
    annotation: Option<u64>,
    audio: Option<u64>,
    tier: Option<u64>,
    boundary: Option<u64>,
    point: Option<u64>,
}

#[wasm_bindgen]
impl WasmApplied {
    /// Effect name, matching the engine's `Applied` variant in lowerCamelCase.
    #[wasm_bindgen(getter)]
    pub fn kind(&self) -> String {
        self.kind.clone()
    }

    /// Affected annotation id, when the effect names one.
    #[wasm_bindgen(getter)]
    pub fn annotation(&self) -> Option<u64> {
        self.annotation
    }

    /// Affected audio id, when the effect names one.
    #[wasm_bindgen(getter)]
    pub fn audio(&self) -> Option<u64> {
        self.audio
    }

    /// Affected tier id, when the effect names one.
    #[wasm_bindgen(getter)]
    pub fn tier(&self) -> Option<u64> {
        self.tier
    }

    /// Affected boundary id, when the effect names one.
    #[wasm_bindgen(getter)]
    pub fn boundary(&self) -> Option<u64> {
        self.boundary
    }

    /// Affected point id, when the effect names one.
    #[wasm_bindgen(getter)]
    pub fn point(&self) -> Option<u64> {
        self.point
    }
}

impl From<Applied> for WasmApplied {
    fn from(applied: Applied) -> Self {
        let (kind, annotation, audio, tier, boundary, point) = match applied {
            Applied::AudioImported { audio } => {
                ("audioImported", None, Some(audio), None, None, None)
            }
            Applied::AudioRemoved { audio } => {
                ("audioRemoved", None, Some(audio), None, None, None)
            }
            Applied::AnnotationAttached { annotation, audio } => (
                "annotationAttached",
                Some(annotation),
                Some(audio),
                None,
                None,
                None,
            ),
            Applied::AnnotationDetached { annotation } => (
                "annotationDetached",
                Some(annotation),
                None,
                None,
                None,
                None,
            ),
            Applied::TierAdded { annotation, tier } => {
                ("tierAdded", Some(annotation), None, Some(tier), None, None)
            }
            Applied::TierRemoved { annotation, tier } => (
                "tierRemoved",
                Some(annotation),
                None,
                Some(tier),
                None,
                None,
            ),
            Applied::TierReordered {
                annotation, tier, ..
            } => (
                "tierReordered",
                Some(annotation),
                None,
                Some(tier),
                None,
                None,
            ),
            Applied::BoundaryInserted {
                annotation,
                tier,
                boundary,
                ..
            } => (
                "boundaryInserted",
                Some(annotation),
                None,
                Some(tier),
                Some(boundary),
                None,
            ),
            Applied::BoundaryMoved { annotation, .. } => {
                ("boundaryMoved", Some(annotation), None, None, None, None)
            }
            Applied::BoundaryRemoved {
                annotation,
                boundary,
            } => (
                "boundaryRemoved",
                Some(annotation),
                None,
                None,
                Some(boundary),
                None,
            ),
            Applied::BoundaryRestored { annotation, .. } => {
                ("boundaryRestored", Some(annotation), None, None, None, None)
            }
            Applied::LabelSet { annotation, .. } => {
                ("labelSet", Some(annotation), None, None, None, None)
            }
            Applied::PointInserted {
                annotation,
                tier,
                point,
                ..
            } => (
                "pointInserted",
                Some(annotation),
                None,
                Some(tier),
                None,
                Some(point),
            ),
            Applied::PointMoved {
                annotation, point, ..
            } => (
                "pointMoved",
                Some(annotation),
                None,
                None,
                None,
                Some(point),
            ),
            Applied::PointRemoved { annotation, point } => (
                "pointRemoved",
                Some(annotation),
                None,
                None,
                None,
                Some(point),
            ),
        };
        Self {
            kind: kind.to_string(),
            annotation: annotation.map(IdExt::id),
            audio: audio.map(IdExt::id),
            tier: tier.map(IdExt::id),
            boundary: boundary.map(IdExt::id),
            point: point.map(IdExt::id),
        }
    }
}

/// Uniform numeric-handle accessor over the engine's several id newtypes.
trait IdExt {
    fn id(self) -> u64;
}

impl IdExt for AnnotationId {
    fn id(self) -> u64 {
        self.as_u64()
    }
}

impl IdExt for AudioId {
    fn id(self) -> u64 {
        self.as_u64()
    }
}

impl IdExt for TierId {
    fn id(self) -> u64 {
        self.get()
    }
}

impl IdExt for BoundaryId {
    fn id(self) -> u64 {
        self.get()
    }
}

impl IdExt for PointId {
    fn id(self) -> u64 {
        self.get()
    }
}

/// A document's tiers as parallel arrays, in document order.
#[wasm_bindgen]
pub struct WasmTierList {
    ids: Vec<u64>,
    names: Vec<String>,
    kinds: Vec<u8>,
}

#[wasm_bindgen]
impl WasmTierList {
    /// Stable tier ids.
    #[wasm_bindgen(getter)]
    pub fn ids(&self) -> Vec<u64> {
        self.ids.clone()
    }

    /// Tier display names, index-aligned with `ids`.
    #[wasm_bindgen(getter)]
    pub fn names(&self) -> Vec<String> {
        self.names.clone()
    }

    /// Per-tier kind flag: `0` interval, `1` point.
    #[wasm_bindgen(getter)]
    pub fn kinds(&self) -> Uint8Array {
        Uint8Array::from(self.kinds.as_slice())
    }
}

/// Intervals of one tier as parallel arrays for rendering.
#[wasm_bindgen]
#[derive(Default)]
pub struct WasmIntervals {
    ids: Vec<u64>,
    start_boundaries: Vec<u64>,
    end_boundaries: Vec<u64>,
    xmin: Vec<f64>,
    xmax: Vec<f64>,
    labels: Vec<String>,
}

#[wasm_bindgen]
impl WasmIntervals {
    /// Stable interval ids.
    #[wasm_bindgen(getter)]
    pub fn ids(&self) -> Vec<u64> {
        self.ids.clone()
    }

    /// Start-boundary id per interval.
    #[wasm_bindgen(getter, js_name = startBoundaries)]
    pub fn start_boundaries(&self) -> Vec<u64> {
        self.start_boundaries.clone()
    }

    /// End-boundary id per interval.
    #[wasm_bindgen(getter, js_name = endBoundaries)]
    pub fn end_boundaries(&self) -> Vec<u64> {
        self.end_boundaries.clone()
    }

    /// Start time in seconds per interval.
    #[wasm_bindgen(getter)]
    pub fn xmin(&self) -> Float64Array {
        Float64Array::from(self.xmin.as_slice())
    }

    /// End time in seconds per interval.
    #[wasm_bindgen(getter)]
    pub fn xmax(&self) -> Float64Array {
        Float64Array::from(self.xmax.as_slice())
    }

    /// Label text per interval, index-aligned with `ids`.
    #[wasm_bindgen(getter)]
    pub fn labels(&self) -> Vec<String> {
        self.labels.clone()
    }
}

/// Points of one tier as parallel arrays for rendering.
#[wasm_bindgen]
#[derive(Default)]
pub struct WasmPoints {
    ids: Vec<u64>,
    times: Vec<f64>,
    labels: Vec<String>,
}

#[wasm_bindgen]
impl WasmPoints {
    /// Stable point ids.
    #[wasm_bindgen(getter)]
    pub fn ids(&self) -> Vec<u64> {
        self.ids.clone()
    }

    /// Point time in seconds, index-aligned with `ids`.
    #[wasm_bindgen(getter)]
    pub fn times(&self) -> Float64Array {
        Float64Array::from(self.times.as_slice())
    }

    /// Label text per point, index-aligned with `ids`.
    #[wasm_bindgen(getter)]
    pub fn labels(&self) -> Vec<String> {
        self.labels.clone()
    }
}

/// Cross-document label search hits as parallel arrays.
#[wasm_bindgen]
#[derive(Default)]
pub struct WasmHits {
    annotations: Vec<u64>,
    tiers: Vec<u64>,
    kinds: Vec<u8>,
    targets: Vec<u64>,
    starts: Vec<u32>,
    ends: Vec<u32>,
}

#[wasm_bindgen]
impl WasmHits {
    /// Document id per hit.
    #[wasm_bindgen(getter)]
    pub fn annotations(&self) -> Vec<u64> {
        self.annotations.clone()
    }

    /// Tier id per hit.
    #[wasm_bindgen(getter)]
    pub fn tiers(&self) -> Vec<u64> {
        self.tiers.clone()
    }

    /// Target kind per hit: `0` interval, `1` point.
    #[wasm_bindgen(getter)]
    pub fn kinds(&self) -> Uint8Array {
        Uint8Array::from(self.kinds.as_slice())
    }

    /// Interval or point id per hit, per `kinds`.
    #[wasm_bindgen(getter)]
    pub fn targets(&self) -> Vec<u64> {
        self.targets.clone()
    }

    /// Inclusive match start byte offset per hit.
    #[wasm_bindgen(getter)]
    pub fn starts(&self) -> Vec<u32> {
        self.starts.clone()
    }

    /// Exclusive match end byte offset per hit.
    #[wasm_bindgen(getter)]
    pub fn ends(&self) -> Vec<u32> {
        self.ends.clone()
    }
}

/// Session engine surface exposed to JavaScript.
///
/// Wraps [`phx_engine::Engine`]: import, audio metadata, waveform pyramid
/// slices, dB→RGBA spectrogram tiles, the analysis tracks, and the journaled
/// annotation surface — command application, undo/redo, tier and interval
/// reads, and cross-document label search. Ids cross the boundary as plain
/// `u64` (JavaScript `BigInt`).
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

    /// Opens a streaming recording at `sampleRate` hertz with `channels`
    /// channels and returns its id.
    ///
    /// The host reads `sampleRate` from the capture device (never assumes one)
    /// and feeds sample chunks through [`WasmEngine::append_samples`]. The take
    /// stays out of the audio store until [`WasmEngine::finish_recording`].
    ///
    /// # Errors
    /// Rejects when `sampleRate` is not finite and positive, or when `channels`
    /// is zero.
    #[wasm_bindgen(js_name = beginRecording)]
    pub fn begin_recording(&mut self, sample_rate: f64, channels: usize) -> Result<u64, JsError> {
        let id = self.inner.begin_recording(sample_rate, channels)?;
        Ok(id.as_u64())
    }

    /// Appends one planar sample chunk to an open recording.
    ///
    /// `samples` crosses the boundary as a borrowed `Float32Array`, copied once
    /// into wasm memory for the call. It holds every channel's samples for this
    /// chunk back to back, so its length must divide evenly by the channel
    /// count the take opened with.
    ///
    /// # Errors
    /// Rejects when `recordingId` names no open take, or when `samples` does
    /// not divide evenly by the channel count.
    #[wasm_bindgen(js_name = appendSamples)]
    pub fn append_samples(&mut self, recording_id: u64, samples: &[f32]) -> Result<(), JsError> {
        self.inner
            .append_samples(RecordingId::from_u64(recording_id), samples)?;
        Ok(())
    }

    /// Finishes a recording, materializing it as a store entry and returning
    /// the new audio id together with the take as WAV bytes.
    ///
    /// The audio id names the same kind of buffer an import produces; the WAV
    /// bytes (24-bit PCM) let the host persist the take beside imported media.
    ///
    /// # Errors
    /// Rejects when `recordingId` names no open take, or when the accumulated
    /// samples cannot form an audio buffer (an empty take).
    #[wasm_bindgen(js_name = finishRecording)]
    pub fn finish_recording(
        &mut self,
        recording_id: u64,
        name: String,
    ) -> Result<WasmFinishedRecording, JsError> {
        let finished = self
            .inner
            .finish_recording(RecordingId::from_u64(recording_id), name)?;
        Ok(WasmFinishedRecording {
            audio: finished.audio.as_u64(),
            wav: finished.wav,
        })
    }

    /// Discards an open recording without materializing it.
    ///
    /// # Errors
    /// Rejects when `recordingId` names no open take.
    #[wasm_bindgen(js_name = abortRecording)]
    pub fn abort_recording(&mut self, recording_id: u64) -> Result<(), JsError> {
        self.inner
            .abort_recording(RecordingId::from_u64(recording_id))?;
        Ok(())
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

    /// Returns the mean raw band energy inside a time–frequency box, in decibels.
    ///
    /// `t0`/`t1`/`f0`/`f1` bound the spectrogram selection, each pair accepted in
    /// either order. This is the direct engine query a selection readout equals
    /// at the same coordinates (the batch-equals-GUI invariant).
    ///
    /// # Errors
    /// Rejects when `id` names no live buffer, or when a bound is not finite.
    #[wasm_bindgen(js_name = bandEnergy)]
    pub fn band_energy(&self, id: u64, t0: f64, t1: f64, f0: f64, f1: f64) -> Result<f64, JsError> {
        Ok(self
            .inner
            .band_energy(AudioId::from_u64(id), t0, t1, f0, f1)?)
    }

    /// Computes the selection measurement readout as a JSON string.
    ///
    /// The object carries the box geometry and the span statistics the readout
    /// bar shows: `f0MeanHz`/`f0MinHz`/`f0MaxHz`, `bandEnergyDb`,
    /// `intensityMeanDb`, and `hnrMeanDb`, with absent values serialized as
    /// `null`. `bandEnergyDb` equals [`WasmEngine::band_energy`] for the same
    /// box, so the bar never recomputes a number the engine already answers.
    ///
    /// # Errors
    /// Rejects when `id` names no live buffer, or when a bound is not finite.
    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(js_name = selectionReadout)]
    pub fn selection_readout(
        &self,
        id: u64,
        t0: f64,
        t1: f64,
        f0: f64,
        f1: f64,
        pitch_floor_hz: f64,
        pitch_ceiling_hz: f64,
        intensity_floor_hz: f64,
    ) -> Result<String, JsError> {
        let readout = self.inner.selection_readout(
            AudioId::from_u64(id),
            t0,
            t1,
            f0,
            f1,
            pitch_floor_hz,
            pitch_ceiling_hz,
            intensity_floor_hz,
        )?;
        let value = serde_json::json!({
            "t0": readout.t0,
            "t1": readout.t1,
            "f0": readout.f0,
            "f1": readout.f1,
            "duration": readout.duration,
            "f0MeanHz": readout.f0_mean_hz,
            "f0MinHz": readout.f0_min_hz,
            "f0MaxHz": readout.f0_max_hz,
            "bandEnergyDb": readout.band_energy_db,
            "intensityMeanDb": readout.intensity_mean_db,
            "hnrMeanDb": readout.hnr_mean_db,
        });
        serde_json::to_string(&value).map_err(|err| JsError::new(&err.to_string()))
    }

    /// Returns the mean of each formant slot over a time span as a
    /// `Float64Array`, with `NaN` for a slot no frame in the span carries.
    ///
    /// Slot `j` is the `j`-th lowest candidate per frame. When `smoothed` is set
    /// the Xia–Espy-Wilson tracker runs first — the provisional tracked view the
    /// readout marks while the tracking weights stay unvalidated.
    ///
    /// # Errors
    /// Rejects when `id` names no live buffer, when a formant parameter is out of
    /// range, or when a bound is not finite.
    #[wasm_bindgen(js_name = formantSpanMeans)]
    pub fn formant_span_means(
        &self,
        id: u64,
        ceiling_hz: f64,
        max_formants: usize,
        smoothed: bool,
        t0: f64,
        t1: f64,
    ) -> Result<Float64Array, JsError> {
        let params = FormantParams {
            ceiling_hz,
            max_formants,
            ..FormantParams::default()
        };
        let means =
            self.inner
                .formant_span_means(AudioId::from_u64(id), &params, smoothed, t0, t1)?;
        let flat: Vec<f64> = means
            .into_iter()
            .map(|value| value.unwrap_or(f64::NAN))
            .collect();
        Ok(Float64Array::from(flat.as_slice()))
    }

    /// Computes the aggregate voice report over a selection span as a JSON string.
    ///
    /// The object carries the pitch summary, jitter and shimmer families, mean
    /// HNR, CPP and CPPS, voice breaks, spectral moments at the span midpoint,
    /// the pulse count, and the analysis parameters used — everything the voice
    /// report card renders. Absent measures serialize as `null`.
    ///
    /// # Errors
    /// Rejects when `id` names no live buffer, or when a bound is not finite.
    #[wasm_bindgen(js_name = voiceReport)]
    pub fn voice_report(
        &self,
        id: u64,
        t0: f64,
        t1: f64,
        pitch_floor_hz: f64,
        pitch_ceiling_hz: f64,
    ) -> Result<String, JsError> {
        let audio_id = AudioId::from_u64(id);
        let report = self
            .inner
            .voice_report(audio_id, t0, t1, pitch_floor_hz, pitch_ceiling_hz)?;
        let moments = self.inner.spectral_moments_in_span(audio_id, t0, t1, 2.0)?;
        let value = serde_json::json!({
            "t0": report.span.start,
            "t1": report.span.end,
            "pitch": {
                "meanHz": report.pitch.mean_hz,
                "medianHz": report.pitch.median_hz,
                "minHz": report.pitch.min_hz,
                "maxHz": report.pitch.max_hz,
            },
            "jitter": {
                "local": report.jitter.local,
                "localAbsolute": report.jitter.local_absolute,
                "rap": report.jitter.rap,
                "ppq5": report.jitter.ppq5,
                "ddp": report.jitter.ddp,
            },
            "shimmer": {
                "local": report.shimmer.local,
                "localDb": report.shimmer.local_db,
                "apq3": report.shimmer.apq3,
                "apq5": report.shimmer.apq5,
                "apq11": report.shimmer.apq11,
                "dda": report.shimmer.dda,
            },
            "meanHnrDb": report.mean_hnr_db,
            "cppDb": report.cpp_db,
            "cppsDb": report.cpps_db,
            "voiceBreaks": {
                "thresholdSeconds": report.voice_breaks.threshold_seconds,
                "totalSeconds": report.voice_breaks.total_seconds,
                "count": report.voice_breaks.gaps.len(),
            },
            "moments": {
                "centreOfGravityHz": moments.centre_of_gravity_hz,
                "standardDeviationHz": moments.standard_deviation_hz,
                "skewness": moments.skewness,
                "kurtosis": moments.kurtosis,
            },
            "pulseCount": report.pulses.times().len(),
            "params": {
                "pitchFloorHz": report.pitch_params.floor_hz,
                "pitchCeilingHz": report.pitch_params.ceiling_hz,
                "harmonicityFloorHz": report.harmonicity_params.floor_hz,
                "periodsPerWindow": report.harmonicity_params.periods_per_window,
                "cppFrameLengthSeconds": report.cpp_params.frame_length_seconds,
                "cppMinF0Hz": report.cpp_params.min_f0_hz,
                "cppMaxF0Hz": report.cpp_params.max_f0_hz,
            },
        });
        serde_json::to_string(&value).map_err(|err| JsError::new(&err.to_string()))
    }

    /// Creates an empty annotation over `[xmin, xmax]` seconds and attaches it
    /// to `audioId`, returning the new annotation id.
    ///
    /// The attachment is journaled; undo detaches it. Later tier and boundary
    /// edits target the returned id.
    ///
    /// # Errors
    /// Rejects when `audioId` names no live buffer, or when the time domain is
    /// empty or non-finite.
    #[wasm_bindgen(js_name = createAnnotation)]
    pub fn create_annotation(
        &mut self,
        audio_id: u64,
        xmin: f64,
        xmax: f64,
    ) -> Result<u64, JsError> {
        let annotation = Annotation::new(xmin, xmax)?;
        let applied = self.inner.apply(Command::AttachAnnotation {
            audio: AudioId::from_u64(audio_id),
            annotation,
        })?;
        match applied {
            Applied::AnnotationAttached { annotation, .. } => Ok(annotation.as_u64()),
            _ => Err(JsError::new("attach did not report an annotation id")),
        }
    }

    /// Adds an interval tier holding one unlabeled interval over the whole
    /// domain, returning the new tier id.
    ///
    /// `relation` selects the tier's cross-tier integrity relation;
    /// `relationTier` names the related tier for `AlignedBoundaries` and
    /// `ChildOf` and is ignored for `Independent`. An aligned tier's boundaries
    /// move together with its peer through a single journaled command.
    ///
    /// # Errors
    /// Rejects when `annotationId` names no live document, when `relationTier`
    /// is missing for a relating variant, or when the relation leaves the
    /// document invalid (an unknown or wrong-kind target).
    #[wasm_bindgen(js_name = addIntervalTier)]
    pub fn add_interval_tier(
        &mut self,
        annotation_id: u64,
        name: String,
        relation: WasmTierRelation,
        relation_tier: Option<u64>,
    ) -> Result<u64, JsError> {
        let relation = tier_relation_of(relation, relation_tier)?;
        let applied = self.inner.apply(Command::AddIntervalTier {
            annotation: AnnotationId::from_u64(annotation_id),
            name,
            relation,
        })?;
        tier_id_of(applied)
    }

    /// Adds an empty independent point tier, returning the new tier id.
    ///
    /// # Errors
    /// Rejects when `annotationId` names no live document.
    #[wasm_bindgen(js_name = addPointTier)]
    pub fn add_point_tier(&mut self, annotation_id: u64, name: String) -> Result<u64, JsError> {
        let applied = self.inner.apply(Command::AddPointTier {
            annotation: AnnotationId::from_u64(annotation_id),
            name,
            points: Vec::new(),
            relation: TierRelation::Independent,
        })?;
        tier_id_of(applied)
    }

    /// Removes a tier and everything it holds.
    ///
    /// # Errors
    /// Rejects when the document or tier is unknown, or when removing the tier
    /// would leave a dangling relation on another tier.
    #[wasm_bindgen(js_name = removeTier)]
    pub fn remove_tier(
        &mut self,
        annotation_id: u64,
        tier_id: u64,
    ) -> Result<WasmApplied, JsError> {
        let applied = self.inner.apply(Command::RemoveTier {
            annotation: AnnotationId::from_u64(annotation_id),
            tier: TierId::new(tier_id),
        })?;
        Ok(applied.into())
    }

    /// Splits an interval by inserting a boundary at `at` seconds, returning the
    /// new boundary id.
    ///
    /// # Errors
    /// Rejects when the document or tier is unknown, or when `at` lies on an
    /// existing boundary or outside every interval.
    #[wasm_bindgen(js_name = insertBoundary)]
    pub fn insert_boundary(
        &mut self,
        annotation_id: u64,
        tier_id: u64,
        at: f64,
    ) -> Result<u64, JsError> {
        let applied = self.inner.apply(Command::InsertBoundary {
            annotation: AnnotationId::from_u64(annotation_id),
            tier: TierId::new(tier_id),
            at,
        })?;
        match applied {
            Applied::BoundaryInserted { boundary, .. } => Ok(boundary.get()),
            _ => Err(JsError::new("insert did not report a boundary id")),
        }
    }

    /// Moves a boundary to `to` seconds. When `linked` is set, aligned peer
    /// boundaries move together; otherwise the move is rejected if the boundary
    /// belongs to an aligned component.
    ///
    /// # Errors
    /// Rejects when the document or boundary is unknown, when the move would
    /// collapse an interval, or when an aligned boundary is moved unlinked.
    #[wasm_bindgen(js_name = moveBoundary)]
    pub fn move_boundary(
        &mut self,
        annotation_id: u64,
        boundary_id: u64,
        to: f64,
        linked: bool,
    ) -> Result<WasmApplied, JsError> {
        let mode = if linked {
            AlignMode::Linked
        } else {
            AlignMode::SingleTier
        };
        let applied = self.inner.apply(Command::MoveBoundary {
            annotation: AnnotationId::from_u64(annotation_id),
            boundary: BoundaryId::new(boundary_id),
            to,
            mode,
        })?;
        Ok(applied.into())
    }

    /// Removes an interior boundary, merging its two intervals.
    ///
    /// # Errors
    /// Rejects when the document or boundary is unknown, or when the boundary is
    /// a domain edge.
    #[wasm_bindgen(js_name = removeBoundary)]
    pub fn remove_boundary(
        &mut self,
        annotation_id: u64,
        boundary_id: u64,
    ) -> Result<WasmApplied, JsError> {
        let applied = self.inner.apply(Command::RemoveBoundary {
            annotation: AnnotationId::from_u64(annotation_id),
            boundary: BoundaryId::new(boundary_id),
        })?;
        Ok(applied.into())
    }

    /// Replaces an interval label.
    ///
    /// # Errors
    /// Rejects when the document, tier, or interval is unknown, or when the text
    /// carries a control character.
    #[wasm_bindgen(js_name = setIntervalLabel)]
    pub fn set_interval_label(
        &mut self,
        annotation_id: u64,
        tier_id: u64,
        interval_id: u64,
        text: String,
    ) -> Result<WasmApplied, JsError> {
        let applied = self.inner.apply(Command::SetLabel {
            annotation: AnnotationId::from_u64(annotation_id),
            target: LabelTarget::Interval {
                tier: TierId::new(tier_id),
                interval: IntervalId::new(interval_id),
            },
            text,
        })?;
        Ok(applied.into())
    }

    /// Replaces a point label.
    ///
    /// # Errors
    /// Rejects when the document, tier, or point is unknown, or when the text
    /// carries a control character.
    #[wasm_bindgen(js_name = setPointLabel)]
    pub fn set_point_label(
        &mut self,
        annotation_id: u64,
        tier_id: u64,
        point_id: u64,
        text: String,
    ) -> Result<WasmApplied, JsError> {
        let applied = self.inner.apply(Command::SetLabel {
            annotation: AnnotationId::from_u64(annotation_id),
            target: LabelTarget::Point {
                tier: TierId::new(tier_id),
                point: PointId::new(point_id),
            },
            text,
        })?;
        Ok(applied.into())
    }

    /// Inserts a point into a point tier at its time-sorted position, returning
    /// the new point id.
    ///
    /// # Errors
    /// Rejects when the document or tier is unknown, when the tier is an
    /// interval tier, when `time` collides with an existing point or lies
    /// outside the domain, or when `label` carries a control character.
    #[wasm_bindgen(js_name = insertPoint)]
    pub fn insert_point(
        &mut self,
        annotation_id: u64,
        tier_id: u64,
        time: f64,
        label: String,
    ) -> Result<u64, JsError> {
        let applied = self.inner.apply(Command::InsertPoint {
            annotation: AnnotationId::from_u64(annotation_id),
            tier: TierId::new(tier_id),
            time,
            label,
        })?;
        match applied {
            Applied::PointInserted { point, .. } => Ok(point.get()),
            _ => Err(JsError::new("insert did not report a point id")),
        }
    }

    /// Moves a point to `to` seconds, keeping its stable id.
    ///
    /// # Errors
    /// Rejects when the document or point is unknown, or when the move would
    /// cross a neighbouring point or leave the domain.
    #[wasm_bindgen(js_name = movePoint)]
    pub fn move_point(
        &mut self,
        annotation_id: u64,
        point_id: u64,
        to: f64,
    ) -> Result<WasmApplied, JsError> {
        let applied = self.inner.apply(Command::MovePoint {
            annotation: AnnotationId::from_u64(annotation_id),
            point: PointId::new(point_id),
            to,
        })?;
        Ok(applied.into())
    }

    /// Removes a point from a point tier.
    ///
    /// # Errors
    /// Rejects when the document or point is unknown.
    #[wasm_bindgen(js_name = removePoint)]
    pub fn remove_point(
        &mut self,
        annotation_id: u64,
        point_id: u64,
    ) -> Result<WasmApplied, JsError> {
        let applied = self.inner.apply(Command::RemovePoint {
            annotation: AnnotationId::from_u64(annotation_id),
            point: PointId::new(point_id),
        })?;
        Ok(applied.into())
    }

    /// Moves a tier to `toIndex` in document order, keeping every stable id.
    ///
    /// `toIndex` is clamped to the last position.
    ///
    /// # Errors
    /// Rejects when the document or tier is unknown.
    #[wasm_bindgen(js_name = reorderTier)]
    pub fn reorder_tier(
        &mut self,
        annotation_id: u64,
        tier_id: u64,
        to_index: usize,
    ) -> Result<WasmApplied, JsError> {
        let applied = self.inner.apply(Command::ReorderTier {
            annotation: AnnotationId::from_u64(annotation_id),
            tier: TierId::new(tier_id),
            to_index,
        })?;
        Ok(applied.into())
    }

    /// Undoes the most recent command. Returns `undefined` when nothing is left
    /// to undo.
    ///
    /// # Errors
    /// Rejects only if a document a stored inverse names has gone missing.
    #[wasm_bindgen(js_name = undo)]
    pub fn undo(&mut self) -> Result<Option<WasmApplied>, JsError> {
        Ok(self.inner.undo()?.map(WasmApplied::from))
    }

    /// Redoes the most recently undone command. Returns `undefined` when nothing
    /// is left to redo.
    ///
    /// # Errors
    /// Rejects only if a document a stored transition names has gone missing.
    #[wasm_bindgen(js_name = redo)]
    pub fn redo(&mut self) -> Result<Option<WasmApplied>, JsError> {
        Ok(self.inner.redo()?.map(WasmApplied::from))
    }

    /// Number of commands that can still be undone.
    #[wasm_bindgen(js_name = undoDepth)]
    #[must_use]
    pub fn undo_depth(&self) -> u32 {
        self.inner.undo_depth() as u32
    }

    /// Number of commands that can still be redone.
    #[wasm_bindgen(js_name = redoDepth)]
    #[must_use]
    pub fn redo_depth(&self) -> u32 {
        self.inner.redo_depth() as u32
    }

    /// Hash of the whole document model, for asserting UI/engine consistency.
    ///
    /// Equal document models hash equal, and undo restores the value it had
    /// before a command (invariant 5, `docs/plan/validation.md`).
    #[wasm_bindgen(js_name = stateHash)]
    #[must_use]
    pub fn state_hash(&self) -> u64 {
        self.inner.state_hash()
    }

    /// Returns every live annotation document attached to `audioId`, ascending
    /// by id.
    ///
    /// Ids climb monotonically and are never reused, so ascending order also
    /// puts the most recently attached document last. A frontend reconciling
    /// its current document against an undo or redo of an attach takes the
    /// last entry as "the" document for this audio, or falls back to the
    /// no-annotation state when the list is empty.
    #[wasm_bindgen(js_name = listAnnotations)]
    #[must_use]
    pub fn list_annotations(&self, audio_id: u64) -> Vec<u64> {
        let audio = AudioId::from_u64(audio_id);
        self.inner
            .annotation_ids()
            .into_iter()
            .filter(|id| self.inner.annotation_audio(*id) == Ok(audio))
            .map(AnnotationId::as_u64)
            .collect()
    }

    /// Returns the tiers of a document as parallel arrays: ids, names, and a
    /// kind flag per tier (`0` interval, `1` point), in document order.
    ///
    /// # Errors
    /// Rejects when `annotationId` names no live document.
    #[wasm_bindgen(js_name = annotationTiers)]
    pub fn annotation_tiers(&self, annotation_id: u64) -> Result<WasmTierList, JsError> {
        let annotation = self
            .inner
            .annotation(AnnotationId::from_u64(annotation_id))?;
        let mut ids = Vec::new();
        let mut names = Vec::new();
        let mut kinds = Vec::new();
        for slot in annotation.tiers() {
            ids.push(slot.id.get());
            match &slot.tier {
                Tier::Interval(tier) => {
                    names.push(tier.name.clone());
                    kinds.push(0u8);
                }
                Tier::Point(tier) => {
                    names.push(tier.name.clone());
                    kinds.push(1u8);
                }
            }
        }
        Ok(WasmTierList { ids, names, kinds })
    }

    /// Returns every interval of an interval tier that overlaps `[t0, t1)`
    /// seconds, as parallel arrays suited to incremental rendering.
    ///
    /// # Errors
    /// Rejects when the document or tier is unknown, or when the tier is a point
    /// tier.
    #[wasm_bindgen(js_name = intervalsInRange)]
    pub fn intervals_in_range(
        &self,
        annotation_id: u64,
        tier_id: u64,
        t0: f64,
        t1: f64,
    ) -> Result<WasmIntervals, JsError> {
        let annotation = self
            .inner
            .annotation(AnnotationId::from_u64(annotation_id))?;
        let slot = annotation
            .tier(TierId::new(tier_id))
            .ok_or_else(|| JsError::new("unknown tier id"))?;
        let Tier::Interval(tier) = &slot.tier else {
            return Err(JsError::new("tier is not an interval tier"));
        };
        let (lo, hi) = (t0.min(t1), t0.max(t1));
        let mut out = WasmIntervals::default();
        for interval in &tier.intervals {
            if interval.xmax > lo && interval.xmin < hi {
                out.ids.push(interval.id.get());
                out.start_boundaries.push(interval.start_boundary.get());
                out.end_boundaries.push(interval.end_boundary.get());
                out.xmin.push(interval.xmin);
                out.xmax.push(interval.xmax);
                out.labels.push(interval.label.clone());
            }
        }
        Ok(out)
    }

    /// Returns every point of a point tier whose time falls in `[t0, t1)`
    /// seconds, as parallel arrays.
    ///
    /// # Errors
    /// Rejects when the document or tier is unknown, or when the tier is an
    /// interval tier.
    #[wasm_bindgen(js_name = pointsInRange)]
    pub fn points_in_range(
        &self,
        annotation_id: u64,
        tier_id: u64,
        t0: f64,
        t1: f64,
    ) -> Result<WasmPoints, JsError> {
        let annotation = self
            .inner
            .annotation(AnnotationId::from_u64(annotation_id))?;
        let slot = annotation
            .tier(TierId::new(tier_id))
            .ok_or_else(|| JsError::new("unknown tier id"))?;
        let Tier::Point(tier) = &slot.tier else {
            return Err(JsError::new("tier is not a point tier"));
        };
        let (lo, hi) = (t0.min(t1), t0.max(t1));
        let mut out = WasmPoints::default();
        for point in &tier.points {
            if point.time >= lo && point.time < hi {
                out.ids.push(point.id.get());
                out.times.push(point.time);
                out.labels.push(point.label.clone());
            }
        }
        Ok(out)
    }

    /// Searches interval and point labels across every attached document.
    ///
    /// When `regex` is set, `pattern` is a Rust `regex` crate expression;
    /// invalid syntax yields no hits. Otherwise `pattern` is a literal
    /// substring. Each hit reports its document, tier, target, and byte span.
    #[wasm_bindgen(js_name = searchLabels)]
    #[must_use]
    pub fn search_labels(&self, pattern: String, regex: bool) -> WasmHits {
        let query = LabelQuery {
            pattern: if regex {
                LabelPattern::Regex(pattern)
            } else {
                LabelPattern::Substring(pattern)
            },
            tiers: None,
        };
        let mut out = WasmHits::default();
        for hit in self.inner.search_labels(&query) {
            out.annotations.push(hit.annotation.as_u64());
            out.tiers.push(hit.hit.tier.get());
            let (kind, target) = match hit.hit.target {
                LabelTarget::Interval { interval, .. } => (0u8, interval.get()),
                LabelTarget::Point { point, .. } => (1u8, point.get()),
            };
            out.kinds.push(kind);
            out.targets.push(target);
            out.starts.push(hit.hit.span.start as u32);
            out.ends.push(hit.hit.span.end as u32);
        }
        out
    }

    /// Reads a Praat TextGrid (long or short text format; UTF-8/UTF-16/Latin-1)
    /// and attaches the parsed annotation to `audioId`, returning the new
    /// annotation id.
    ///
    /// The attachment is journaled like any other command, so undo detaches the
    /// imported document. Tier relations are not carried by the TextGrid format;
    /// every imported tier is independent.
    ///
    /// # Errors
    /// Rejects when the bytes are not a TextGrid this crate can read, when the
    /// parsed document fails validation, or when `audioId` names no live buffer.
    #[wasm_bindgen(js_name = importTextGrid)]
    pub fn import_text_grid(&mut self, audio_id: u64, bytes: &[u8]) -> Result<u64, JsError> {
        let (annotation, _source) =
            phx_textgrid::read(bytes).map_err(|err| JsError::new(&err.to_string()))?;
        let applied = self.inner.apply(Command::AttachAnnotation {
            audio: AudioId::from_u64(audio_id),
            annotation,
        })?;
        match applied {
            Applied::AnnotationAttached { annotation, .. } => Ok(annotation.as_u64()),
            _ => Err(JsError::new("attach did not report an annotation id")),
        }
    }

    /// Serializes a document to a Praat TextGrid: long text format, UTF-8, `LF`
    /// line endings, no byte-order mark.
    ///
    /// # Errors
    /// Rejects when `annotationId` names no live document.
    #[wasm_bindgen(js_name = exportTextGrid)]
    pub fn export_text_grid(&self, annotation_id: u64) -> Result<Uint8Array, JsError> {
        let annotation = self
            .inner
            .annotation(AnnotationId::from_u64(annotation_id))?;
        let bytes = phx_textgrid::write(annotation);
        Ok(Uint8Array::from(bytes.as_slice()))
    }

    /// Assembles a figure from live session state and returns it as JSON.
    ///
    /// `specJson` is a serialized [`phx_engine::FigureRequest`]: the audio and
    /// optional annotation ids, the time and frequency window, the per-layer
    /// toggles, the physical size and palette, and the analysis parameters. The
    /// returned JSON is a self-contained [`phx_engine::Figure`] the preview and
    /// every export backend consume — the dialog builds it once, then renders
    /// and exports off the same bytes.
    ///
    /// # Errors
    /// Rejects when `specJson` is not a valid request, when an id names no live
    /// object, or when the window or an analysis parameter is unusable.
    #[wasm_bindgen(js_name = buildFigure)]
    pub fn build_figure(&self, spec_json: &str) -> Result<String, JsError> {
        let request: FigureRequest = serde_json::from_str(spec_json)
            .map_err(|err| JsError::new(&format!("invalid figure request: {err}")))?;
        let figure = self.inner.build_figure(&request)?;
        figure
            .to_json()
            .map_err(|err| JsError::new(&err.to_string()))
    }

    /// Serializes the annotation document `annotationId` to its JSON form.
    ///
    /// The JSON is the same annotation payload a project container stores under
    /// `annotations/<id>.json`, so the host can carry a document through OPFS and
    /// hand it back to [`WasmEngine::attach_annotation_json`] unchanged.
    ///
    /// # Errors
    /// Rejects when `annotationId` names no live document.
    #[wasm_bindgen(js_name = annotationJson)]
    pub fn annotation_json(&self, annotation_id: u64) -> Result<String, JsError> {
        let annotation = self
            .inner
            .annotation(AnnotationId::from_u64(annotation_id))?;
        serde_json::to_string(annotation).map_err(|err| JsError::new(&err.to_string()))
    }

    /// Attaches a document deserialized from `json` to `audioId`, returning the
    /// new annotation id.
    ///
    /// The attachment is journaled like any other command, so undo detaches the
    /// restored document. This is the load-time counterpart of
    /// [`WasmEngine::annotation_json`].
    ///
    /// # Errors
    /// Rejects when `json` is not a valid annotation document, when it fails
    /// validation, or when `audioId` names no live buffer.
    #[wasm_bindgen(js_name = attachAnnotationJson)]
    pub fn attach_annotation_json(&mut self, audio_id: u64, json: &str) -> Result<u64, JsError> {
        let annotation: Annotation =
            serde_json::from_str(json).map_err(|err| JsError::new(&err.to_string()))?;
        let applied = self.inner.apply(Command::AttachAnnotation {
            audio: AudioId::from_u64(audio_id),
            annotation,
        })?;
        match applied {
            Applied::AnnotationAttached { annotation, .. } => Ok(annotation.as_u64()),
            _ => Err(JsError::new("attach did not report an annotation id")),
        }
    }

    /// Serializes a project container from `specJson` and the live documents it
    /// names, returning the versioned ZIP bytes.
    ///
    /// `specJson` is a [`SaveProjectSpec`]: the project name, save timestamp,
    /// opaque view blob, and one media entry per recording. Each entry carries
    /// the reference fields the manifest stores (relative path, content hash,
    /// duration, sample rate, channels) and, when the recording is annotated,
    /// the session annotation id whose document is pulled from the engine and
    /// stored under the media's stable id. Media stays external; only the
    /// documents cross into the container.
    ///
    /// # Errors
    /// Rejects when `specJson` does not parse, when a hash is not 64 hex
    /// characters, or when an annotation id names no live document.
    #[wasm_bindgen(js_name = saveProjectContainer)]
    pub fn save_project_container(&self, spec_json: &str) -> Result<Uint8Array, JsError> {
        let spec: SaveProjectSpec = serde_json::from_str(spec_json)
            .map_err(|err| JsError::new(&format!("invalid project spec: {err}")))?;
        let mut project = Project::new(spec.name);
        project.saved_at = spec.saved_at;
        project.view = spec.view;
        for media in spec.media {
            let hash =
                ContentHash::from_hex(&media.hash).map_err(|err| JsError::new(&err.to_string()))?;
            let media_id = MediaId::new(media.media_id);
            project.media.push(MediaRef {
                id: media_id,
                relative_path: media.relative_path,
                hash,
                duration: media.duration,
                sample_rate: media.sample_rate,
                channels: media.channels,
            });
            if let Some(annotation_id) = media.annotation {
                let annotation = self
                    .inner
                    .annotation(AnnotationId::from_u64(annotation_id))?
                    .clone();
                project.annotations.insert(media_id, annotation);
            }
        }
        Ok(Uint8Array::from(phx_project::save(&project).as_slice()))
    }
}

/// One media entry in a [`SaveProjectSpec`].
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveProjectMedia {
    media_id: u64,
    relative_path: String,
    hash: String,
    duration: f64,
    sample_rate: f64,
    channels: usize,
    #[serde(default)]
    annotation: Option<u64>,
}

/// The `saveProjectContainer` argument: project metadata and its media entries.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveProjectSpec {
    name: String,
    saved_at: u64,
    #[serde(default)]
    view: Value,
    media: Vec<SaveProjectMedia>,
}

/// One media entry returned by [`load_project_container`], with its document
/// serialized inline for the host to re-attach after importing the audio.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LoadProjectMedia {
    media_id: u64,
    relative_path: String,
    hash: String,
    duration: f64,
    sample_rate: f64,
    channels: usize,
    annotation_json: Option<String>,
}

/// The `loadProjectContainer` result: project metadata and its media entries.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LoadProjectResult {
    name: String,
    saved_at: u64,
    view: Value,
    media: Vec<LoadProjectMedia>,
}

/// Parses a project container into JSON the host can drive the session from.
///
/// Returns the project name, its save timestamp, the opaque view blob, and one
/// entry per referenced recording. A recording that carried an annotation gets
/// its document serialized inline (`annotationJson`); the host imports the audio
/// from its own store, then hands each document back through
/// [`WasmEngine::attach_annotation_json`]. Recovery compares the `savedAt` this
/// returns for a project file against the same field of its autosave sidecar.
///
/// # Errors
/// Rejects when the bytes are not a readable project container this build
/// understands.
#[wasm_bindgen(js_name = loadProjectContainer)]
pub fn load_project_container(bytes: &[u8]) -> Result<String, JsError> {
    let project = phx_project::load(bytes).map_err(|err| JsError::new(&err.to_string()))?;
    let mut media = Vec::with_capacity(project.media.len());
    for reference in &project.media {
        let annotation_json = match project.annotations.get(&reference.id) {
            Some(annotation) => Some(
                serde_json::to_string(annotation).map_err(|err| JsError::new(&err.to_string()))?,
            ),
            None => None,
        };
        media.push(LoadProjectMedia {
            media_id: reference.id.get(),
            relative_path: reference.relative_path.clone(),
            hash: reference.hash.to_hex(),
            duration: reference.duration,
            sample_rate: reference.sample_rate,
            channels: reference.channels,
            annotation_json,
        });
    }
    let result = LoadProjectResult {
        name: project.name,
        saved_at: project.saved_at,
        view: project.view,
        media,
    };
    serde_json::to_string(&result).map_err(|err| JsError::new(&err.to_string()))
}

/// Rewrites a project container's name, preserving everything else.
///
/// Loads the container and re-saves it with `name`, keeping media references,
/// annotations, profiles, view state, and save timestamp. This renames a stored
/// project without decoding its recordings into a session.
///
/// # Errors
/// Rejects when the bytes are not a readable project container this build
/// understands.
#[wasm_bindgen(js_name = renameProjectContainer)]
pub fn rename_project_container(bytes: &[u8], name: &str) -> Result<Uint8Array, JsError> {
    let mut project = phx_project::load(bytes).map_err(|err| JsError::new(&err.to_string()))?;
    project.name = name.to_string();
    Ok(Uint8Array::from(phx_project::save(&project).as_slice()))
}

/// Returns the BLAKE3 content hash of `bytes` as 64 lowercase hex characters.
///
/// This is the content address a project manifest records for a recording, so
/// the host computes it once at import and stores it beside the media.
#[wasm_bindgen(js_name = contentHash)]
#[must_use]
pub fn content_hash(bytes: &[u8]) -> String {
    ContentHash::of(bytes).to_hex()
}

/// A figure export bundle crossing the boundary: the main document plus any
/// sidecar files a text backend references.
#[wasm_bindgen]
pub struct WasmExportBundle {
    main_name: String,
    main_bytes: Vec<u8>,
    mime: String,
    is_text: bool,
    sidecar_names: Vec<String>,
    sidecar_bytes: Vec<Vec<u8>>,
}

#[wasm_bindgen]
impl WasmExportBundle {
    /// Suggested file name for the main document.
    #[wasm_bindgen(getter, js_name = mainName)]
    pub fn main_name(&self) -> String {
        self.main_name.clone()
    }

    /// Main document bytes.
    #[wasm_bindgen(getter, js_name = mainBytes)]
    pub fn main_bytes(&self) -> Uint8Array {
        Uint8Array::from(self.main_bytes.as_slice())
    }

    /// MIME type of the main document.
    #[wasm_bindgen(getter)]
    pub fn mime(&self) -> String {
        self.mime.clone()
    }

    /// Whether the main document is UTF-8 text.
    #[wasm_bindgen(getter, js_name = isText)]
    pub fn is_text(&self) -> bool {
        self.is_text
    }

    /// Sidecar file names, in bundle order.
    #[wasm_bindgen(getter, js_name = sidecarNames)]
    pub fn sidecar_names(&self) -> Vec<String> {
        self.sidecar_names.clone()
    }

    /// Bytes of sidecar `index`, or an empty array when out of range.
    #[wasm_bindgen(js_name = sidecarBytes)]
    pub fn sidecar_bytes(&self, index: usize) -> Uint8Array {
        match self.sidecar_bytes.get(index) {
            Some(bytes) => Uint8Array::from(bytes.as_slice()),
            None => Uint8Array::new_with_length(0),
        }
    }
}

impl From<ExportBundle> for WasmExportBundle {
    fn from(bundle: ExportBundle) -> Self {
        let mut sidecar_names = Vec::with_capacity(bundle.sidecars.len());
        let mut sidecar_bytes = Vec::with_capacity(bundle.sidecars.len());
        for sidecar in bundle.sidecars {
            sidecar_names.push(sidecar.name);
            sidecar_bytes.push(sidecar.bytes);
        }
        Self {
            main_name: bundle.main_name,
            main_bytes: bundle.main_bytes,
            mime: bundle.mime,
            is_text: bundle.is_text,
            sidecar_names,
            sidecar_bytes,
        }
    }
}

/// Renders a figure JSON string to its SVG scene graph.
///
/// This is the preview backend: the SVG it returns is byte-for-byte the SVG an
/// SVG export writes, so a preview built from a figure equals that figure's
/// export by construction.
///
/// # Errors
/// Rejects when `figureJson` is not a figure this build can decode.
#[wasm_bindgen(js_name = renderFigureSvg)]
pub fn render_figure_svg(figure_json: &str) -> Result<String, JsError> {
    let figure = Figure::from_json(figure_json).map_err(|err| JsError::new(&err.to_string()))?;
    Ok(figure_to_svg(&figure))
}

/// Exports a figure JSON string to a downloadable bundle in `format`.
///
/// `format` is one of `svg`, `png`, `pdf`, `vega`, `tikz`, `typst`, `python`,
/// `r`, `julia`, `graphml`. On the wasm build `png` and `pdf` are native-only
/// and reject; the web app rasterizes the SVG preview for PNG instead.
///
/// # Errors
/// Rejects when `figureJson` is undecodable, when `format` is unknown, or when a
/// native-only format is requested on this build.
#[wasm_bindgen(js_name = exportFigure)]
pub fn export_figure(figure_json: &str, format: &str) -> Result<WasmExportBundle, JsError> {
    let figure = Figure::from_json(figure_json).map_err(|err| JsError::new(&err.to_string()))?;
    let format = parse_figure_format(format)?;
    let bundle = engine_export_figure(&figure, format)?;
    Ok(bundle.into())
}

/// Parses a figure format name into a [`FigureFormat`].
fn parse_figure_format(name: &str) -> Result<FigureFormat, JsError> {
    match name {
        "svg" => Ok(FigureFormat::Svg),
        "png" => Ok(FigureFormat::Png),
        "pdf" => Ok(FigureFormat::Pdf),
        "vega" => Ok(FigureFormat::Vega),
        "tikz" => Ok(FigureFormat::Tikz),
        "typst" => Ok(FigureFormat::Typst),
        "python" => Ok(FigureFormat::Python),
        "r" => Ok(FigureFormat::R),
        "julia" => Ok(FigureFormat::Julia),
        "graphml" => Ok(FigureFormat::Graphml),
        other => Err(JsError::new(&format!("unknown figure format: {other}"))),
    }
}

/// Pulls the tier id out of a [`Applied::TierAdded`] report.
fn tier_id_of(applied: Applied) -> Result<u64, JsError> {
    match applied {
        Applied::TierAdded { tier, .. } => Ok(tier.get()),
        _ => Err(JsError::new("add tier did not report a tier id")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::wasm_bindgen_test;

    const FIXTURE_WAV: &[u8] = include_bytes!("../../../tests/fixtures/audio/arctic_bdl_a0001.wav");
    const VOWEL_WAV: &[u8] = include_bytes!("../../../tests/fixtures/audio/synth_vowel_a.wav");

    #[wasm_bindgen_test]
    fn selection_and_voice_report_cross_the_boundary() {
        let mut engine = WasmEngine::new();
        let id = engine.import_wav_bytes(VOWEL_WAV).unwrap();
        let info = engine.audio_info(id).unwrap();
        let (t0, t1, f0, f1) = (info.duration() * 0.3, info.duration() * 0.6, 0.0, 4000.0);

        let direct = engine.band_energy(id, t0, t1, f0, f1).unwrap();
        assert!(direct.is_finite());

        let readout_json = engine
            .selection_readout(id, t0, t1, f0, f1, 75.0, 600.0, 100.0)
            .unwrap();
        let readout: serde_json::Value = serde_json::from_str(&readout_json).unwrap();
        // The batch-equals-GUI invariant across the JS boundary: the readout's
        // band energy is bit-identical to the direct query.
        assert_eq!(
            readout["bandEnergyDb"].as_f64().unwrap().to_bits(),
            direct.to_bits()
        );
        assert!(readout["f0MeanHz"].as_f64().unwrap() > 0.0);

        let means = engine
            .formant_span_means(id, 5000.0, 5, false, t0, t1)
            .unwrap();
        assert_eq!(means.length(), 5);

        let report_json = engine.voice_report(id, t0, t1, 75.0, 600.0).unwrap();
        let report: serde_json::Value = serde_json::from_str(&report_json).unwrap();
        assert!(report["jitter"]["local"].as_f64().unwrap() < 0.05);
        assert!(report["meanHnrDb"].as_f64().unwrap() > 10.0);
        assert!(report["moments"]["centreOfGravityHz"].as_f64().unwrap() > 0.0);
    }

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

    #[wasm_bindgen_test]
    fn recording_begin_append_finish_round_trips_through_bindings() {
        let mut engine = WasmEngine::new();
        let sample_rate = 16_000.0;
        let samples: Vec<f32> = (0..1_600).map(|i| (i as f32 * 0.01).sin() * 0.5).collect();

        let rec = engine.begin_recording(sample_rate, 1).unwrap();
        for chunk in samples.chunks(400) {
            engine.append_samples(rec, chunk).unwrap();
        }
        let finished = engine.finish_recording(rec, "take".to_string()).unwrap();
        let audio_id = finished.audio();
        assert!(finished.wav().length() > 44);

        // The materialized take reports the captured rate and full duration, and
        // its waveform reads back as a real signal.
        let info = engine.audio_info(audio_id).unwrap();
        assert_eq!(info.sample_rate(), sample_rate);
        assert!((info.duration() - samples.len() as f64 / sample_rate).abs() < 1.0e-6);
        let waveform = engine
            .waveform_slice(audio_id, 0.0, info.duration(), 16)
            .unwrap();
        assert_eq!(waveform.length(), 32);

        // The spent id rejects a second finish.
        assert!(engine.finish_recording(rec, "again".to_string()).is_err());
    }

    #[wasm_bindgen_test]
    fn aborted_recording_rejects_further_use() {
        let mut engine = WasmEngine::new();
        let rec = engine.begin_recording(8_000.0, 1).unwrap();
        engine.append_samples(rec, &[0.0; 64]).unwrap();
        engine.abort_recording(rec).unwrap();
        assert!(engine.append_samples(rec, &[0.0; 4]).is_err());
        assert!(engine.abort_recording(rec).is_err());
    }

    #[wasm_bindgen_test]
    fn annotation_apply_undo_redo_through_bindings() {
        let mut engine = WasmEngine::new();
        let audio = engine.import_wav_bytes(FIXTURE_WAV).unwrap();
        let info = engine.audio_info(audio).unwrap();

        let doc = engine
            .create_annotation(audio, 0.0, info.duration())
            .unwrap();
        let tier = engine
            .add_interval_tier(
                doc,
                "phones".to_string(),
                WasmTierRelation::Independent,
                None,
            )
            .unwrap();

        let before = engine.state_hash();
        let boundary = engine
            .insert_boundary(doc, tier, info.duration() / 2.0)
            .unwrap();
        let after_insert = engine.state_hash();
        assert_ne!(before, after_insert);

        // Two intervals now cover the domain; label the first.
        let intervals = engine
            .intervals_in_range(doc, tier, 0.0, info.duration())
            .unwrap();
        assert_eq!(intervals.ids().len(), 2);
        let first = intervals.ids()[0];
        let applied = engine
            .set_interval_label(doc, tier, first, "aː".to_string())
            .unwrap();
        assert_eq!(applied.kind(), "labelSet");
        assert_eq!(applied.annotation(), Some(doc));

        let hits = engine.search_labels("aː".to_string(), false);
        assert_eq!(hits.annotations().len(), 1);
        assert_eq!(hits.annotations()[0], doc);

        // Undo the label, then the boundary; each returns to a prior hash.
        let undone = engine.undo().unwrap().unwrap();
        assert_eq!(undone.kind(), "labelSet");
        assert_eq!(engine.state_hash(), after_insert);
        engine.undo().unwrap();
        assert_eq!(engine.state_hash(), before);

        // Redo both back to the final state.
        engine.redo().unwrap();
        engine.redo().unwrap();
        let intervals = engine
            .intervals_in_range(doc, tier, 0.0, info.duration())
            .unwrap();
        assert_eq!(intervals.labels()[0], "aː");
        let _ = boundary;

        let tiers = engine.annotation_tiers(doc).unwrap();
        assert_eq!(tiers.ids(), vec![tier]);
    }

    #[wasm_bindgen_test]
    fn textgrid_export_then_import_round_trips_labels() {
        let mut engine = WasmEngine::new();
        let audio = engine.import_wav_bytes(FIXTURE_WAV).unwrap();
        let info = engine.audio_info(audio).unwrap();

        let doc = engine
            .create_annotation(audio, 0.0, info.duration())
            .unwrap();
        let tier = engine
            .add_interval_tier(
                doc,
                "phones".to_string(),
                WasmTierRelation::Independent,
                None,
            )
            .unwrap();
        engine
            .insert_boundary(doc, tier, info.duration() / 2.0)
            .unwrap();
        let intervals = engine
            .intervals_in_range(doc, tier, 0.0, info.duration())
            .unwrap();
        let first = intervals.ids()[0];
        engine
            .set_interval_label(doc, tier, first, "aː".to_string())
            .unwrap();

        let bytes = engine.export_text_grid(doc).unwrap().to_vec();
        let reimported = engine.import_text_grid(audio, &bytes).unwrap();
        let tiers = engine.annotation_tiers(reimported).unwrap();
        assert_eq!(tiers.names(), vec!["phones".to_string()]);
        let reintervals = engine
            .intervals_in_range(reimported, tiers.ids()[0], 0.0, info.duration())
            .unwrap();
        assert_eq!(reintervals.labels()[0], "aː");
    }

    #[wasm_bindgen_test]
    fn list_annotations_tracks_attach_undo_redo() {
        let mut engine = WasmEngine::new();
        let audio = engine.import_wav_bytes(FIXTURE_WAV).unwrap();
        let info = engine.audio_info(audio).unwrap();

        assert!(engine.list_annotations(audio).is_empty());

        let first = engine
            .create_annotation(audio, 0.0, info.duration())
            .unwrap();
        assert_eq!(engine.list_annotations(audio), vec![first]);

        let bytes = engine.export_text_grid(first).unwrap().to_vec();
        let second = engine.import_text_grid(audio, &bytes).unwrap();
        assert_eq!(engine.list_annotations(audio), vec![first, second]);

        // Undo the import: the second document detaches and the first, older
        // document is what a frontend should repoint to.
        let undone = engine.undo().unwrap().unwrap();
        assert_eq!(undone.kind(), "annotationDetached");
        assert_eq!(undone.annotation(), Some(second));
        assert_eq!(engine.list_annotations(audio), vec![first]);

        // Redo restores the second document under its original id.
        let redone = engine.redo().unwrap().unwrap();
        assert_eq!(redone.kind(), "annotationAttached");
        assert_eq!(redone.annotation(), Some(second));
        assert_eq!(redone.audio(), Some(audio));
        assert_eq!(engine.list_annotations(audio), vec![first, second]);
    }

    #[wasm_bindgen_test]
    fn undo_with_empty_journal_returns_none() {
        let mut engine = WasmEngine::new();
        assert!(engine.undo().unwrap().is_none());
        assert!(engine.redo().unwrap().is_none());
        assert_eq!(engine.undo_depth(), 0);
    }

    #[wasm_bindgen_test]
    fn point_insert_move_remove_round_trip_through_bindings() {
        let mut engine = WasmEngine::new();
        let audio = engine.import_wav_bytes(FIXTURE_WAV).unwrap();
        let info = engine.audio_info(audio).unwrap();
        let doc = engine
            .create_annotation(audio, 0.0, info.duration())
            .unwrap();
        let tier = engine.add_point_tier(doc, "tones".to_string()).unwrap();

        let empty = engine.state_hash();
        let at = info.duration() / 2.0;
        let point = engine.insert_point(doc, tier, at, "H".to_string()).unwrap();
        let inserted = engine.state_hash();
        assert_ne!(inserted, empty);

        let points = engine
            .points_in_range(doc, tier, 0.0, info.duration())
            .unwrap();
        assert_eq!(points.ids(), vec![point]);
        assert_eq!(points.labels(), vec!["H".to_string()]);

        let moved = engine
            .move_point(doc, point, info.duration() / 3.0)
            .unwrap();
        assert_eq!(moved.kind(), "pointMoved");
        assert_eq!(moved.point(), Some(point));

        let removed = engine.remove_point(doc, point).unwrap();
        assert_eq!(removed.kind(), "pointRemoved");
        assert_eq!(engine.state_hash(), empty);

        // Undo the removal, move, and insertion back to the empty tier, then
        // redo the insertion and confirm the id and label return unchanged.
        engine.undo().unwrap();
        engine.undo().unwrap();
        engine.undo().unwrap();
        assert_eq!(engine.state_hash(), empty);
        let restored = engine.redo().unwrap().unwrap();
        assert_eq!(restored.kind(), "pointInserted");
        assert_eq!(restored.point(), Some(point));
        assert_eq!(engine.state_hash(), inserted);
        let points = engine
            .points_in_range(doc, tier, 0.0, info.duration())
            .unwrap();
        assert_eq!(points.ids(), vec![point]);
        assert_eq!(points.labels(), vec!["H".to_string()]);
    }

    #[wasm_bindgen_test]
    fn project_container_round_trips_media_and_annotation() {
        let mut engine = WasmEngine::new();
        let audio = engine.import_wav_bytes(FIXTURE_WAV).unwrap();
        let info = engine.audio_info(audio).unwrap();
        let doc = engine
            .create_annotation(audio, 0.0, info.duration())
            .unwrap();
        let tier = engine
            .add_interval_tier(
                doc,
                "phones".to_string(),
                WasmTierRelation::Independent,
                None,
            )
            .unwrap();
        engine
            .insert_boundary(doc, tier, info.duration() / 2.0)
            .unwrap();
        let intervals = engine
            .intervals_in_range(doc, tier, 0.0, info.duration())
            .unwrap();
        engine
            .set_interval_label(doc, tier, intervals.ids()[0], "aː".to_string())
            .unwrap();

        let hash = content_hash(FIXTURE_WAV);
        let spec = format!(
            "{{\"name\":\"Fieldwork\",\"savedAt\":42,\"view\":null,\"media\":[{{\
             \"mediaId\":7,\"relativePath\":\"audio/a.wav\",\"hash\":\"{hash}\",\
             \"duration\":{},\"sampleRate\":{},\"channels\":{},\"annotation\":{doc}}}]}}",
            info.duration(),
            info.sample_rate(),
            info.channels()
        );
        let bytes = engine.save_project_container(&spec).unwrap().to_vec();

        // Loading returns the media and an inline annotation document.
        let loaded_json = load_project_container(&bytes).unwrap();
        let loaded: serde_json::Value = serde_json::from_str(&loaded_json).unwrap();
        assert_eq!(loaded["name"], "Fieldwork");
        assert_eq!(loaded["savedAt"], 42);
        let media = loaded["media"].as_array().unwrap();
        assert_eq!(media.len(), 1);
        assert_eq!(media[0]["mediaId"], 7);
        assert_eq!(media[0]["hash"], hash);
        let annotation_json = media[0]["annotationJson"].as_str().unwrap();

        // Re-attaching the document to a fresh audio restores the label.
        let mut restored = WasmEngine::new();
        let audio2 = restored.import_wav_bytes(FIXTURE_WAV).unwrap();
        let doc2 = restored
            .attach_annotation_json(audio2, annotation_json)
            .unwrap();
        let tiers = restored.annotation_tiers(doc2).unwrap();
        let reintervals = restored
            .intervals_in_range(doc2, tiers.ids()[0], 0.0, info.duration())
            .unwrap();
        assert_eq!(reintervals.labels()[0], "aː");

        // Renaming the container keeps the annotation reachable.
        let renamed = rename_project_container(&bytes, "Renamed")
            .unwrap()
            .to_vec();
        let after: serde_json::Value =
            serde_json::from_str(&load_project_container(&renamed).unwrap()).unwrap();
        assert_eq!(after["name"], "Renamed");
        assert!(after["media"][0]["annotationJson"].is_string());
    }

    #[wasm_bindgen_test]
    fn aligned_interval_tier_moves_both_tiers_and_reorders() {
        let mut engine = WasmEngine::new();
        let audio = engine.import_wav_bytes(FIXTURE_WAV).unwrap();
        let info = engine.audio_info(audio).unwrap();
        let doc = engine
            .create_annotation(audio, 0.0, info.duration())
            .unwrap();
        let primary = engine
            .add_interval_tier(
                doc,
                "phones".to_string(),
                WasmTierRelation::Independent,
                None,
            )
            .unwrap();
        let aligned = engine
            .add_interval_tier(
                doc,
                "words".to_string(),
                WasmTierRelation::AlignedBoundaries,
                Some(primary),
            )
            .unwrap();

        // A boundary inserted on the primary tier propagates to the aligned peer.
        let boundary = engine
            .insert_boundary(doc, primary, info.duration() / 2.0)
            .unwrap();
        for tier in [primary, aligned] {
            let intervals = engine
                .intervals_in_range(doc, tier, 0.0, info.duration())
                .unwrap();
            assert_eq!(intervals.ids().len(), 2);
        }

        // Moving the shared boundary linked moves both tiers; one undo restores.
        let before_move = engine.state_hash();
        engine
            .move_boundary(doc, boundary, info.duration() * 0.6, true)
            .unwrap();
        engine.undo().unwrap();
        assert_eq!(engine.state_hash(), before_move);

        // Reordering brings the aligned tier to the front; undo restores order.
        let before_reorder = engine.state_hash();
        let applied = engine.reorder_tier(doc, aligned, 0).unwrap();
        assert_eq!(applied.kind(), "tierReordered");
        assert_eq!(engine.annotation_tiers(doc).unwrap().ids()[0], aligned);
        engine.undo().unwrap();
        assert_eq!(engine.state_hash(), before_reorder);
        assert_eq!(engine.annotation_tiers(doc).unwrap().ids()[0], primary);
    }
}
