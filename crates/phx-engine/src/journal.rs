//! The unified undo journal.
//!
//! Every applied [`crate::Command`] pushes one [`JournalEntry`] holding two
//! fully resolved, id-stable transitions: an `undo` that returns the state to
//! exactly what it was, and a `redo` that reproduces the change. Both are
//! expressed against stable ids captured at apply time, never by replaying the
//! original mutator — replay would re-run `phx-annot`'s monotonic id
//! allocators and hand back different ids, breaking the state-hash-identity
//! invariant (`docs/plan/validation.md` invariant 5).
//!
//! Content edits (boundaries, labels) reduce to a [`phx_annot::InverseMutation`]
//! in both directions, applied through [`phx_annot::Annotation::apply_inverse`]:
//! that method's four variants — remove, move, restore-merge, set-label — form
//! a complete, id-stable basis for undoing and redoing every content command.
//! Object lifecycle (audio import, document attachment, tier add/remove) is
//! handled by the engine directly, since it lives above the annotation model.
//!
//! Depth is unlimited: entries are only ever appended and moved between the two
//! stacks, and applying a fresh command clears the redo stack so an entry never
//! contradicts a command applied after it.

use phx_annot::{Annotation, InverseMutation, TierId, TierSlot};

use crate::commands::Applied;
use crate::document::{AnnotationId, Document, DocumentStore};
use crate::error::EngineError;
use crate::store::{AudioId, AudioStore};
use phx_audio::Audio;

/// A resolved, id-stable state transition in one direction.
pub(crate) enum Reverse {
    /// Reinsert a previously removed audio buffer under its original id.
    ImportAudio { id: AudioId, audio: Box<Audio> },
    /// Remove an audio buffer.
    RemoveAudio { id: AudioId },
    /// Reattach a previously detached document under its original id.
    Attach {
        id: AnnotationId,
        document: Box<Document>,
    },
    /// Detach a document.
    Detach { id: AnnotationId },
    /// Reinstate a tier at a fixed position, restoring every stable id it held.
    InsertTier {
        doc: AnnotationId,
        index: usize,
        slot: Box<TierSlot>,
    },
    /// Remove a tier.
    RemoveTier { doc: AnnotationId, tier: TierId },
    /// Apply an id-stable content inverse to a document.
    Content {
        doc: AnnotationId,
        mutation: InverseMutation,
    },
}

impl Reverse {
    /// Applies this transition to the engine's stores and reports what changed.
    pub(crate) fn apply(
        &self,
        audio_store: &mut AudioStore,
        documents: &mut DocumentStore,
    ) -> Result<Applied, EngineError> {
        match self {
            Self::ImportAudio { id, audio } => {
                audio_store.restore(*id, (**audio).clone());
                Ok(Applied::AudioImported { audio: *id })
            }
            Self::RemoveAudio { id } => {
                audio_store.remove(*id);
                Ok(Applied::AudioRemoved { audio: *id })
            }
            Self::Attach { id, document } => {
                documents.reattach(*id, (**document).clone());
                Ok(Applied::AnnotationAttached {
                    annotation: *id,
                    audio: document.audio,
                })
            }
            Self::Detach { id } => {
                documents.detach(*id);
                Ok(Applied::AnnotationDetached { annotation: *id })
            }
            Self::InsertTier { doc, index, slot } => {
                let document = documents.get_mut(*doc)?;
                let restored = insert_tier(&document.annotation, *index, (**slot).clone())?;
                document.annotation = restored;
                Ok(Applied::TierAdded {
                    annotation: *doc,
                    tier: slot.id,
                })
            }
            Self::RemoveTier { doc, tier } => {
                let document = documents.get_mut(*doc)?;
                let reduced = remove_tier(&document.annotation, *tier)?;
                document.annotation = reduced;
                Ok(Applied::TierRemoved {
                    annotation: *doc,
                    tier: *tier,
                })
            }
            Self::Content { doc, mutation } => {
                let document = documents.get_mut(*doc)?;
                document.annotation.apply_inverse(mutation)?;
                Ok(applied_for_content(*doc, mutation))
            }
        }
    }
}

/// Rebuilds a document with `slot` reinserted at `index`, keeping every other
/// tier and every stable id untouched, then rejects the result if it fails
/// validation.
pub(crate) fn insert_tier(
    annotation: &Annotation,
    index: usize,
    slot: TierSlot,
) -> Result<Annotation, EngineError> {
    let mut tiers = annotation.tiers().to_vec();
    let index = index.min(tiers.len());
    tiers.insert(index, slot);
    let rebuilt = Annotation::from_raw(annotation.xmin(), annotation.xmax(), tiers);
    reject_invalid(rebuilt)
}

/// Rebuilds a document with `tier` removed, keeping every other tier and stable
/// id untouched, then rejects the result if it fails validation (a dangling
/// aligned or child relation, for instance).
pub(crate) fn remove_tier(
    annotation: &Annotation,
    tier: TierId,
) -> Result<Annotation, EngineError> {
    if annotation.tier(tier).is_none() {
        return Err(EngineError::Annotation(
            phx_annot::AnnotationError::UnknownTier { tier },
        ));
    }
    let tiers: Vec<TierSlot> = annotation
        .tiers()
        .iter()
        .filter(|slot| slot.id != tier)
        .cloned()
        .collect();
    let rebuilt = Annotation::from_raw(annotation.xmin(), annotation.xmax(), tiers);
    reject_invalid(rebuilt)
}

/// Returns the position of `tier` in document order.
pub(crate) fn tier_index(annotation: &Annotation, tier: TierId) -> Option<usize> {
    annotation.tiers().iter().position(|slot| slot.id == tier)
}

fn reject_invalid(annotation: Annotation) -> Result<Annotation, EngineError> {
    let issues = annotation.validate();
    if issues.is_empty() {
        Ok(annotation)
    } else {
        Err(EngineError::InvalidAnnotation(issues))
    }
}

/// Describes the effect of a content inverse for a frontend patch.
///
/// The [`InverseMutation`] value carries enough to name the affected ids
/// without re-reading the document, so undo and redo report the same shape of
/// change that the forward command did.
fn applied_for_content(doc: AnnotationId, mutation: &InverseMutation) -> Applied {
    match mutation {
        InverseMutation::RemoveBoundary { boundary } => Applied::BoundaryRemoved {
            annotation: doc,
            boundary: *boundary,
        },
        InverseMutation::MoveBoundaries { moves } => {
            // `apply_inverse` drives each boundary from its stored `to` back to
            // its stored `from`; report that real motion, not the stored order.
            let moves = moves
                .iter()
                .map(|m| phx_annot::BoundaryMove {
                    tier: m.tier,
                    boundary: m.boundary,
                    from: m.to,
                    to: m.from,
                })
                .collect();
            Applied::BoundaryMoved {
                annotation: doc,
                moves,
            }
        }
        InverseMutation::RestoreMergedBoundary { merged } => Applied::BoundaryRestored {
            annotation: doc,
            merges: merged.merges.clone(),
        },
        InverseMutation::SetLabel { target, text } => Applied::LabelSet {
            annotation: doc,
            target: *target,
            text: text.clone(),
        },
    }
}

/// One reversible step: paired undo and redo transitions.
pub(crate) struct JournalEntry {
    pub(crate) undo: Reverse,
    pub(crate) redo: Reverse,
}

/// The unified undo/redo journal: two stacks of resolved transitions.
#[derive(Default)]
pub(crate) struct Journal {
    undo_stack: Vec<JournalEntry>,
    redo_stack: Vec<JournalEntry>,
}

impl Journal {
    /// Records a freshly applied command and drops any pending redo history.
    pub(crate) fn record(&mut self, entry: JournalEntry) {
        self.undo_stack.push(entry);
        self.redo_stack.clear();
    }

    /// Pops the most recent entry for undo, moving it onto the redo stack.
    pub(crate) fn take_undo(&mut self) -> Option<JournalEntry> {
        self.undo_stack.pop()
    }

    /// Returns an undone entry to the redo stack.
    pub(crate) fn park_redo(&mut self, entry: JournalEntry) {
        self.redo_stack.push(entry);
    }

    /// Pops the most recent undone entry for redo, moving it back to the undo
    /// stack.
    pub(crate) fn take_redo(&mut self) -> Option<JournalEntry> {
        self.redo_stack.pop()
    }

    /// Returns a redone entry to the undo stack.
    pub(crate) fn park_undo(&mut self, entry: JournalEntry) {
        self.undo_stack.push(entry);
    }

    /// Number of entries that can still be undone.
    #[must_use]
    pub(crate) fn undo_depth(&self) -> usize {
        self.undo_stack.len()
    }

    /// Number of entries that can still be redone.
    #[must_use]
    pub(crate) fn redo_depth(&self) -> usize {
        self.redo_stack.len()
    }
}
