//! The command surface of the engine journal.
//!
//! Every mutation of session state is one [`Command`] — audio lifecycle,
//! annotation attachment, tier lifecycle, and the boundary and label edits of
//! the annotation loop (design rule 5, `docs/plan/architecture.md`). Commands
//! are `serde`-serializable and self-describing so a later project layer can
//! persist a session's history; the journal itself stays in memory for now
//! (project-file persistence lands with `phx-project` in phase 4).
//!
//! Applying a command returns an [`Applied`] describing exactly what changed,
//! by stable id and span, so a frontend patches its view incrementally instead
//! of reloading the whole document.

use phx_annot::{
    AlignMode, Annotation, BoundaryId, BoundaryMove, Hit, LabelTarget, TierId, TierMerge,
    TierRelation,
};
use serde::{Deserialize, Serialize};

use crate::document::AnnotationId;
use crate::store::AudioId;

/// A single journaled mutation of engine state.
///
/// The redo stack is cleared whenever a command is applied, so a command never
/// coexists with a redo entry it could contradict.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Command {
    /// Decode a WAV buffer and add it to the audio store under `name`.
    ImportAudio {
        /// RIFF/WAVE bytes.
        bytes: Vec<u8>,
        /// Display name recorded on the decoded buffer.
        name: String,
    },
    /// Attach an annotation document to a stored audio buffer.
    AttachAnnotation {
        /// Audio buffer the document annotates.
        audio: AudioId,
        /// Annotation content; validated before it enters the store.
        annotation: Annotation,
    },
    /// Add an interval tier holding one unlabeled interval over the domain.
    AddIntervalTier {
        /// Target document.
        annotation: AnnotationId,
        /// Tier name.
        name: String,
        /// Relation to another tier.
        relation: TierRelation,
    },
    /// Add a point tier with sorted, strictly increasing point times.
    AddPointTier {
        /// Target document.
        annotation: AnnotationId,
        /// Tier name.
        name: String,
        /// `(time, label)` pairs in seconds.
        points: Vec<(f64, String)>,
        /// Relation to another tier.
        relation: TierRelation,
    },
    /// Remove a tier and everything it holds.
    RemoveTier {
        /// Target document.
        annotation: AnnotationId,
        /// Tier to remove.
        tier: TierId,
    },
    /// Split an interval by inserting a boundary at `at` seconds.
    InsertBoundary {
        /// Target document.
        annotation: AnnotationId,
        /// Tier to split.
        tier: TierId,
        /// Boundary time in seconds.
        at: f64,
    },
    /// Move a boundary to `to` seconds.
    MoveBoundary {
        /// Target document.
        annotation: AnnotationId,
        /// Boundary to move.
        boundary: BoundaryId,
        /// New boundary time in seconds.
        to: f64,
        /// Whether aligned peer boundaries move together.
        mode: AlignMode,
    },
    /// Remove an interior boundary, merging its two intervals.
    RemoveBoundary {
        /// Target document.
        annotation: AnnotationId,
        /// Boundary to remove.
        boundary: BoundaryId,
    },
    /// Replace an interval or point label.
    SetLabel {
        /// Target document.
        annotation: AnnotationId,
        /// Interval or point whose label changes.
        target: LabelTarget,
        /// New label text.
        text: String,
    },
}

/// What a successful [`Command`], `undo`, or `redo` changed.
///
/// Each variant names the affected object by stable id and, where a frontend
/// needs it to repaint, the spans that moved or merged.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Applied {
    /// A WAV buffer entered the audio store.
    AudioImported {
        /// New audio id.
        audio: AudioId,
    },
    /// An audio buffer left the store (an import undone).
    AudioRemoved {
        /// Audio id that was removed.
        audio: AudioId,
    },
    /// A document was attached to audio.
    AnnotationAttached {
        /// New annotation id.
        annotation: AnnotationId,
        /// Audio the document annotates.
        audio: AudioId,
    },
    /// A document was detached (an attachment undone).
    AnnotationDetached {
        /// Annotation id that was detached.
        annotation: AnnotationId,
    },
    /// A tier entered a document.
    TierAdded {
        /// Owning document.
        annotation: AnnotationId,
        /// New tier id.
        tier: TierId,
    },
    /// A tier left a document.
    TierRemoved {
        /// Owning document.
        annotation: AnnotationId,
        /// Tier id that was removed.
        tier: TierId,
    },
    /// A boundary was inserted, splitting one interval into two.
    BoundaryInserted {
        /// Owning document.
        annotation: AnnotationId,
        /// Tier the boundary splits.
        tier: TierId,
        /// New boundary id.
        boundary: BoundaryId,
        /// Boundary time in seconds.
        at: f64,
    },
    /// One or more boundaries moved.
    BoundaryMoved {
        /// Owning document.
        annotation: AnnotationId,
        /// Every boundary that moved, with its from/to times.
        moves: Vec<BoundaryMove>,
    },
    /// A boundary was removed, merging two intervals per affected tier.
    BoundaryRemoved {
        /// Owning document.
        annotation: AnnotationId,
        /// Boundary id that was removed.
        boundary: BoundaryId,
    },
    /// A boundary was restored, re-splitting a merged interval (an inverse of
    /// [`Applied::BoundaryRemoved`]).
    BoundaryRestored {
        /// Owning document.
        annotation: AnnotationId,
        /// Per-tier merges that were reversed.
        merges: Vec<TierMerge>,
    },
    /// An interval or point label changed.
    LabelSet {
        /// Owning document.
        annotation: AnnotationId,
        /// Interval or point whose label changed.
        target: LabelTarget,
        /// Label text now in place.
        text: String,
    },
}

/// A label search hit paired with the document it was found in.
///
/// [`phx_annot::Hit`] locates a match inside one annotation; the engine
/// searches every document, so it reports which one each hit came from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EngineHit {
    /// Document containing the match.
    pub annotation: AnnotationId,
    /// Match location within that document.
    pub hit: Hit,
}
