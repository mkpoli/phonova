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
mod journal;
mod pyramid;
mod store;

use std::hash::{Hash, Hasher};

use phx_audio::Audio;
use phx_dsp::Window;

use document::DocumentStore;
use journal::{Journal, JournalEntry, Reverse};

pub use commands::{Applied, Command, EngineHit};
pub use document::{AnnotationId, Document};
pub use error::EngineError;
pub use phx_annot::{
    AlignMode, Annotation, AnnotationError, BoundaryId, BoundaryMove, Hit, IntegrityIssue,
    Interval, IntervalId, IntervalTier, LabelPattern, LabelQuery, LabelTarget, MatchSpan, Merged,
    Moved, Point, PointId, PointTier, Tier, TierId, TierKind, TierMerge, TierRelation, TierSlot,
};
pub use phx_formant::{FormantFrame, FormantParams, FormantPoint, FormantTrack};
pub use phx_intensity::{IntensityParams, IntensityTrack};
pub use phx_pitch::{PitchFrame, PitchParams, PitchTrack};
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
    documents: DocumentStore,
    journal: Journal,
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
        let audio = self.store.audio(id)?;
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
        let audio = self.store.audio(id)?;
        let sample_rate = audio.sample_rate();
        let frames = audio.frames();
        let duration = audio.duration();
        let lo = t0.min(t1).clamp(0.0, duration);
        let hi = t0.max(t1).clamp(0.0, duration);
        let start = ((lo * sample_rate).floor() as usize).min(frames);
        let end = ((hi * sample_rate).ceil() as usize).clamp(start, frames);
        let view = audio.slice_samples(start..end);
        let track = phx_pitch::pitch_track(view, params);
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
        let audio = self.store.audio(id)?;
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
        let audio = self.store.audio(id)?;
        let view = audio.slice_samples(0..audio.frames());
        Ok(phx_intensity::intensity_track(view, params))
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
            0..=44 => {
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
            45..=64 => {
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
            65..=77 => {
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
            78..=85 => {
                // Remove an interior boundary.
                let (_tier, intervals) = pick_multi_interval(&tiers, rng)?;
                let i = rng.below(intervals.len() - 1);
                Some(Command::RemoveBoundary {
                    annotation: doc,
                    boundary: intervals[i].end_boundary,
                })
            }
            86..=90 => {
                *name_counter += 1;
                Some(Command::AddIntervalTier {
                    annotation: doc,
                    name: format!("tier{name_counter}"),
                    relation: TierRelation::Independent,
                })
            }
            91..=93 => {
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
            94..=96 => {
                *name_counter += 1;
                Some(Command::AddPointTier {
                    annotation: doc,
                    name: format!("pts{name_counter}"),
                    points: vec![(0.4, "a".to_string()), (1.2, "b".to_string())],
                    relation: TierRelation::Independent,
                })
            }
            97 => Some(Command::AttachAnnotation {
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
}
