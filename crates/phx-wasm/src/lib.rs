//! wasm-bindgen bindings over phx-engine.
#![warn(missing_docs)]

use js_sys::{Float32Array, Float64Array, Uint8Array};
use phx_engine::{
    AlignMode, Annotation, AnnotationId, Applied, AudioId, BoundaryId, Colormap as EngineColormap,
    Command, DisplayMapping, Engine, FormantParams, IntensityParams, IntervalId, LabelPattern,
    LabelQuery, LabelTarget, PitchParams, PointId, SpectrogramParams, Theme as EngineTheme, Tier,
    TierId, TierRelation, TileRequest,
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
}

impl From<Applied> for WasmApplied {
    fn from(applied: Applied) -> Self {
        let (kind, annotation, audio, tier, boundary) = match applied {
            Applied::AudioImported { audio } => ("audioImported", None, Some(audio), None, None),
            Applied::AudioRemoved { audio } => ("audioRemoved", None, Some(audio), None, None),
            Applied::AnnotationAttached { annotation, audio } => (
                "annotationAttached",
                Some(annotation),
                Some(audio),
                None,
                None,
            ),
            Applied::AnnotationDetached { annotation } => {
                ("annotationDetached", Some(annotation), None, None, None)
            }
            Applied::TierAdded { annotation, tier } => {
                ("tierAdded", Some(annotation), None, Some(tier), None)
            }
            Applied::TierRemoved { annotation, tier } => {
                ("tierRemoved", Some(annotation), None, Some(tier), None)
            }
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
            ),
            Applied::BoundaryMoved { annotation, .. } => {
                ("boundaryMoved", Some(annotation), None, None, None)
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
            ),
            Applied::BoundaryRestored { annotation, .. } => {
                ("boundaryRestored", Some(annotation), None, None, None)
            }
            Applied::LabelSet { annotation, .. } => {
                ("labelSet", Some(annotation), None, None, None)
            }
        };
        Self {
            kind: kind.to_string(),
            annotation: annotation.map(IdExt::id),
            audio: audio.map(IdExt::id),
            tier: tier.map(IdExt::id),
            boundary: boundary.map(IdExt::id),
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

    /// Adds an independent interval tier holding one unlabeled interval over the
    /// whole domain, returning the new tier id.
    ///
    /// # Errors
    /// Rejects when `annotationId` names no live document.
    #[wasm_bindgen(js_name = addIntervalTier)]
    pub fn add_interval_tier(&mut self, annotation_id: u64, name: String) -> Result<u64, JsError> {
        let applied = self.inner.apply(Command::AddIntervalTier {
            annotation: AnnotationId::from_u64(annotation_id),
            name,
            relation: TierRelation::Independent,
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
    fn annotation_apply_undo_redo_through_bindings() {
        let mut engine = WasmEngine::new();
        let audio = engine.import_wav_bytes(FIXTURE_WAV).unwrap();
        let info = engine.audio_info(audio).unwrap();

        let doc = engine
            .create_annotation(audio, 0.0, info.duration())
            .unwrap();
        let tier = engine.add_interval_tier(doc, "phones".to_string()).unwrap();

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
        let tier = engine.add_interval_tier(doc, "phones".to_string()).unwrap();
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
    fn undo_with_empty_journal_returns_none() {
        let mut engine = WasmEngine::new();
        assert!(engine.undo().unwrap().is_none());
        assert!(engine.redo().unwrap().is_none());
        assert_eq!(engine.undo_depth(), 0);
    }
}
