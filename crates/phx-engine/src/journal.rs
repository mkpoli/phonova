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
//!
//! Each entry also carries a journal-assigned id, separate from the
//! object-model ids above, so a caller that captured an id right after
//! applying a command — a delete's undo toast, say — can later ask
//! [`crate::Engine::journal_head_id`] whether that same entry is still what
//! [`crate::Engine::undo`] would target, or whether something else has been
//! journaled since.

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
    /// Set an audio buffer's display name.
    RenameAudio { id: AudioId, name: Option<String> },
    /// Detach an audio buffer, cascading to every document referencing it.
    DetachAudio { id: AudioId },
    /// Reattach a previously detached audio buffer and the documents that were
    /// cascaded off it, each under its original id.
    RestoreAudio {
        id: AudioId,
        docs: Vec<(AnnotationId, Document)>,
    },
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
            Self::RenameAudio { id, name } => {
                audio_store.set_name(*id, name.clone())?;
                Ok(Applied::AudioRenamed {
                    audio: *id,
                    name: name.clone().unwrap_or_default(),
                })
            }
            Self::DetachAudio { id } => {
                let annotations = documents.ids_referencing(*id);
                for annotation in &annotations {
                    documents.detach(*annotation);
                }
                audio_store.detach(*id);
                Ok(Applied::AudioDetached {
                    audio: *id,
                    annotations,
                })
            }
            Self::RestoreAudio { id, docs } => {
                audio_store.reattach_detached(*id);
                let mut annotations: Vec<AnnotationId> = docs
                    .iter()
                    .map(|(annotation, document)| {
                        documents.reattach(*annotation, document.clone());
                        *annotation
                    })
                    .collect();
                annotations.sort_unstable();
                Ok(Applied::AudioRestored {
                    audio: *id,
                    annotations,
                })
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
    let rebuilt = Annotation::from_raw(annotation.xmin(), annotation.xmax(), tiers)?;
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
    let rebuilt = Annotation::from_raw(annotation.xmin(), annotation.xmax(), tiers)?;
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
        InverseMutation::RemovePoint { point } => Applied::PointRemoved {
            annotation: doc,
            point: *point,
        },
        InverseMutation::RestorePoint { tier, point } => Applied::PointInserted {
            annotation: doc,
            tier: *tier,
            point: point.id,
            at: point.time,
        },
        InverseMutation::MovePoint { point, to } => Applied::PointMoved {
            annotation: doc,
            point: *point,
            to: *to,
        },
        InverseMutation::ReorderTier { tier, to_index } => Applied::TierReordered {
            annotation: doc,
            tier: *tier,
            to_index: *to_index,
        },
    }
}

/// A paired undo and redo transition, not yet stamped with a journal id.
///
/// `Engine::execute` and the direct-journaling import paths (raw WAV import,
/// streamed open, finished recording) build this; [`Journal::record`] stamps
/// it into a [`JournalEntry`] and pushes it.
pub(crate) struct Transition {
    pub(crate) undo: Reverse,
    pub(crate) redo: Reverse,
}

/// One reversible step, identified by a journal-assigned id so a caller that
/// captured the id at apply time can later tell whether this entry is still
/// the one [`Journal::take_undo`] would return — the check
/// [`crate::Engine::journal_head_id`] exists for.
pub(crate) struct JournalEntry {
    pub(crate) id: u64,
    pub(crate) undo: Reverse,
    pub(crate) redo: Reverse,
}

/// The unified undo/redo journal: two stacks of resolved transitions.
#[derive(Default)]
pub(crate) struct Journal {
    next_id: u64,
    undo_stack: Vec<JournalEntry>,
    redo_stack: Vec<JournalEntry>,
}

impl Journal {
    /// Records a freshly applied transition, drops any pending redo history,
    /// and returns the id stamped on the new entry.
    pub(crate) fn record(&mut self, transition: Transition) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.undo_stack.push(JournalEntry {
            id,
            undo: transition.undo,
            redo: transition.redo,
        });
        self.redo_stack.clear();
        id
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

    /// Id of the entry a call to [`Journal::take_undo`] would return right
    /// now, or `None` when there is nothing to undo.
    #[must_use]
    pub(crate) fn head_id(&self) -> Option<u64> {
        self.undo_stack.last().map(|entry| entry.id)
    }
}
