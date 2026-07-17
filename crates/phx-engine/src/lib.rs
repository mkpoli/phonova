//! The one API both frontends and future bindings consume: commands with
//! explicit arguments, journaled unified undo, content-addressed analysis
//! cache.
//!
//! The mutation surface is a single journaled command path: every change to
//! session state — audio import, annotation attachment, tier lifecycle, and
//! the boundary and label edits of the annotation loop — goes through
//! [`Engine::apply`], which records an id-stable inverse so [`Engine::undo`]
//! and [`Engine::redo`] restore state-hash-identical documents (design rule 5,
//! `docs/plan/architecture.md`; invariant 5, `docs/plan/validation.md`). The
//! journal is in memory; persisting a session's history to the project file
//! arrives with `phx-project` in phase 4. Analyses (pitch, formants,
//! intensity, spectrogram tiles) stay outside the journal — they are pure
//! functions of `(audio, params)` and never mutate a document.
#![warn(missing_docs)]

mod commands;
mod document;
mod error;
mod figure;
mod journal;
mod pyramid;
mod recording;
mod store;
mod stream_pyramid;
mod tile_cache;

use std::hash::{Hash, Hasher};
use std::sync::Mutex;

use phx_spectrogram::{analysis_axes, compute_column_block, select_axis_indices};

use tile_cache::{BlockKey, TILE_COLS, TileCache, params_hash};

use phx_audio::{Audio, BitDepth, StreamingWav};
use phx_dsp::Window;

use std::sync::Arc;

use stream_pyramid::StreamPyramid;

use document::DocumentStore;
use journal::{Journal, JournalEntry, Reverse};

pub use commands::{Applied, Command, EngineHit};
pub use document::{AnnotationId, Document};
pub use error::EngineError;
pub use figure::{
    ExportBundle, FigureColormap, FigureFormat, FigurePitchUnit, FigureRequest, FigureTheme,
    FigureUnit, LayerToggles, default_figure_request, export_figure, figure_to_svg,
};
pub use phx_annot::{
    AlignMode, Annotation, AnnotationError, BoundaryId, BoundaryMove, Hit, IntegrityIssue,
    Interval, IntervalId, IntervalTier, LabelPattern, LabelQuery, LabelTarget, MatchSpan, Merged,
    Moved, Point, PointId, PointTier, Tier, TierId, TierKind, TierMerge, TierRelation, TierSlot,
};
pub use phx_audio::{AudioError, ByteReader, BytesReader, StreamSampleFormat, WavStreamInfo};
pub use phx_figure::Figure;
pub use phx_formant::{FormantFrame, FormantParams, FormantPoint, FormantTrack};
pub use phx_intensity::{IntensityParams, IntensityTrack};
pub use phx_pitch::{PitchFrame, PitchParams, PitchTrack, TimeSpan};
pub use phx_render::{Colormap, DisplayMapping, Theme};
pub use phx_spectrogram::{SpectrogramParams, Tile, TileRequest};
pub use phx_voice::{
    CppParams, HarmonicityParams, JitterMeasures, Moments, PitchSummary, PointProcess, PulseParams,
    ShimmerMeasures, VoiceBreaks, VoiceReport,
};
pub use pyramid::MinMax;
pub use recording::{FinishedRecording, RecordingId};
pub use store::{AudioId, AudioInfo, AudioStore, SampleAccess};

use recording::RecordingStore;

/// Frame count at or below which audio stays on the eager whole-decode path.
///
/// Two minutes at 48 kHz. Below it, a per-sample pyramid and a resident buffer
/// cost little and keep every analysis a borrow away; above it, the streamed
/// path keeps opening and scrolling bounded. A frontend reads this through
/// [`Engine::eager_import_frame_limit`].
const EAGER_MAX_FRAMES: usize = 48_000 * 120;

/// The measurement readout for a time–frequency selection.
///
/// Geometry (`t0`/`t1`/`f0`/`f1`/`duration`) is the box in signal coordinates;
/// the remaining fields are engine queries over it, so a selection bar built
/// from this struct shows exactly what a script reading the same API would.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SelectionReadout {
    /// Selection start time in seconds.
    pub t0: f64,
    /// Selection end time in seconds.
    pub t1: f64,
    /// Selection low frequency in hertz.
    pub f0: f64,
    /// Selection high frequency in hertz.
    pub f1: f64,
    /// Selection duration in seconds.
    pub duration: f64,
    /// Mean voiced fundamental over the span, in hertz.
    pub f0_mean_hz: Option<f64>,
    /// Minimum voiced fundamental over the span, in hertz.
    pub f0_min_hz: Option<f64>,
    /// Maximum voiced fundamental over the span, in hertz.
    pub f0_max_hz: Option<f64>,
    /// Mean raw band energy inside the box, in decibels.
    pub band_energy_db: f64,
    /// Mean intensity over the span, in dB SPL, absent when the span is empty.
    pub intensity_mean_db: Option<f64>,
    /// Mean harmonics-to-noise ratio over the span, in decibels.
    pub hnr_mean_db: Option<f64>,
}

/// Session engine: the audio store plus the pure functions that read it.
///
/// Every method beyond store bookkeeping is stateless-by-arguments — the
/// same `(id, params)` pair always produces the same result, independent of
/// call order, viewport, or any other implicit state (rule 1,
/// `docs/plan/architecture.md`).
#[derive(Default)]
pub struct Engine {
    store: AudioStore,
    documents: DocumentStore,
    journal: Journal,
    recordings: RecordingStore,
    tiles: Mutex<TileCache>,
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

    /// Opens a WAV from a byte reader as a streamed source and returns its id.
    ///
    /// The header is parsed and the bounded waveform pyramid built in one
    /// streaming pass; the decoded signal is never held whole, so metadata is
    /// ready at header speed and the waveform scrolls without the full-decode
    /// footprint. This is the path for recordings past the eager comfort
    /// threshold ([`EAGER_MAX_FRAMES`]) — an hour-long take the desktop shell
    /// backs with a file handle or the web worker with an OPFS access handle.
    /// Whole-signal analysis of a streamed source still materializes it on
    /// demand ([`Engine::pitch_track`] and the other whole-signal contours);
    /// the streamed win is that opening and scrolling never pay that cost.
    ///
    /// `name` is the display name the metadata surface reports.
    ///
    /// # Errors
    /// Returns [`EngineError::Audio`] when the header is malformed, the sample
    /// format is unsupported, or the backing store fails during the pyramid
    /// pass.
    pub fn open_streaming_wav(
        &mut self,
        reader: impl ByteReader + Send + Sync + 'static,
        name: Option<String>,
    ) -> Result<AudioId, EngineError> {
        let source = Arc::new(StreamingWav::open(reader)?);
        let pyramid = StreamPyramid::build(&source)?;
        Ok(self.store.insert_streamed(source, pyramid, name))
    }

    /// Frame count at or below which an import stays eager (whole-signal
    /// decode). Two minutes of 48 kHz audio; longer takes belong on the
    /// streamed path so their decoded footprint and per-sample pyramid never
    /// enter memory. Frontends deciding between [`Engine::import_wav_bytes`]
    /// and [`Engine::open_streaming_wav`] read this bound from
    /// [`Engine::eager_import_frame_limit`].
    #[must_use]
    pub fn eager_import_frame_limit() -> usize {
        EAGER_MAX_FRAMES
    }

    /// Opens a streaming recording and returns its id.
    ///
    /// `sample_rate` is the true capture rate (the host reads it from the
    /// audio device, never assumes one) and `channels` its channel count.
    /// Sample chunks arrive through [`Engine::append_samples`]; the take stays
    /// out of the audio store until [`Engine::finish_recording`] materializes
    /// it, so nothing queries a half-captured buffer.
    ///
    /// # Errors
    /// Returns [`EngineError::InvalidRequest`] when `sample_rate` is not finite
    /// and positive, or when `channels` is zero.
    pub fn begin_recording(
        &mut self,
        sample_rate: f64,
        channels: usize,
    ) -> Result<RecordingId, EngineError> {
        self.recordings.begin(sample_rate, channels)
    }

    /// Appends one planar sample chunk to an open recording.
    ///
    /// `planar` carries every channel's samples for this chunk back to back,
    /// so its length must divide evenly by the take's channel count. Chunks
    /// accumulate in memory until the take is finished or aborted.
    ///
    /// # Errors
    /// Returns [`EngineError::UnknownRecordingId`] when `id` names no open
    /// take, and [`EngineError::InvalidRequest`] when `planar` does not divide
    /// evenly by the channel count.
    pub fn append_samples(&mut self, id: RecordingId, planar: &[f32]) -> Result<(), EngineError> {
        self.recordings.append(id, planar)
    }

    /// Finishes a recording, materializing it as a store entry and returning
    /// that id alongside the take encoded as WAV bytes.
    ///
    /// The store entry is the same kind of buffer an import produces, so every
    /// analysis reads a recorded take exactly as it reads an imported file. The
    /// WAV bytes (24-bit PCM, lossless for the captured `[-1, 1]` signal) let
    /// the host persist the take beside imported media through its own storage
    /// path. Finishing consumes the take; the id is invalid afterwards.
    ///
    /// # Errors
    /// Returns [`EngineError::UnknownRecordingId`] when `id` names no open
    /// take, and [`EngineError::Audio`] when the accumulated samples cannot
    /// form an audio buffer (an empty take, or one too large to allocate).
    pub fn finish_recording(
        &mut self,
        id: RecordingId,
        name: String,
    ) -> Result<FinishedRecording, EngineError> {
        let take = self.recordings.finish(id)?;
        let audio = Audio::new(take.channels, take.sample_rate)?.with_name(name);
        let wav = audio.to_wav_bytes(BitDepth::Pcm24)?;
        let audio = self.store.insert(audio);
        Ok(FinishedRecording { audio, wav })
    }

    /// Discards an open recording without materializing it.
    ///
    /// # Errors
    /// Returns [`EngineError::UnknownRecordingId`] when `id` names no open take.
    pub fn abort_recording(&mut self, id: RecordingId) -> Result<(), EngineError> {
        self.recordings.abort(id)
    }

    /// Returns duration, sample rate, channel count, and name for `id`.
    ///
    /// # Errors
    /// Returns [`EngineError::UnknownAudioId`] when `id` does not name a
    /// live store entry.
    pub fn audio_info(&self, id: AudioId) -> Result<AudioInfo, EngineError> {
        self.store.info(id)
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
        self.store.waveform(id, t0, t1, px)
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
        let tile_db = self.spectrogram_tile_db(id, req)?;

        let expected_len = req.width_px as usize * req.height_px as usize;
        if tile_db.len() != expected_len {
            return Err(EngineError::InvalidRequest {
                reason: format!(
                    "tile produced {} values for a {}x{} request; the audio is likely too \
                     short, or the time/frequency range too narrow, to fit a single analysis \
                     frame",
                    tile_db.len(),
                    req.width_px,
                    req.height_px
                ),
            });
        }

        Ok(phx_render::colorize(
            &tile_db,
            req.width_px,
            req.height_px,
            display,
            colormap,
            theme,
        ))
    }

    /// Assembles the raw dB values for a tile from the block cache, in the
    /// row-major, lowest-frequency-first order [`phx_render::colorize`] expects.
    ///
    /// The frame columns the request selects are grouped into fixed
    /// [`tile_cache`] blocks aligned to the object-level frame grid; each block's
    /// STFT is computed once and reused, so a colormap, theme, or dynamic-range
    /// change re-colorizes cached dB without recomputing the transform. The
    /// values are bit-for-bit identical to a direct `compute_tile`, since both
    /// read the same frame centres off the same grid.
    fn spectrogram_tile_db(&self, id: AudioId, req: &TileRequest) -> Result<Vec<f32>, EngineError> {
        let access = self.store.whole(id)?;
        let audio = access.audio();
        let view = audio.slice_samples(0..audio.frames());
        let axes = analysis_axes(view.clone(), &req.params);
        let freq_len = axes.frequencies.len();

        let time_indices = select_axis_indices(
            &axes.times,
            req.t0.min(req.t1),
            req.t0.max(req.t1),
            req.width_px as usize,
        );
        let freq_indices = select_axis_indices(
            &axes.frequencies,
            req.f0.min(req.f1),
            req.f0.max(req.f1),
            req.height_px as usize,
        );
        if time_indices.is_empty() || freq_indices.is_empty() {
            return Ok(Vec::new());
        }

        let hash = params_hash(&req.params);
        let mut needed: Vec<usize> = time_indices.iter().map(|&t| t / TILE_COLS).collect();
        needed.sort_unstable();
        needed.dedup();

        let mut blocks: std::collections::HashMap<usize, phx_spectrogram::ColumnBlock> =
            std::collections::HashMap::with_capacity(needed.len());
        for &block_index in &needed {
            let key = BlockKey {
                audio: id.as_u64(),
                params_hash: hash,
                block_index,
            };
            let hit = self.tiles.lock().expect("tile cache poisoned").get(key);
            let block = match hit {
                Some(block) => block,
                None => {
                    let block = compute_column_block(
                        view.clone(),
                        &req.params,
                        block_index * TILE_COLS,
                        TILE_COLS,
                    );
                    self.tiles
                        .lock()
                        .expect("tile cache poisoned")
                        .insert(key, block.clone());
                    block
                }
            };
            blocks.insert(block_index, block);
        }

        let mut db = Vec::with_capacity(freq_indices.len() * time_indices.len());
        for &f in &freq_indices {
            for &t in &time_indices {
                let block = &blocks[&(t / TILE_COLS)];
                let local = t - block.first_col;
                db.push(block.db[local * freq_len + f]);
            }
        }
        Ok(db)
    }

    /// Number of raw dB spectrogram blocks currently held in the tile cache.
    ///
    /// Exposed for the frontend perf probe: a colormap or theme change must
    /// leave this count unchanged, since it re-colorizes cached dB rather than
    /// recomputing the STFT.
    #[must_use]
    pub fn spectrogram_cached_block_count(&self) -> usize {
        self.tiles.lock().expect("tile cache poisoned").len()
    }

    /// Computes the autocorrelation pitch track of `id` over its whole signal.
    ///
    /// The track sits on a frame grid derived from the audio duration alone,
    /// so a value queried at time *t* is the same at any zoom or scroll
    /// (rule 2, `docs/plan/architecture.md`). `phx_pitch::pitch_track` returns
    /// an empty track for parameters it cannot analyse rather than panicking,
    /// so this method never surfaces a parameter error of its own.
    ///
    /// # Errors
    /// Returns [`EngineError::UnknownAudioId`] when `id` does not name a live
    /// store entry.
    pub fn pitch_track(
        &self,
        id: AudioId,
        params: &PitchParams,
    ) -> Result<PitchTrack, EngineError> {
        let access = self.store.whole(id)?;
        let audio = access.audio();
        let view = audio.slice_samples(0..audio.frames());
        Ok(phx_pitch::pitch_track(view, params))
    }

    /// Computes pitch over just the samples spanning `[t0, t1)` seconds,
    /// returning the track together with the absolute start time of the
    /// analysed slice.
    ///
    /// This is the fast preview a live parameter edit renders first: pitch is
    /// the one contour whose whole-signal cost grows with duration, so the
    /// visible window is analysed on its own before the full-signal
    /// [`Engine::pitch_track`] result (the authoritative, zoom-independent one)
    /// replaces it. Frame times are relative to the slice; add the returned
    /// start time to place them on the absolute timeline. Because the Viterbi
    /// path here sees only the windowed frames, the preview can differ from the
    /// full track near the window edges — the whole-signal result is the one
    /// callers keep.
    ///
    /// # Errors
    /// Returns [`EngineError::UnknownAudioId`] when `id` does not name a live
    /// store entry, and [`EngineError::InvalidRequest`] when `t0`/`t1` are not
    /// finite.
    pub fn pitch_track_span(
        &self,
        id: AudioId,
        params: &PitchParams,
        t0: f64,
        t1: f64,
    ) -> Result<(PitchTrack, f64), EngineError> {
        if !t0.is_finite() || !t1.is_finite() {
            return Err(EngineError::InvalidRequest {
                reason: "pitch_track_span t0/t1 must be finite".to_string(),
            });
        }
        let info = self.store.info(id)?;
        let sample_rate = info.sample_rate;
        let frames = (info.duration * sample_rate).round() as usize;
        let lo = t0.min(t1).clamp(0.0, info.duration);
        let hi = t0.max(t1).clamp(0.0, info.duration);
        let start = ((lo * sample_rate).floor() as usize).min(frames);
        let end = ((hi * sample_rate).ceil() as usize).clamp(start, frames);
        // The span is a viewport window; decode only its samples so a streamed
        // source never materializes the whole signal for a preview.
        let window = self.store.range_owned(id, start, end)?;
        let track = phx_pitch::pitch_track(window.slice_samples(0..window.frames()), params);
        Ok((track, start as f64 / sample_rate))
    }

    /// Computes the raw Burg formant candidates of `id` over its whole signal.
    ///
    /// These are the frequency-gated LPC roots per frame, before any tracking
    /// reassigns them to formant slots — the display default while the
    /// tracking weights remain provisional (`docs/plan/tasks/phase-4.md`).
    /// Call [`Engine::formant_track_smoothed`] for the Viterbi-tracked view.
    ///
    /// # Errors
    /// Returns [`EngineError::UnknownAudioId`] when `id` does not name a live
    /// store entry, and [`EngineError::InvalidRequest`] when a formant
    /// parameter is outside the range `phx_formant` accepts (its analysis
    /// entry point asserts these, so the engine boundary checks them first).
    pub fn formant_track(
        &self,
        id: AudioId,
        params: &FormantParams,
    ) -> Result<FormantTrack, EngineError> {
        validate_formant_params(params)?;
        let access = self.store.whole(id)?;
        let audio = access.audio();
        let view = audio.slice_samples(0..audio.frames());
        Ok(phx_formant::formant_track(view, params))
    }

    /// Computes Xia–Espy-Wilson smoothed formants of `id` over its whole
    /// signal, using the crate's default neutral references and cost weights.
    ///
    /// Those weights are documented as provisional
    /// (`docs/plan/tasks/phase-4.md`); the UI surfaces this track only behind
    /// an explicit toggle and marks it as such.
    ///
    /// # Errors
    /// Returns [`EngineError::UnknownAudioId`] when `id` does not name a live
    /// store entry, and [`EngineError::InvalidRequest`] when a formant
    /// parameter is outside the range `phx_formant` accepts.
    pub fn formant_track_smoothed(
        &self,
        id: AudioId,
        params: &FormantParams,
    ) -> Result<FormantTrack, EngineError> {
        let raw = self.formant_track(id, params)?;
        Ok(phx_formant::track_smoothed(
            &raw,
            &phx_formant::TrackingRefs::default(),
        ))
    }

    /// Computes the intensity contour of `id` over its whole signal.
    ///
    /// The contour sits on a frame grid derived from the audio duration
    /// alone (rule 2, `docs/plan/architecture.md`).
    ///
    /// # Errors
    /// Returns [`EngineError::UnknownAudioId`] when `id` does not name a live
    /// store entry, and [`EngineError::InvalidRequest`] when the pitch floor
    /// is not finite and positive.
    pub fn intensity_track(
        &self,
        id: AudioId,
        params: &IntensityParams,
    ) -> Result<IntensityTrack, EngineError> {
        if !(params.pitch_floor_hz.is_finite() && params.pitch_floor_hz > 0.0) {
            return Err(EngineError::InvalidRequest {
                reason: "intensity pitch_floor_hz must be finite and positive".to_string(),
            });
        }
        let access = self.store.whole(id)?;
        let audio = access.audio();
        let view = audio.slice_samples(0..audio.frames());
        Ok(phx_intensity::intensity_track(view, params))
    }

    /// Returns the mean raw power spectral density inside a time–frequency box,
    /// in decibels.
    ///
    /// The box is the spectrogram selection: `[t0, t1]` seconds by `[f0, f1]`
    /// hertz, each pair accepted in either order and clamped to the signal. The
    /// value is the analysis grid's raw PSD (no display pre-emphasis), averaged
    /// as linear power over every snapped cell that falls inside the box, then
    /// converted back to decibels — a function of the signal and the box alone,
    /// so the readout equals this query at identical coordinates (the
    /// batch-equals-GUI invariant, `docs/plan/tasks/phase-4.md` T4.4).
    ///
    /// Returns `f64::NEG_INFINITY` for an empty box (no analysis cell falls
    /// inside it).
    ///
    /// # Errors
    /// Returns [`EngineError::UnknownAudioId`] when `id` does not name a live
    /// store entry, and [`EngineError::InvalidRequest`] when a bound is not
    /// finite.
    pub fn band_energy(
        &self,
        id: AudioId,
        t0: f64,
        t1: f64,
        f0: f64,
        f1: f64,
    ) -> Result<f64, EngineError> {
        if ![t0, t1, f0, f1].iter().all(|value| value.is_finite()) {
            return Err(EngineError::InvalidRequest {
                reason: "band_energy bounds must be finite".to_string(),
            });
        }
        let access = self.store.whole(id)?;
        let audio = access.audio();
        let duration = audio.duration();
        let (lo, hi) = ordered_clamped(t0, t1, 0.0, duration);
        let (flo, fhi) = ordered_clamped(f0, f1, 0.0, f64::INFINITY);
        if hi - lo <= 0.0 || fhi - flo <= 0.0 {
            return Ok(f64::NEG_INFINITY);
        }
        let params = SpectrogramParams {
            max_frequency: fhi.max(SpectrogramParams::default().max_frequency),
            ..SpectrogramParams::default()
        };
        // Match the tile resolution to the analysis grid so every snapped
        // frame and frequency bin inside the box contributes about once.
        let time_step = phx_spectrogram::effective_time_step(&params);
        let frequency_step = phx_spectrogram::effective_frequency_step(&params);
        let width = (((hi - lo) / time_step).ceil() as u32).clamp(1, 4096);
        let height = (((fhi - flo) / frequency_step).ceil() as u32).clamp(1, 4096);
        let req = TileRequest {
            t0: lo,
            t1: hi,
            f0: flo,
            f1: fhi,
            width_px: width,
            height_px: height,
            params,
        };
        let view = audio.slice_samples(0..audio.frames());
        let tile = phx_spectrogram::compute_tile(view, &req);
        if tile.db.is_empty() {
            return Ok(f64::NEG_INFINITY);
        }
        let mut sum = 0.0;
        for &db in &tile.db {
            sum += 10.0_f64.powf(f64::from(db) / 10.0);
        }
        Ok(10.0 * (sum / tile.db.len() as f64).log10())
    }

    /// Computes the measurement readout for a selection: its geometry plus the
    /// span statistics the selection bar shows.
    ///
    /// Every number is an engine query over the selection, so the bar displays
    /// exactly what a script reading this API would get for the same box (the
    /// batch-equals-GUI invariant). Pitch statistics cover voiced frames inside
    /// the span; band energy comes from [`Engine::band_energy`]; mean intensity
    /// and mean HNR are frame means over the span, absent when the span holds no
    /// frame.
    ///
    /// # Errors
    /// Returns [`EngineError::UnknownAudioId`] when `id` does not name a live
    /// store entry, and [`EngineError::InvalidRequest`] when a bound is not
    /// finite.
    #[allow(clippy::too_many_arguments)]
    pub fn selection_readout(
        &self,
        id: AudioId,
        t0: f64,
        t1: f64,
        f0: f64,
        f1: f64,
        pitch_floor_hz: f64,
        pitch_ceiling_hz: f64,
        intensity_floor_hz: f64,
    ) -> Result<SelectionReadout, EngineError> {
        if ![t0, t1, f0, f1].iter().all(|value| value.is_finite()) {
            return Err(EngineError::InvalidRequest {
                reason: "selection_readout bounds must be finite".to_string(),
            });
        }
        let access = self.store.whole(id)?;
        let audio = access.audio();
        let duration = audio.duration();
        let (lo, hi) = ordered_clamped(t0, t1, 0.0, duration);
        let (flo, fhi) = ordered_clamped(f0, f1, 0.0, f64::INFINITY);
        let span = TimeSpan::new(lo, hi);
        let view = audio.slice_samples(0..audio.frames());

        let pitch_params = PitchParams {
            floor_hz: pitch_floor_hz,
            ceiling_hz: pitch_ceiling_hz,
            ..PitchParams::default()
        };
        let pitch = phx_pitch::pitch_track(view.clone(), &pitch_params);

        let intensity_params = IntensityParams {
            pitch_floor_hz: intensity_floor_hz,
            ..IntensityParams::default()
        };
        let intensity = phx_intensity::intensity_track(view.clone(), &intensity_params);
        let intensity_mean_db = mean_in_span(intensity.iter(), span);

        let harmonicity_params = HarmonicityParams {
            floor_hz: pitch_floor_hz,
            ..HarmonicityParams::default()
        };
        let hnr = phx_voice::hnr_track(view, &harmonicity_params);
        let hnr_mean_db = hnr.mean_db(span);

        Ok(SelectionReadout {
            t0: lo,
            t1: hi,
            f0: flo,
            f1: fhi,
            duration: hi - lo,
            f0_mean_hz: pitch.mean_hz(span),
            f0_min_hz: pitch.min_hz(span),
            f0_max_hz: pitch.max_hz(span),
            band_energy_db: self.band_energy(id, lo, hi, flo, fhi)?,
            intensity_mean_db,
            hnr_mean_db,
        })
    }

    /// Returns the mean frequency of each formant slot over a time span, in
    /// hertz.
    ///
    /// Slot `j` is the `j`-th lowest candidate of each frame; its mean is taken
    /// over the frames inside `[t0, t1]` that carry that slot, or `None` when no
    /// frame does. These are the provisional tracked-formant means the readout
    /// marks while the tracking weights stay unvalidated
    /// (`docs/plan/tasks/phase-4.md`).
    ///
    /// # Errors
    /// Returns [`EngineError::UnknownAudioId`] when `id` does not name a live
    /// store entry, and [`EngineError::InvalidRequest`] when a formant parameter
    /// is outside the range the analysis accepts, or a bound is not finite.
    pub fn formant_span_means(
        &self,
        id: AudioId,
        params: &FormantParams,
        smoothed: bool,
        t0: f64,
        t1: f64,
    ) -> Result<Vec<Option<f64>>, EngineError> {
        if !t0.is_finite() || !t1.is_finite() {
            return Err(EngineError::InvalidRequest {
                reason: "formant_span_means t0/t1 must be finite".to_string(),
            });
        }
        let track = if smoothed {
            self.formant_track_smoothed(id, params)?
        } else {
            self.formant_track(id, params)?
        };
        let (lo, hi) = (t0.min(t1), t0.max(t1));
        let mut sums = vec![0.0; params.max_formants];
        let mut counts = vec![0usize; params.max_formants];
        for frame in &track.frames {
            if frame.time < lo || frame.time > hi {
                continue;
            }
            for (slot, formant) in frame.formants.iter().enumerate().take(params.max_formants) {
                sums[slot] += formant.frequency;
                counts[slot] += 1;
            }
        }
        Ok(sums
            .into_iter()
            .zip(counts)
            .map(|(sum, count)| (count > 0).then(|| sum / count as f64))
            .collect())
    }

    /// Computes power-weighted spectral moments at the midpoint of a span.
    ///
    /// The slice is the raw spectrogram frame nearest `(t0 + t1) / 2`, its dB
    /// PSD converted to linear power before weighting. `power` is the moment
    /// weighting exponent (`2.0` weights by power, Praat's default).
    ///
    /// # Errors
    /// Returns [`EngineError::UnknownAudioId`] when `id` does not name a live
    /// store entry, and [`EngineError::InvalidRequest`] when a bound is not
    /// finite.
    pub fn spectral_moments_in_span(
        &self,
        id: AudioId,
        t0: f64,
        t1: f64,
        power: f64,
    ) -> Result<Moments, EngineError> {
        if !t0.is_finite() || !t1.is_finite() {
            return Err(EngineError::InvalidRequest {
                reason: "spectral_moments_in_span t0/t1 must be finite".to_string(),
            });
        }
        let access = self.store.whole(id)?;
        let audio = access.audio();
        let midpoint = 0.5 * (t0.min(t1) + t0.max(t1));
        let view = audio.slice_samples(0..audio.frames());
        let slice = phx_spectrogram::spectral_slice(view, midpoint, &SpectrogramParams::default());
        let values = slice
            .db
            .iter()
            .map(|&db| 10.0_f64.powf(f64::from(db) / 10.0))
            .collect();
        let spectrum = phx_voice::SpectrumSlice {
            frequencies_hz: slice.f_axis,
            values,
        };
        Ok(phx_voice::spectral_moments(&spectrum, power))
    }

    /// Computes the aggregate voice report over a selection span.
    ///
    /// Wraps [`phx_voice::voice_report`]: it tracks pitch, extracts pulses, and
    /// aggregates the jitter, shimmer, HNR, CPP, and voice-break measures over
    /// `[t0, t1]`, embedding the parameters used. The pitch floor and ceiling
    /// come from the selection's analysis parameters.
    ///
    /// # Errors
    /// Returns [`EngineError::UnknownAudioId`] when `id` does not name a live
    /// store entry, and [`EngineError::InvalidRequest`] when a bound is not
    /// finite.
    pub fn voice_report(
        &self,
        id: AudioId,
        t0: f64,
        t1: f64,
        pitch_floor_hz: f64,
        pitch_ceiling_hz: f64,
    ) -> Result<VoiceReport, EngineError> {
        if !t0.is_finite() || !t1.is_finite() {
            return Err(EngineError::InvalidRequest {
                reason: "voice_report t0/t1 must be finite".to_string(),
            });
        }
        let access = self.store.whole(id)?;
        let audio = access.audio();
        let duration = audio.duration();
        let (lo, hi) = ordered_clamped(t0, t1, 0.0, duration);
        let view = audio.slice_samples(0..audio.frames());
        let pitch_params = PitchParams {
            floor_hz: pitch_floor_hz,
            ceiling_hz: pitch_ceiling_hz,
            ..PitchParams::default()
        };
        Ok(phx_voice::voice_report(
            view,
            TimeSpan::new(lo, hi),
            &pitch_params,
        ))
    }

    /// Applies one command through the journal and reports what changed.
    ///
    /// This is the only path that mutates a document: it runs the command,
    /// records an id-stable inverse for [`Engine::undo`], captures the id-stable
    /// redo, and clears the redo stack so the new command cannot be contradicted
    /// by pending redo history. On any error the state is left untouched — every
    /// underlying mutator commits only a fully validated result.
    ///
    /// # Errors
    /// Returns [`EngineError::Audio`] for an undecodable import,
    /// [`EngineError::UnknownAudioId`] / [`EngineError::UnknownAnnotationId`]
    /// for a missing target, [`EngineError::InvalidAnnotation`] for an attached
    /// document that fails validation, and [`EngineError::Annotation`] for a
    /// rejected annotation mutation (an out-of-range boundary, a control
    /// character in a label, a dangling relation left by a tier removal).
    pub fn apply(&mut self, cmd: Command) -> Result<Applied, EngineError> {
        let (applied, entry) = self.execute(cmd)?;
        self.journal.record(entry);
        Ok(applied)
    }

    /// Undoes the most recent command, restoring a state-hash-identical
    /// document, and reports what changed. Returns `None` when nothing is left
    /// to undo.
    ///
    /// # Errors
    /// Returns [`EngineError::UnknownAnnotationId`] only if a document a stored
    /// inverse names has gone missing, which the journal's own bookkeeping
    /// prevents in practice.
    pub fn undo(&mut self) -> Result<Option<Applied>, EngineError> {
        let Some(entry) = self.journal.take_undo() else {
            return Ok(None);
        };
        match entry.undo.apply(&mut self.store, &mut self.documents) {
            Ok(applied) => {
                self.journal.park_redo(entry);
                Ok(Some(applied))
            }
            Err(err) => {
                self.journal.park_undo(entry);
                Err(err)
            }
        }
    }

    /// Redoes the most recently undone command and reports what changed.
    /// Returns `None` when nothing is left to redo.
    ///
    /// # Errors
    /// Returns [`EngineError::UnknownAnnotationId`] only if a document a stored
    /// transition names has gone missing, which the journal's own bookkeeping
    /// prevents in practice.
    pub fn redo(&mut self) -> Result<Option<Applied>, EngineError> {
        let Some(entry) = self.journal.take_redo() else {
            return Ok(None);
        };
        match entry.redo.apply(&mut self.store, &mut self.documents) {
            Ok(applied) => {
                self.journal.park_undo(entry);
                Ok(Some(applied))
            }
            Err(err) => {
                self.journal.park_redo(entry);
                Err(err)
            }
        }
    }

    /// Number of commands that can still be undone.
    #[must_use]
    pub fn undo_depth(&self) -> usize {
        self.journal.undo_depth()
    }

    /// Number of commands that can still be redone.
    #[must_use]
    pub fn redo_depth(&self) -> usize {
        self.journal.redo_depth()
    }

    /// Returns a hash of the whole document model — every stored audio buffer's
    /// identity and every annotation document's content.
    ///
    /// Two engines whose document models are equal produce the same value, and
    /// undoing a command restores the value it had before (invariant 5,
    /// `docs/plan/validation.md`). The fold visits ids in ascending order so the
    /// result never depends on hash-map iteration order. The value is stable
    /// within a process run, which is all a consistency assertion needs; it is
    /// not a persisted content address.
    #[must_use]
    pub fn state_hash(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        let audio_ids = self.store.ids_sorted();
        (audio_ids.len() as u64).hash(&mut hasher);
        for id in audio_ids {
            id.as_u64().hash(&mut hasher);
            if let Ok(audio) = self.store.audio(id) {
                (audio.frames() as u64).hash(&mut hasher);
                audio.sample_rate().to_bits().hash(&mut hasher);
                (audio.channel_count() as u64).hash(&mut hasher);
                audio.name().hash(&mut hasher);
            }
        }
        let doc_ids = self.documents.ids_sorted();
        (doc_ids.len() as u64).hash(&mut hasher);
        for id in doc_ids {
            id.as_u64().hash(&mut hasher);
            if let Ok(document) = self.documents.get(id) {
                document.audio.as_u64().hash(&mut hasher);
                hash_annotation(&document.annotation, &mut hasher);
            }
        }
        hasher.finish()
    }

    /// Searches interval and point labels across every attached document.
    ///
    /// Each hit is tagged with the document it was found in, so a cross-project
    /// search can navigate to the right document and then to the span within it.
    /// Documents are visited in ascending id order.
    #[must_use]
    pub fn search_labels(&self, query: &LabelQuery) -> Vec<EngineHit> {
        let mut hits = Vec::new();
        for id in self.documents.ids_sorted() {
            let Ok(document) = self.documents.get(id) else {
                continue;
            };
            for hit in document.annotation.search(query) {
                hits.push(EngineHit {
                    annotation: id,
                    hit,
                });
            }
        }
        hits
    }

    /// Returns the annotation content of a document for read-only rendering.
    ///
    /// # Errors
    /// Returns [`EngineError::UnknownAnnotationId`] when `id` names no live
    /// document.
    pub fn annotation(&self, id: AnnotationId) -> Result<&Annotation, EngineError> {
        Ok(&self.documents.get(id)?.annotation)
    }

    /// Returns the audio a document annotates.
    ///
    /// # Errors
    /// Returns [`EngineError::UnknownAnnotationId`] when `id` names no live
    /// document.
    pub fn annotation_audio(&self, id: AnnotationId) -> Result<AudioId, EngineError> {
        Ok(self.documents.get(id)?.audio)
    }

    /// Returns every live annotation id in ascending order.
    #[must_use]
    pub fn annotation_ids(&self) -> Vec<AnnotationId> {
        self.documents.ids_sorted()
    }

    /// Runs a command forward against live state, returning the report and the
    /// journal entry that reverses and reproduces it.
    fn execute(&mut self, cmd: Command) -> Result<(Applied, JournalEntry), EngineError> {
        use phx_annot::InverseMutation;

        match cmd {
            Command::ImportAudio { bytes, name } => {
                let audio = Audio::from_wav_bytes(&bytes)?.with_name(name);
                let replay = audio.clone();
                let id = self.store.insert(audio);
                Ok((
                    Applied::AudioImported { audio: id },
                    JournalEntry {
                        undo: Reverse::RemoveAudio { id },
                        redo: Reverse::ImportAudio {
                            id,
                            audio: Box::new(replay),
                        },
                    },
                ))
            }
            Command::AttachAnnotation { audio, annotation } => {
                if !self.store.contains(audio) {
                    return Err(EngineError::UnknownAudioId(audio));
                }
                let issues = annotation.validate();
                if !issues.is_empty() {
                    return Err(EngineError::InvalidAnnotation(issues));
                }
                let document = Document {
                    audio,
                    annotation: annotation.clone(),
                };
                let id = self.documents.attach(audio, annotation);
                Ok((
                    Applied::AnnotationAttached {
                        annotation: id,
                        audio,
                    },
                    JournalEntry {
                        undo: Reverse::Detach { id },
                        redo: Reverse::Attach {
                            id,
                            document: Box::new(document),
                        },
                    },
                ))
            }
            Command::AddIntervalTier {
                annotation,
                name,
                relation,
            } => {
                let document = self.documents.get_mut(annotation)?;
                let tier = document.annotation.add_interval_tier(&name, relation)?;
                let (index, slot) = captured_tier(&document.annotation, tier)?;
                Ok((
                    Applied::TierAdded { annotation, tier },
                    JournalEntry {
                        undo: Reverse::RemoveTier {
                            doc: annotation,
                            tier,
                        },
                        redo: Reverse::InsertTier {
                            doc: annotation,
                            index,
                            slot: Box::new(slot),
                        },
                    },
                ))
            }
            Command::AddPointTier {
                annotation,
                name,
                points,
                relation,
            } => {
                let document = self.documents.get_mut(annotation)?;
                let tier = document
                    .annotation
                    .add_point_tier(&name, points, relation)?;
                let (index, slot) = captured_tier(&document.annotation, tier)?;
                Ok((
                    Applied::TierAdded { annotation, tier },
                    JournalEntry {
                        undo: Reverse::RemoveTier {
                            doc: annotation,
                            tier,
                        },
                        redo: Reverse::InsertTier {
                            doc: annotation,
                            index,
                            slot: Box::new(slot),
                        },
                    },
                ))
            }
            Command::RemoveTier { annotation, tier } => {
                let document = self.documents.get_mut(annotation)?;
                let (index, slot) = captured_tier(&document.annotation, tier)?;
                let reduced = journal::remove_tier(&document.annotation, tier)?;
                document.annotation = reduced;
                Ok((
                    Applied::TierRemoved { annotation, tier },
                    JournalEntry {
                        undo: Reverse::InsertTier {
                            doc: annotation,
                            index,
                            slot: Box::new(slot),
                        },
                        redo: Reverse::RemoveTier {
                            doc: annotation,
                            tier,
                        },
                    },
                ))
            }
            Command::InsertBoundary {
                annotation,
                tier,
                at,
            } => {
                let document = self.documents.get_mut(annotation)?;
                let boundary = document.annotation.insert_boundary(tier, at)?;
                // Capture the split as a restore-merge so redo re-creates the
                // same boundary id rather than allocating a fresh one.
                let mut probe = document.annotation.clone();
                let merged = probe.remove_boundary(boundary)?;
                Ok((
                    Applied::BoundaryInserted {
                        annotation,
                        tier,
                        boundary,
                        at,
                    },
                    JournalEntry {
                        undo: Reverse::Content {
                            doc: annotation,
                            mutation: InverseMutation::RemoveBoundary { boundary },
                        },
                        redo: Reverse::Content {
                            doc: annotation,
                            mutation: InverseMutation::RestoreMergedBoundary { merged },
                        },
                    },
                ))
            }
            Command::MoveBoundary {
                annotation,
                boundary,
                to,
                mode,
            } => {
                let document = self.documents.get_mut(annotation)?;
                let moved = document.annotation.move_boundary(boundary, to, mode)?;
                let redo_moves = moved
                    .moves
                    .iter()
                    .map(|m| BoundaryMove {
                        tier: m.tier,
                        boundary: m.boundary,
                        from: m.to,
                        to: m.from,
                    })
                    .collect();
                Ok((
                    Applied::BoundaryMoved {
                        annotation,
                        moves: moved.moves.clone(),
                    },
                    JournalEntry {
                        undo: Reverse::Content {
                            doc: annotation,
                            mutation: InverseMutation::MoveBoundaries { moves: moved.moves },
                        },
                        redo: Reverse::Content {
                            doc: annotation,
                            mutation: InverseMutation::MoveBoundaries { moves: redo_moves },
                        },
                    },
                ))
            }
            Command::RemoveBoundary {
                annotation,
                boundary,
            } => {
                let document = self.documents.get_mut(annotation)?;
                let merged = document.annotation.remove_boundary(boundary)?;
                Ok((
                    Applied::BoundaryRemoved {
                        annotation,
                        boundary,
                    },
                    JournalEntry {
                        undo: Reverse::Content {
                            doc: annotation,
                            mutation: InverseMutation::RestoreMergedBoundary { merged },
                        },
                        redo: Reverse::Content {
                            doc: annotation,
                            mutation: InverseMutation::RemoveBoundary { boundary },
                        },
                    },
                ))
            }
            Command::SetLabel {
                annotation,
                target,
                text,
            } => {
                let document = self.documents.get_mut(annotation)?;
                let change = document.annotation.set_label(target, &text)?;
                Ok((
                    Applied::LabelSet {
                        annotation,
                        target,
                        text: change.new_text.clone(),
                    },
                    JournalEntry {
                        undo: Reverse::Content {
                            doc: annotation,
                            mutation: InverseMutation::SetLabel {
                                target,
                                text: change.old_text,
                            },
                        },
                        redo: Reverse::Content {
                            doc: annotation,
                            mutation: InverseMutation::SetLabel {
                                target,
                                text: change.new_text,
                            },
                        },
                    },
                ))
            }
            Command::InsertPoint {
                annotation,
                tier,
                time,
                label,
            } => {
                let document = self.documents.get_mut(annotation)?;
                let insertion = document.annotation.insert_point(tier, time, &label)?;
                let point = insertion.point.clone();
                Ok((
                    Applied::PointInserted {
                        annotation,
                        tier,
                        point: point.id,
                        at: point.time,
                    },
                    JournalEntry {
                        undo: Reverse::Content {
                            doc: annotation,
                            mutation: InverseMutation::RemovePoint { point: point.id },
                        },
                        // Redo restores the exact point id rather than allocating
                        // a fresh one, keeping undo/redo state-hash-identical.
                        redo: Reverse::Content {
                            doc: annotation,
                            mutation: InverseMutation::RestorePoint { tier, point },
                        },
                    },
                ))
            }
            Command::MovePoint {
                annotation,
                point,
                to,
            } => {
                let document = self.documents.get_mut(annotation)?;
                let moved = document.annotation.move_point(point, to)?;
                Ok((
                    Applied::PointMoved {
                        annotation,
                        point,
                        to: moved.to,
                    },
                    JournalEntry {
                        undo: Reverse::Content {
                            doc: annotation,
                            mutation: InverseMutation::MovePoint {
                                point,
                                to: moved.from,
                            },
                        },
                        redo: Reverse::Content {
                            doc: annotation,
                            mutation: InverseMutation::MovePoint {
                                point,
                                to: moved.to,
                            },
                        },
                    },
                ))
            }
            Command::RemovePoint { annotation, point } => {
                let document = self.documents.get_mut(annotation)?;
                let removal = document.annotation.remove_point(point)?;
                Ok((
                    Applied::PointRemoved { annotation, point },
                    JournalEntry {
                        undo: Reverse::Content {
                            doc: annotation,
                            mutation: InverseMutation::RestorePoint {
                                tier: removal.tier,
                                point: removal.point,
                            },
                        },
                        redo: Reverse::Content {
                            doc: annotation,
                            mutation: InverseMutation::RemovePoint { point },
                        },
                    },
                ))
            }
            Command::ReorderTier {
                annotation,
                tier,
                to_index,
            } => {
                let document = self.documents.get_mut(annotation)?;
                let reorder = document.annotation.reorder_tier(tier, to_index)?;
                Ok((
                    Applied::TierReordered {
                        annotation,
                        tier,
                        to_index: reorder.to_index,
                    },
                    JournalEntry {
                        undo: Reverse::Content {
                            doc: annotation,
                            mutation: InverseMutation::ReorderTier {
                                tier,
                                to_index: reorder.from_index,
                            },
                        },
                        redo: Reverse::Content {
                            doc: annotation,
                            mutation: InverseMutation::ReorderTier {
                                tier,
                                to_index: reorder.to_index,
                            },
                        },
                    },
                ))
            }
        }
    }
}

/// Captures a tier's document position and full slot for the journal, so undo
/// or redo can reinstate it with every stable id intact.
fn captured_tier(annotation: &Annotation, tier: TierId) -> Result<(usize, TierSlot), EngineError> {
    let index = journal::tier_index(annotation, tier).ok_or(EngineError::Annotation(
        AnnotationError::UnknownTier { tier },
    ))?;
    let slot = annotation.tiers()[index].clone();
    Ok((index, slot))
}

/// Folds an annotation's content into `hasher` in document order.
///
/// Every field that distinguishes two documents contributes: the time domain,
/// tier order, relations, and each interval's or point's stable ids, times, and
/// label. Floats are hashed by bit pattern so the fold matches
/// [`phx_annot::Annotation`]'s own bitwise equality.
fn hash_annotation<H: Hasher>(annotation: &Annotation, hasher: &mut H) {
    annotation.xmin().to_bits().hash(hasher);
    annotation.xmax().to_bits().hash(hasher);
    (annotation.tiers().len() as u64).hash(hasher);
    for slot in annotation.tiers() {
        slot.id.get().hash(hasher);
        match slot.relation {
            TierRelation::Independent => 0u8.hash(hasher),
            TierRelation::AlignedBoundaries { with } => {
                1u8.hash(hasher);
                with.get().hash(hasher);
            }
            TierRelation::ChildOf { parent } => {
                2u8.hash(hasher);
                parent.get().hash(hasher);
            }
        }
        match &slot.tier {
            Tier::Interval(tier) => {
                0u8.hash(hasher);
                tier.name.hash(hasher);
                (tier.intervals.len() as u64).hash(hasher);
                for interval in &tier.intervals {
                    interval.id.get().hash(hasher);
                    interval.start_boundary.get().hash(hasher);
                    interval.end_boundary.get().hash(hasher);
                    interval.xmin.to_bits().hash(hasher);
                    interval.xmax.to_bits().hash(hasher);
                    interval.label.hash(hasher);
                }
            }
            Tier::Point(tier) => {
                1u8.hash(hasher);
                tier.name.hash(hasher);
                (tier.points.len() as u64).hash(hasher);
                for point in &tier.points {
                    point.id.get().hash(hasher);
                    point.time.to_bits().hash(hasher);
                    point.label.hash(hasher);
                }
            }
        }
    }
}

/// Orders a pair and clamps it to `[min, max]`, returning `(low, high)`.
fn ordered_clamped(a: f64, b: f64, min: f64, max: f64) -> (f64, f64) {
    let lo = a.min(b).clamp(min, max);
    let hi = a.max(b).clamp(min, max);
    (lo, hi)
}

/// Mean of the values whose time falls inside `span`, or `None` when none do.
fn mean_in_span(frames: impl Iterator<Item = (f64, f64)>, span: TimeSpan) -> Option<f64> {
    let mut sum = 0.0;
    let mut count = 0usize;
    for (time, value) in frames {
        if span.contains(time) {
            sum += value;
            count += 1;
        }
    }
    (count > 0).then(|| sum / count as f64)
}

/// Validates a [`FormantParams`] before it reaches `phx_formant`.
///
/// `phx_formant::formant_track` asserts these same properties and panics on
/// violation. The engine is the boundary untrusted callers reach, so it
/// re-checks them here and turns a would-be panic into a typed error.
fn validate_formant_params(params: &FormantParams) -> Result<(), EngineError> {
    let invalid = |reason: &str| {
        Err(EngineError::InvalidRequest {
            reason: reason.to_string(),
        })
    };
    if !(params.ceiling_hz.is_finite() && params.ceiling_hz > 100.0) {
        return invalid("params.ceiling_hz must be finite and greater than 100 Hz");
    }
    if params.max_formants == 0 {
        return invalid("params.max_formants must be positive");
    }
    if !(params.window_length.is_finite() && params.window_length > 0.0) {
        return invalid("params.window_length must be finite and positive");
    }
    if let Some(step) = params.time_step
        && !(step.is_finite() && step > 0.0)
    {
        return invalid("params.time_step must be finite and positive when set");
    }
    if !params.preemphasis_from_hz.is_finite() {
        return invalid("params.preemphasis_from_hz must be finite");
    }
    Ok(())
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
    const VOWEL_WAV: &[u8] = include_bytes!("../../../tests/fixtures/audio/synth_vowel_a.wav");

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
    fn band_energy_is_finite_over_a_voiced_box_and_errors_on_nan() {
        let mut engine = Engine::new();
        let id = engine.import_wav_bytes(VOWEL_WAV).unwrap();
        let info = engine.audio_info(id).unwrap();
        let value = engine
            .band_energy(id, info.duration * 0.3, info.duration * 0.6, 0.0, 4000.0)
            .unwrap();
        assert!(value.is_finite(), "band energy over a vowel box is finite");
        // Order-independence: swapping the bounds names the same box.
        let swapped = engine
            .band_energy(id, info.duration * 0.6, info.duration * 0.3, 4000.0, 0.0)
            .unwrap();
        assert_eq!(value.to_bits(), swapped.to_bits());
        assert!(matches!(
            engine.band_energy(id, f64::NAN, 0.1, 0.0, 4000.0),
            Err(EngineError::InvalidRequest { .. })
        ));
    }

    #[test]
    fn selection_readout_band_energy_equals_the_direct_query() {
        let mut engine = Engine::new();
        let id = engine.import_wav_bytes(VOWEL_WAV).unwrap();
        let info = engine.audio_info(id).unwrap();
        let (t0, t1, f0, f1) = (info.duration * 0.3, info.duration * 0.6, 0.0, 4000.0);
        let readout = engine
            .selection_readout(id, t0, t1, f0, f1, 75.0, 600.0, 100.0)
            .unwrap();
        let direct = engine.band_energy(id, t0, t1, f0, f1).unwrap();
        // The batch-equals-GUI invariant: the readout's band energy is the same
        // engine query a script would run for the same box.
        assert_eq!(readout.band_energy_db.to_bits(), direct.to_bits());
        assert!((readout.duration - (t1 - t0)).abs() < 1.0e-12);
        assert!(readout.f0_mean_hz.is_some(), "vowel span should be voiced");
    }

    #[test]
    fn colormap_change_recolorizes_cached_db_without_recomputing() {
        let mut engine = Engine::new();
        let id = engine.import_wav_bytes(VOWEL_WAV).unwrap();
        let info = engine.audio_info(id).unwrap();
        let req = TileRequest {
            t0: 0.0,
            t1: info.duration,
            f0: 0.0,
            f1: 5000.0,
            width_px: 220,
            height_px: 128,
            params: SpectrogramParams::default(),
        };
        let display = DisplayMapping::default();
        let viridis = engine
            .spectrogram_tile_rgba(id, &req, &display, Colormap::Viridis, Theme::Dark)
            .unwrap();
        let after_first = engine.spectrogram_cached_block_count();
        assert!(after_first > 0, "first tile populates the block cache");

        let magma = engine
            .spectrogram_tile_rgba(id, &req, &display, Colormap::Magma, Theme::Dark)
            .unwrap();
        // Re-colorizing the same viewport reuses every cached block: no new STFT.
        assert_eq!(engine.spectrogram_cached_block_count(), after_first);
        assert_ne!(
            viridis, magma,
            "different palettes produce different pixels"
        );

        // Deterministic through the cache: the same request twice is identical.
        let viridis_again = engine
            .spectrogram_tile_rgba(id, &req, &display, Colormap::Viridis, Theme::Dark)
            .unwrap();
        assert_eq!(viridis, viridis_again);
    }

    #[test]
    fn changing_analysis_params_keys_new_blocks() {
        let mut engine = Engine::new();
        let id = engine.import_wav_bytes(VOWEL_WAV).unwrap();
        let info = engine.audio_info(id).unwrap();
        let display = DisplayMapping::default();
        let base = TileRequest {
            t0: 0.0,
            t1: info.duration,
            f0: 0.0,
            f1: 5000.0,
            width_px: 200,
            height_px: 120,
            params: SpectrogramParams::default(),
        };
        engine
            .spectrogram_tile_rgba(id, &base, &display, Colormap::Viridis, Theme::Dark)
            .unwrap();
        let after_base = engine.spectrogram_cached_block_count();

        let widened = TileRequest {
            params: SpectrogramParams {
                window_length: 0.01,
                ..SpectrogramParams::default()
            },
            ..base.clone()
        };
        engine
            .spectrogram_tile_rgba(id, &widened, &display, Colormap::Viridis, Theme::Dark)
            .unwrap();
        // A different analysis parameter hashes to a different key, so the new
        // blocks sit alongside the old rather than colliding with them.
        assert!(engine.spectrogram_cached_block_count() > after_base);
    }

    #[test]
    fn voice_report_on_clean_vowel_has_low_perturbation_and_high_hnr() {
        let mut engine = Engine::new();
        let id = engine.import_wav_bytes(VOWEL_WAV).unwrap();
        let info = engine.audio_info(id).unwrap();
        let report = engine
            .voice_report(id, info.duration * 0.2, info.duration * 0.8, 75.0, 600.0)
            .unwrap();
        let jitter = report.jitter.local.expect("local jitter over the vowel");
        let shimmer = report.shimmer.local.expect("local shimmer over the vowel");
        let hnr = report.mean_hnr_db.expect("mean HNR over the vowel");
        assert!(
            jitter < 0.05,
            "clean vowel local jitter {jitter} should be small"
        );
        assert!(
            shimmer < 0.2,
            "clean vowel local shimmer {shimmer} should be small"
        );
        assert!(hnr > 10.0, "clean vowel HNR {hnr} dB should be high");
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
    fn streaming_recording_equals_a_one_shot_buffer_bit_for_bit() {
        let sample_rate = 16_000.0;
        let samples: Vec<f32> = (0..2_000)
            .map(|i| (2.0 * PI * 220.0 * i as f64 / sample_rate).sin() as f32)
            .collect();

        // Stream the same samples through three uneven chunks.
        let mut engine = Engine::new();
        let rec = engine.begin_recording(sample_rate, 1).unwrap();
        for chunk in samples.chunks(517) {
            engine.append_samples(rec, chunk).unwrap();
        }
        let finished = engine.finish_recording(rec, "take".to_string()).unwrap();

        // The materialized buffer matches one built from the whole sample vector
        // in a single call, sample for sample.
        let streamed = engine.store.audio(finished.audio).unwrap();
        assert_eq!(streamed.sample_rate(), sample_rate);
        assert_eq!(streamed.name(), Some("take"));
        let one_shot = Audio::new(vec![samples.clone()], sample_rate).unwrap();
        assert_eq!(streamed.frames(), one_shot.frames());
        for (a, b) in streamed.channel(0).iter().zip(one_shot.channel(0)) {
            assert_eq!(a.to_bits(), b.to_bits());
        }

        // The finished id is spent; a second finish is a typed error.
        assert!(matches!(
            engine.finish_recording(rec, "again".to_string()),
            Err(EngineError::UnknownRecordingId(_))
        ));

        // The WAV bytes round-trip back to the same signal.
        let reloaded = Audio::from_wav_bytes(&finished.wav).unwrap();
        assert_eq!(reloaded.frames(), samples.len());
    }

    #[test]
    fn streaming_recording_interleaves_planar_channels() {
        let mut engine = Engine::new();
        let rec = engine.begin_recording(8_000.0, 2).unwrap();
        // Two frames per chunk, planar: [L0, L1, R0, R1].
        engine.append_samples(rec, &[0.1, 0.2, -0.1, -0.2]).unwrap();
        engine.append_samples(rec, &[0.3, 0.4, -0.3, -0.4]).unwrap();
        let finished = engine.finish_recording(rec, "stereo".to_string()).unwrap();
        let audio = engine.store.audio(finished.audio).unwrap();
        assert_eq!(audio.channel_count(), 2);
        assert_eq!(audio.channel(0), &[0.1, 0.2, 0.3, 0.4]);
        assert_eq!(audio.channel(1), &[-0.1, -0.2, -0.3, -0.4]);
    }

    #[test]
    fn aborted_recording_leaves_no_store_entry_and_a_typed_error() {
        let mut engine = Engine::new();
        let before = engine.store.ids_sorted().len();
        let rec = engine.begin_recording(16_000.0, 1).unwrap();
        engine.append_samples(rec, &[0.0; 256]).unwrap();
        engine.abort_recording(rec).unwrap();
        // Aborting materializes nothing.
        assert_eq!(engine.store.ids_sorted().len(), before);
        // The take is gone: appending, finishing, or aborting again all reject.
        assert!(matches!(
            engine.append_samples(rec, &[0.0; 4]),
            Err(EngineError::UnknownRecordingId(_))
        ));
        assert!(matches!(
            engine.abort_recording(rec),
            Err(EngineError::UnknownRecordingId(_))
        ));
    }

    #[test]
    fn recording_rejects_bad_parameters_and_ragged_chunks() {
        let mut engine = Engine::new();
        assert!(matches!(
            engine.begin_recording(0.0, 1),
            Err(EngineError::InvalidRequest { .. })
        ));
        assert!(matches!(
            engine.begin_recording(16_000.0, 0),
            Err(EngineError::InvalidRequest { .. })
        ));
        let rec = engine.begin_recording(16_000.0, 2).unwrap();
        // An odd chunk cannot split across two channels.
        assert!(matches!(
            engine.append_samples(rec, &[0.1, 0.2, 0.3]),
            Err(EngineError::InvalidRequest { .. })
        ));
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
    fn pitch_track_on_fixture_is_voiced_in_the_male_speech_band() {
        let mut engine = Engine::new();
        let id = engine.import_wav_bytes(FIXTURE_WAV).unwrap();
        let track = engine.pitch_track(id, &PitchParams::default()).unwrap();
        let voiced: Vec<f64> = track.frames().iter().filter_map(|frame| frame.f0).collect();
        assert!(!voiced.is_empty(), "male speech fixture should be voiced");
        // bdl is an adult male speaker; every voiced frame stays inside the
        // analysis band, and the median must sit in the 70-300 Hz range Praat
        // reports for this corpus. Individual frames can reach the ceiling on
        // octave slips, so the band claim rests on the median, not each frame.
        for f0 in &voiced {
            assert!(
                *f0 > 50.0 && *f0 <= PitchParams::default().ceiling_hz,
                "F0 {f0} Hz outside the analysis band"
            );
        }
        let mut sorted = voiced.clone();
        sorted.sort_by(f64::total_cmp);
        let median = sorted[sorted.len() / 2];
        assert!(
            (70.0..=300.0).contains(&median),
            "median F0 {median} Hz outside male band"
        );
    }

    #[test]
    fn pitch_track_span_places_frames_on_the_absolute_timeline() {
        let mut engine = Engine::new();
        let id = engine.import_wav_bytes(FIXTURE_WAV).unwrap();
        let info = engine.audio_info(id).unwrap();
        let t0 = info.duration * 0.3;
        let t1 = info.duration * 0.6;
        let (track, start_time) = engine
            .pitch_track_span(id, &PitchParams::default(), t0, t1)
            .unwrap();
        assert!(!track.frames().is_empty());
        assert!(start_time >= t0 - 1.0e-3 && start_time <= t1);
        // Every frame, shifted onto the absolute timeline, lands inside the
        // requested window (allowing the leading half-window margin).
        for frame in track.frames() {
            let abs = start_time + frame.time;
            assert!(
                abs >= t0 - 1.0e-3 && abs <= t1 + 1.0e-3,
                "abs {abs} out of span"
            );
        }
    }

    #[test]
    fn formant_track_raw_and_smoothed_share_the_frame_grid() {
        let mut engine = Engine::new();
        let id = engine.import_wav_bytes(FIXTURE_WAV).unwrap();
        let params = FormantParams::default();
        let raw = engine.formant_track(id, &params).unwrap();
        let smoothed = engine.formant_track_smoothed(id, &params).unwrap();
        assert!(!raw.frames.is_empty());
        assert_eq!(raw.frames.len(), smoothed.frames.len());
        assert_eq!(raw.frame_grid, smoothed.frame_grid);
        let has_formants = raw.frames.iter().any(|frame| !frame.formants.is_empty());
        assert!(
            has_formants,
            "speech fixture should yield formant candidates"
        );
    }

    #[test]
    fn bad_formant_ceiling_is_a_typed_error_not_a_panic() {
        let mut engine = Engine::new();
        let id = engine.import_wav_bytes(FIXTURE_WAV).unwrap();
        let params = FormantParams {
            ceiling_hz: 10.0,
            ..FormantParams::default()
        };
        assert!(matches!(
            engine.formant_track(id, &params),
            Err(EngineError::InvalidRequest { .. })
        ));
    }

    #[test]
    fn intensity_track_on_fixture_is_non_empty_and_finite() {
        let mut engine = Engine::new();
        let id = engine.import_wav_bytes(FIXTURE_WAV).unwrap();
        let track = engine
            .intensity_track(id, &IntensityParams::default())
            .unwrap();
        assert!(!track.is_empty());
        assert!(track.values().iter().all(|db| db.is_finite()));
    }

    #[test]
    fn bad_intensity_floor_is_a_typed_error_not_a_panic() {
        let mut engine = Engine::new();
        let id = engine.import_wav_bytes(FIXTURE_WAV).unwrap();
        let params = IntensityParams {
            pitch_floor_hz: 0.0,
            ..IntensityParams::default()
        };
        assert!(matches!(
            engine.intensity_track(id, &params),
            Err(EngineError::InvalidRequest { .. })
        ));
    }

    #[test]
    fn analysis_on_unknown_id_is_a_typed_error() {
        let engine = Engine::new();
        let bogus = AudioId::from_u64(4242);
        assert!(matches!(
            engine.pitch_track(bogus, &PitchParams::default()),
            Err(EngineError::UnknownAudioId(_))
        ));
        assert!(matches!(
            engine.formant_track(bogus, &FormantParams::default()),
            Err(EngineError::UnknownAudioId(_))
        ));
        assert!(matches!(
            engine.intensity_track(bogus, &IntensityParams::default()),
            Err(EngineError::UnknownAudioId(_))
        ));
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

    // --- Journal and annotation surface ---

    /// Small deterministic xorshift generator; the property test needs a
    /// reproducible command stream without pulling in an rng dependency.
    struct Rng(u64);

    impl Rng {
        fn new(seed: u64) -> Self {
            Self(seed | 1)
        }

        fn next_u64(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }

        fn below(&mut self, n: usize) -> usize {
            (self.next_u64() % n as u64) as usize
        }

        fn frac(&mut self) -> f64 {
            (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
        }
    }

    fn annotation_with_tier(xmax: f64) -> Annotation {
        let mut annotation = Annotation::new(0.0, xmax).unwrap();
        annotation
            .add_interval_tier("phones", TierRelation::Independent)
            .unwrap();
        annotation
    }

    fn base_engine() -> (Engine, AudioId, AnnotationId) {
        let mut engine = Engine::new();
        let bytes = sine_wav_bytes(8_000, 2.0, 220.0);
        let audio = match engine
            .apply(Command::ImportAudio {
                bytes,
                name: "base".to_string(),
            })
            .unwrap()
        {
            Applied::AudioImported { audio } => audio,
            other => panic!("expected AudioImported, got {other:?}"),
        };
        let annotation = annotation_with_tier(2.0);
        let doc = match engine
            .apply(Command::AttachAnnotation { audio, annotation })
            .unwrap()
        {
            Applied::AnnotationAttached { annotation, .. } => annotation,
            other => panic!("expected AnnotationAttached, got {other:?}"),
        };
        (engine, audio, doc)
    }

    fn interval_tiers(annotation: &Annotation) -> Vec<(TierId, Vec<Interval>)> {
        annotation
            .tiers()
            .iter()
            .filter_map(|slot| match &slot.tier {
                Tier::Interval(tier) => Some((slot.id, tier.intervals.clone())),
                Tier::Point(_) => None,
            })
            .collect()
    }

    #[test]
    fn attach_reports_incremental_changes_and_reads_back() {
        let (mut engine, audio, doc) = base_engine();
        let (tier, intervals) = {
            let annotation = engine.annotation(doc).unwrap();
            interval_tiers(annotation).remove(0)
        };
        assert_eq!(intervals.len(), 1);
        assert_eq!(engine.annotation_audio(doc).unwrap(), audio);

        let at = 1.0;
        let boundary = match engine
            .apply(Command::InsertBoundary {
                annotation: doc,
                tier,
                at,
            })
            .unwrap()
        {
            Applied::BoundaryInserted { boundary, .. } => boundary,
            other => panic!("expected BoundaryInserted, got {other:?}"),
        };
        let intervals = interval_tiers(engine.annotation(doc).unwrap()).remove(0).1;
        assert_eq!(intervals.len(), 2);

        let applied = engine
            .apply(Command::SetLabel {
                annotation: doc,
                target: LabelTarget::Interval {
                    tier,
                    interval: intervals[0].id,
                },
                text: "aː".to_string(),
            })
            .unwrap();
        assert!(matches!(applied, Applied::LabelSet { .. }));
        let _ = boundary;
    }

    #[test]
    fn undo_redo_restores_state_hash_against_untouched_engine() {
        let (mut engine, _audio, doc) = base_engine();
        let (mut untouched, _, _) = base_engine();
        let baseline = engine.state_hash();
        assert_eq!(baseline, untouched.state_hash());
        // Both engines are independent constructions; the "untouched" one is
        // the never-mutated reference the invariant compares against.
        assert!(untouched.undo().unwrap().is_some());
        assert!(untouched.redo().unwrap().is_some());
        assert_eq!(untouched.state_hash(), baseline);

        let (tier, intervals) = {
            let annotation = engine.annotation(doc).unwrap();
            interval_tiers(annotation).remove(0)
        };
        engine
            .apply(Command::InsertBoundary {
                annotation: doc,
                tier,
                at: 0.5,
            })
            .unwrap();
        engine
            .apply(Command::InsertBoundary {
                annotation: doc,
                tier,
                at: 1.5,
            })
            .unwrap();
        engine
            .apply(Command::SetLabel {
                annotation: doc,
                target: LabelTarget::Interval {
                    tier,
                    interval: intervals[0].id,
                },
                text: "x".to_string(),
            })
            .unwrap();
        let final_hash = engine.state_hash();
        assert_ne!(final_hash, baseline);

        for _ in 0..3 {
            assert!(engine.undo().unwrap().is_some());
        }
        assert_eq!(engine.state_hash(), baseline);
        assert_eq!(engine.state_hash(), untouched.state_hash());

        for _ in 0..3 {
            assert!(engine.redo().unwrap().is_some());
        }
        assert_eq!(engine.state_hash(), final_hash);
    }

    #[test]
    fn redo_stack_clears_on_new_command() {
        let (mut engine, _audio, doc) = base_engine();
        let tier = interval_tiers(engine.annotation(doc).unwrap()).remove(0).0;
        engine
            .apply(Command::InsertBoundary {
                annotation: doc,
                tier,
                at: 1.0,
            })
            .unwrap();
        engine.undo().unwrap();
        assert_eq!(engine.redo_depth(), 1);
        // A fresh command discards the pending redo.
        engine
            .apply(Command::AddIntervalTier {
                annotation: doc,
                name: "words".to_string(),
                relation: TierRelation::Independent,
            })
            .unwrap();
        assert_eq!(engine.redo_depth(), 0);
        assert!(engine.redo().unwrap().is_none());
    }

    #[test]
    fn tier_lifecycle_undo_restores_ids() {
        let (mut engine, _audio, doc) = base_engine();
        let before = engine.state_hash();
        let tier = match engine
            .apply(Command::AddPointTier {
                annotation: doc,
                name: "tones".to_string(),
                points: vec![(0.5, "H".to_string()), (1.5, "L".to_string())],
                relation: TierRelation::Independent,
            })
            .unwrap()
        {
            Applied::TierAdded { tier, .. } => tier,
            other => panic!("expected TierAdded, got {other:?}"),
        };
        let added = engine.state_hash();
        engine
            .apply(Command::RemoveTier {
                annotation: doc,
                tier,
            })
            .unwrap();
        assert_eq!(engine.state_hash(), before);
        engine.undo().unwrap(); // undo the removal
        assert_eq!(engine.state_hash(), added);
        engine.undo().unwrap(); // undo the addition
        assert_eq!(engine.state_hash(), before);
    }

    #[test]
    fn search_labels_spans_all_documents() {
        let (mut engine, audio, first) = base_engine();
        let tier = interval_tiers(engine.annotation(first).unwrap())
            .remove(0)
            .0;
        let interval = interval_tiers(engine.annotation(first).unwrap())
            .remove(0)
            .1[0]
            .id;
        engine
            .apply(Command::SetLabel {
                annotation: first,
                target: LabelTarget::Interval { tier, interval },
                text: "vowel".to_string(),
            })
            .unwrap();

        let second_annotation = {
            let mut a = annotation_with_tier(2.0);
            let (tier_id, intervals) = interval_tiers(&a).remove(0);
            a.set_label(
                LabelTarget::Interval {
                    tier: tier_id,
                    interval: intervals[0].id,
                },
                "vowel space",
            )
            .unwrap();
            a
        };
        let second = match engine
            .apply(Command::AttachAnnotation {
                audio,
                annotation: second_annotation,
            })
            .unwrap()
        {
            Applied::AnnotationAttached { annotation, .. } => annotation,
            other => panic!("expected AnnotationAttached, got {other:?}"),
        };

        let hits = engine.search_labels(&LabelQuery::substring("vowel"));
        assert_eq!(hits.len(), 2);
        let docs: Vec<AnnotationId> = hits.iter().map(|hit| hit.annotation).collect();
        assert!(docs.contains(&first));
        assert!(docs.contains(&second));
    }

    /// Roadmap phase-3 gate: a random 50-command mix undone in full returns to
    /// the initial state hash, and redone in full returns to the final one.
    #[test]
    fn random_fifty_command_undo_stack_is_hash_stable() {
        let (mut engine, audio, doc) = base_engine();
        let initial = engine.state_hash();

        let mut rng = Rng::new(0x9E37_79B9_7F4A_7C15);
        let mut name_counter = 0_u32;
        let mut applied_hashes = Vec::new();
        let mut guard = 0;

        while applied_hashes.len() < 50 {
            guard += 1;
            assert!(guard < 20_000, "generator failed to reach 50 commands");
            let Some(cmd) = gen_command(&engine, audio, doc, &mut rng, &mut name_counter) else {
                continue;
            };
            if engine.apply(cmd).is_ok() {
                applied_hashes.push(engine.state_hash());
            }
        }

        let final_hash = engine.state_hash();
        assert_eq!(engine.undo_depth(), 52); // 50 + import + attach

        // Undo the 50 generated commands; each step matches the hash recorded
        // just before it was applied.
        for expected in applied_hashes.iter().rev().skip(1) {
            engine.undo().unwrap();
            assert_eq!(engine.state_hash(), *expected);
        }
        engine.undo().unwrap();
        assert_eq!(engine.state_hash(), initial);

        // Redo the 50 commands; each step reproduces its recorded hash.
        for expected in &applied_hashes {
            engine.redo().unwrap();
            assert_eq!(engine.state_hash(), *expected);
        }
        assert_eq!(engine.state_hash(), final_hash);
    }

    /// Chooses a state-valid command from the current engine, or `None` when
    /// the roll cannot be satisfied (the caller retries). Content targets are
    /// read from live state so the generated command almost always applies.
    fn gen_command(
        engine: &Engine,
        audio: AudioId,
        doc: AnnotationId,
        rng: &mut Rng,
        name_counter: &mut u32,
    ) -> Option<Command> {
        let annotation = engine.annotation(doc).ok()?;
        let tiers = interval_tiers(annotation);
        let roll = rng.below(100);

        match roll {
            0..=39 => {
                // Set a label on a random interval.
                if tiers.is_empty() {
                    return None;
                }
                let (tier, intervals) = &tiers[rng.below(tiers.len())];
                let interval = &intervals[rng.below(intervals.len())];
                let text = format!("l{}", rng.below(1000));
                Some(Command::SetLabel {
                    annotation: doc,
                    target: LabelTarget::Interval {
                        tier: *tier,
                        interval: interval.id,
                    },
                    text,
                })
            }
            40..=57 => {
                // Split a wide interval at an interior fraction.
                let wide: Vec<&(TierId, Vec<Interval>)> = tiers
                    .iter()
                    .filter(|(_, ivs)| ivs.iter().any(|iv| iv.xmax - iv.xmin > 1.0e-3))
                    .collect();
                if wide.is_empty() {
                    return None;
                }
                let (tier, intervals) = wide[rng.below(wide.len())];
                let candidates: Vec<&Interval> = intervals
                    .iter()
                    .filter(|iv| iv.xmax - iv.xmin > 1.0e-3)
                    .collect();
                let interval = candidates[rng.below(candidates.len())];
                let frac = 0.2 + 0.6 * rng.frac();
                let at = interval.xmin + frac * (interval.xmax - interval.xmin);
                if at.to_bits() == interval.xmin.to_bits()
                    || at.to_bits() == interval.xmax.to_bits()
                {
                    return None;
                }
                Some(Command::InsertBoundary {
                    annotation: doc,
                    tier: *tier,
                    at,
                })
            }
            58..=69 => {
                // Move an interior boundary within its neighbours.
                let (_tier, intervals) = pick_multi_interval(&tiers, rng)?;
                let i = rng.below(intervals.len() - 1);
                let lo = intervals[i].xmin;
                let hi = intervals[i + 1].xmax;
                let frac = 0.2 + 0.6 * rng.frac();
                let at = lo + frac * (hi - lo);
                if at.to_bits() == intervals[i].xmax.to_bits() {
                    return None;
                }
                Some(Command::MoveBoundary {
                    annotation: doc,
                    boundary: intervals[i].end_boundary,
                    to: at,
                    mode: AlignMode::Linked,
                })
            }
            70..=77 => {
                // Remove an interior boundary.
                let (_tier, intervals) = pick_multi_interval(&tiers, rng)?;
                let i = rng.below(intervals.len() - 1);
                Some(Command::RemoveBoundary {
                    annotation: doc,
                    boundary: intervals[i].end_boundary,
                })
            }
            78..=82 => {
                *name_counter += 1;
                Some(Command::AddIntervalTier {
                    annotation: doc,
                    name: format!("tier{name_counter}"),
                    relation: TierRelation::Independent,
                })
            }
            83..=85 => {
                // Remove a tier other than the primary one, when one exists.
                if annotation.tiers().len() < 2 {
                    return None;
                }
                let slot = &annotation.tiers()[1 + rng.below(annotation.tiers().len() - 1)];
                Some(Command::RemoveTier {
                    annotation: doc,
                    tier: slot.id,
                })
            }
            86..=88 => {
                *name_counter += 1;
                Some(Command::AddPointTier {
                    annotation: doc,
                    name: format!("pts{name_counter}"),
                    points: vec![(0.4, "a".to_string()), (1.2, "b".to_string())],
                    relation: TierRelation::Independent,
                })
            }
            89..=91 => {
                // Insert a point in a free slot of a random point tier.
                let point_tiers = point_tiers(annotation);
                if point_tiers.is_empty() {
                    return None;
                }
                let (tier, points) = &point_tiers[rng.below(point_tiers.len())];
                let mut fence = vec![annotation.xmin()];
                fence.extend(points.iter().map(|point| point.time));
                fence.push(annotation.xmax());
                let mids: Vec<f64> = fence
                    .windows(2)
                    .filter_map(|pair| {
                        let mid = (pair[0] + pair[1]) / 2.0;
                        (mid.to_bits() != pair[0].to_bits() && mid.to_bits() != pair[1].to_bits())
                            .then_some(mid)
                    })
                    .collect();
                if mids.is_empty() {
                    return None;
                }
                Some(Command::InsertPoint {
                    annotation: doc,
                    tier: *tier,
                    time: mids[rng.below(mids.len())],
                    label: format!("p{}", rng.below(1000)),
                })
            }
            92..=93 => {
                // Move a point within its immediate neighbours.
                let movable = movable_points(annotation);
                if movable.is_empty() {
                    return None;
                }
                let (point, lower, upper) = movable[rng.below(movable.len())];
                let frac = 0.2 + 0.6 * rng.frac();
                let to = lower + frac * (upper - lower);
                if to.to_bits() == lower.to_bits() || to.to_bits() == upper.to_bits() {
                    return None;
                }
                Some(Command::MovePoint {
                    annotation: doc,
                    point,
                    to,
                })
            }
            94..=95 => {
                // Remove a random point.
                let ids: Vec<PointId> = point_tiers(annotation)
                    .into_iter()
                    .flat_map(|(_tier, points)| points.into_iter().map(|point| point.id))
                    .collect();
                if ids.is_empty() {
                    return None;
                }
                Some(Command::RemovePoint {
                    annotation: doc,
                    point: ids[rng.below(ids.len())],
                })
            }
            96..=97 => {
                // Move a tier to a random index.
                let count = annotation.tiers().len();
                if count < 2 {
                    return None;
                }
                let tier = annotation.tiers()[rng.below(count)].id;
                Some(Command::ReorderTier {
                    annotation: doc,
                    tier,
                    to_index: rng.below(count),
                })
            }
            98 => Some(Command::AttachAnnotation {
                audio,
                annotation: annotation_with_tier(2.0),
            }),
            _ => {
                let bytes = sine_wav_bytes(8_000, 0.2, 300.0);
                Some(Command::ImportAudio {
                    bytes,
                    name: format!("clip{name_counter}"),
                })
            }
        }
    }

    fn pick_multi_interval<'a>(
        tiers: &'a [(TierId, Vec<Interval>)],
        rng: &mut Rng,
    ) -> Option<(TierId, &'a Vec<Interval>)> {
        let multi: Vec<&(TierId, Vec<Interval>)> =
            tiers.iter().filter(|(_, ivs)| ivs.len() >= 2).collect();
        if multi.is_empty() {
            return None;
        }
        let (tier, intervals) = multi[rng.below(multi.len())];
        Some((*tier, intervals))
    }

    fn point_tiers(annotation: &Annotation) -> Vec<(TierId, Vec<Point>)> {
        annotation
            .tiers()
            .iter()
            .filter_map(|slot| match &slot.tier {
                Tier::Point(tier) => Some((slot.id, tier.points.clone())),
                Tier::Interval(_) => None,
            })
            .collect()
    }

    fn movable_points(annotation: &Annotation) -> Vec<(PointId, f64, f64)> {
        let mut out = Vec::new();
        for (_tier, points) in point_tiers(annotation) {
            for (index, point) in points.iter().enumerate() {
                let lower = if index > 0 {
                    points[index - 1].time
                } else {
                    annotation.xmin()
                };
                let upper = if index + 1 < points.len() {
                    points[index + 1].time
                } else {
                    annotation.xmax()
                };
                if upper > lower {
                    out.push((point.id, lower, upper));
                }
            }
        }
        out
    }

    /// A shared boundary on an aligned tier pair moves both tiers atomically as
    /// one journal entry, and a single undo restores both.
    #[test]
    fn aligned_tier_pair_moves_and_undoes_as_one_entry() {
        let (mut engine, _audio, doc) = base_engine();
        let primary = interval_tiers(engine.annotation(doc).unwrap()).remove(0).0;

        let aligned = match engine
            .apply(Command::AddIntervalTier {
                annotation: doc,
                name: "words".to_string(),
                relation: TierRelation::AlignedBoundaries { with: primary },
            })
            .unwrap()
        {
            Applied::TierAdded { tier, .. } => tier,
            other => panic!("expected TierAdded, got {other:?}"),
        };

        // Undo the tier add cleanly, then redo it back.
        let with_pair = engine.state_hash();
        engine.undo().unwrap();
        assert!(engine.annotation(doc).unwrap().tier(aligned).is_none());
        engine.redo().unwrap();
        assert_eq!(engine.state_hash(), with_pair);

        // Insert a boundary on the primary tier; it propagates to the aligned
        // peer, and the whole propagation is one undoable entry.
        let depth_before = engine.undo_depth();
        let boundary = match engine
            .apply(Command::InsertBoundary {
                annotation: doc,
                tier: primary,
                at: 1.0,
            })
            .unwrap()
        {
            Applied::BoundaryInserted { boundary, .. } => boundary,
            other => panic!("expected BoundaryInserted, got {other:?}"),
        };
        assert_eq!(engine.undo_depth(), depth_before + 1);
        for (_id, intervals) in interval_tiers(engine.annotation(doc).unwrap()) {
            assert_eq!(intervals.len(), 2, "both tiers gained the boundary");
        }

        // Move the shared boundary linked; both tiers move in one entry.
        let before_move = engine.state_hash();
        let moves = match engine
            .apply(Command::MoveBoundary {
                annotation: doc,
                boundary,
                to: 1.3,
                mode: AlignMode::Linked,
            })
            .unwrap()
        {
            Applied::BoundaryMoved { moves, .. } => moves,
            other => panic!("expected BoundaryMoved, got {other:?}"),
        };
        assert_eq!(moves.len(), 2, "both aligned tiers moved");
        for (_id, intervals) in interval_tiers(engine.annotation(doc).unwrap()) {
            assert_eq!(intervals[0].xmax.to_bits(), 1.3_f64.to_bits());
        }

        // A single undo restores both tiers to the shared boundary at 1.0.
        engine.undo().unwrap();
        assert_eq!(engine.state_hash(), before_move);
        for (_id, intervals) in interval_tiers(engine.annotation(doc).unwrap()) {
            assert_eq!(intervals[0].xmax.to_bits(), 1.0_f64.to_bits());
        }
    }

    /// A point round-trip through the journal: insert, move, remove, then undo
    /// each back to the initial hash and redo to the final one.
    #[test]
    fn point_commands_undo_redo_are_hash_stable() {
        let (mut engine, _audio, doc) = base_engine();
        let tier = match engine
            .apply(Command::AddPointTier {
                annotation: doc,
                name: "tones".to_string(),
                points: Vec::new(),
                relation: TierRelation::Independent,
            })
            .unwrap()
        {
            Applied::TierAdded { tier, .. } => tier,
            other => panic!("expected TierAdded, got {other:?}"),
        };
        let empty = engine.state_hash();

        let point = match engine
            .apply(Command::InsertPoint {
                annotation: doc,
                tier,
                time: 1.0,
                label: "H".to_string(),
            })
            .unwrap()
        {
            Applied::PointInserted { point, .. } => point,
            other => panic!("expected PointInserted, got {other:?}"),
        };
        let inserted = engine.state_hash();

        engine
            .apply(Command::MovePoint {
                annotation: doc,
                point,
                to: 1.4,
            })
            .unwrap();
        let moved = engine.state_hash();
        assert_ne!(moved, inserted);

        engine
            .apply(Command::RemovePoint {
                annotation: doc,
                point,
            })
            .unwrap();
        assert_eq!(
            engine.state_hash(),
            empty,
            "removal returns to the empty tier"
        );

        // Undo removal, move, insertion.
        engine.undo().unwrap();
        assert_eq!(engine.state_hash(), moved);
        engine.undo().unwrap();
        assert_eq!(engine.state_hash(), inserted);
        engine.undo().unwrap();
        assert_eq!(engine.state_hash(), empty);

        // Redo insertion, move, removal back to the empty tier.
        engine.redo().unwrap();
        assert_eq!(engine.state_hash(), inserted);
        engine.redo().unwrap();
        assert_eq!(engine.state_hash(), moved);
        engine.redo().unwrap();
        assert_eq!(engine.state_hash(), empty);
    }

    /// Reordering a tier is invertible and hash-stable through the journal.
    #[test]
    fn reorder_tier_undo_restores_order() {
        let (mut engine, _audio, doc) = base_engine();
        engine
            .apply(Command::AddIntervalTier {
                annotation: doc,
                name: "words".to_string(),
                relation: TierRelation::Independent,
            })
            .unwrap();
        let order_before: Vec<TierId> = engine
            .annotation(doc)
            .unwrap()
            .tiers()
            .iter()
            .map(|slot| slot.id)
            .collect();
        let hash_before = engine.state_hash();

        let last = *order_before.last().unwrap();
        engine
            .apply(Command::ReorderTier {
                annotation: doc,
                tier: last,
                to_index: 0,
            })
            .unwrap();
        let reordered: Vec<TierId> = engine
            .annotation(doc)
            .unwrap()
            .tiers()
            .iter()
            .map(|slot| slot.id)
            .collect();
        assert_eq!(reordered[0], last);
        assert_ne!(engine.state_hash(), hash_before);

        engine.undo().unwrap();
        assert_eq!(engine.state_hash(), hash_before);
        let restored: Vec<TierId> = engine
            .annotation(doc)
            .unwrap()
            .tiers()
            .iter()
            .map(|slot| slot.id)
            .collect();
        assert_eq!(restored, order_before);
    }

    // --- Streamed path: equal to the eager path on the same bytes -----------

    fn eager_and_streamed(bytes: &[u8]) -> (Engine, AudioId, Engine, AudioId) {
        let mut eager = Engine::new();
        let eager_id = eager.import_wav_bytes(bytes).unwrap();
        let mut streamed = Engine::new();
        let streamed_id = streamed
            .open_streaming_wav(BytesReader::new(bytes.to_vec()), Some("take".to_string()))
            .unwrap();
        (eager, eager_id, streamed, streamed_id)
    }

    #[test]
    fn streamed_audio_info_matches_the_eager_decode() {
        let (eager, eid, streamed, sid) = eager_and_streamed(FIXTURE_WAV);
        let a = eager.audio_info(eid).unwrap();
        let b = streamed.audio_info(sid).unwrap();
        assert_eq!(a.duration.to_bits(), b.duration.to_bits());
        assert_eq!(a.sample_rate.to_bits(), b.sample_rate.to_bits());
        assert_eq!(a.channels, b.channels);
    }

    #[test]
    fn streamed_waveform_slices_are_bit_identical_to_the_eager_pyramid() {
        let (eager, eid, streamed, sid) = eager_and_streamed(FIXTURE_WAV);
        let duration = eager.audio_info(eid).unwrap().duration;
        for &px in &[1u32, 13, 128, 777, 2000] {
            for &(a, b) in &[
                (0.0, duration),
                (0.0, duration * 0.5),
                (duration * 0.25, duration * 0.85),
                (duration * 0.499, duration * 0.501),
                (0.0, 0.003),
            ] {
                let expected = eager.waveform_slice(eid, a, b, px).unwrap();
                let actual = streamed.waveform_slice(sid, a, b, px).unwrap();
                assert_eq!(actual.len(), expected.len(), "px {px} span {a}..{b}");
                for (i, (x, y)) in actual.iter().zip(&expected).enumerate() {
                    assert_eq!(
                        x.min.to_bits(),
                        y.min.to_bits(),
                        "px {px} {a}..{b} bucket {i}"
                    );
                    assert_eq!(
                        x.max.to_bits(),
                        y.max.to_bits(),
                        "px {px} {a}..{b} bucket {i}"
                    );
                }
            }
        }
    }

    #[test]
    fn streamed_spectrogram_tile_db_is_bit_identical_to_the_eager_tile() {
        let (eager, eid, streamed, sid) = eager_and_streamed(FIXTURE_WAV);
        let duration = eager.audio_info(eid).unwrap().duration;
        let req = TileRequest {
            t0: duration * 0.2,
            t1: duration * 0.7,
            f0: 0.0,
            f1: 5000.0,
            width_px: 220,
            height_px: 90,
            params: SpectrogramParams::default(),
        };
        let expected = eager.spectrogram_tile_db(eid, &req).unwrap();
        let actual = streamed.spectrogram_tile_db(sid, &req).unwrap();
        assert_eq!(actual.len(), expected.len());
        for (i, (x, y)) in actual.iter().zip(&expected).enumerate() {
            assert_eq!(x.to_bits(), y.to_bits(), "db cell {i}");
        }
    }

    #[test]
    fn streamed_pitch_track_span_is_bit_identical_to_the_eager_span() {
        let (eager, eid, streamed, sid) = eager_and_streamed(FIXTURE_WAV);
        let duration = eager.audio_info(eid).unwrap().duration;
        let params = PitchParams::default();
        let (a, at) = eager
            .pitch_track_span(eid, &params, duration * 0.3, duration * 0.6)
            .unwrap();
        let (b, bt) = streamed
            .pitch_track_span(sid, &params, duration * 0.3, duration * 0.6)
            .unwrap();
        assert_eq!(at.to_bits(), bt.to_bits());
        assert_eq!(a.frames().len(), b.frames().len());
        for (i, (x, y)) in a.frames().iter().zip(b.frames()).enumerate() {
            assert_eq!(x.time.to_bits(), y.time.to_bits(), "frame {i} time");
            match (x.f0, y.f0) {
                (Some(fx), Some(fy)) => assert_eq!(fx.to_bits(), fy.to_bits(), "frame {i} f0"),
                (None, None) => {}
                _ => panic!("frame {i} voicing differs"),
            }
        }
    }

    #[test]
    fn streamed_whole_signal_pitch_track_matches_eager() {
        let (eager, eid, streamed, sid) = eager_and_streamed(VOWEL_WAV);
        let params = PitchParams::default();
        let a = eager.pitch_track(eid, &params).unwrap();
        let b = streamed.pitch_track(sid, &params).unwrap();
        assert_eq!(a.frames().len(), b.frames().len());
        for (i, (x, y)) in a.frames().iter().zip(b.frames()).enumerate() {
            assert_eq!(x.time.to_bits(), y.time.to_bits(), "frame {i}");
            assert_eq!(
                x.f0.map(f64::to_bits),
                y.f0.map(f64::to_bits),
                "frame {i} f0"
            );
        }
    }
}
