//! Annotation documents held by the engine, keyed by [`AnnotationId`] and
//! attached to a stored audio buffer.
//!
//! A document is the annotation half of the session state the journal mutates.
//! The store mirrors [`crate::store::AudioStore`]: ids climb monotonically and
//! are never recycled, so an id undo has removed can always be restored to the
//! same slot on redo without ever colliding with a freshly attached document.

use std::collections::HashMap;

use phx_annot::Annotation;
use serde::{Deserialize, Serialize};

use crate::error::EngineError;
use crate::store::AudioId;

/// Opaque handle to an annotation document held by a [`DocumentStore`].
///
/// Ids are assigned in attachment order starting from zero and are never
/// reused within a store's lifetime. [`AnnotationId::as_u64`] and
/// [`AnnotationId::from_u64`] let a thin binding layer (`phx-wasm`) pass the id
/// across a boundary that only understands plain numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AnnotationId(u64);

impl AnnotationId {
    /// Returns the numeric handle backing this id.
    #[must_use]
    pub fn as_u64(self) -> u64 {
        self.0
    }

    /// Reconstructs an id from a numeric handle previously returned by
    /// [`AnnotationId::as_u64`].
    ///
    /// This does not check that the id refers to a live document; lookups with
    /// an id that was never issued, or that named a document since detached,
    /// return [`EngineError::UnknownAnnotationId`].
    #[must_use]
    pub fn from_u64(value: u64) -> Self {
        Self(value)
    }
}

/// An annotation document together with the audio buffer it annotates.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Document {
    /// Audio buffer this document annotates.
    pub audio: AudioId,
    /// Annotation content.
    pub annotation: Annotation,
}

/// In-memory store of annotation documents, keyed by [`AnnotationId`].
#[derive(Default)]
pub struct DocumentStore {
    next_id: u64,
    documents: HashMap<AnnotationId, Document>,
}

impl DocumentStore {
    /// Attaches a document to `audio` and returns its new id.
    pub fn attach(&mut self, audio: AudioId, annotation: Annotation) -> AnnotationId {
        let id = AnnotationId(self.next_id);
        self.next_id += 1;
        self.documents.insert(id, Document { audio, annotation });
        id
    }

    /// Reattaches a previously detached document under its original id.
    ///
    /// This is the redo path for an undone attachment: the id is the one the
    /// original [`DocumentStore::attach`] issued, so it is below `next_id` and
    /// no live document holds it.
    pub fn reattach(&mut self, id: AnnotationId, document: Document) {
        self.documents.insert(id, document);
    }

    /// Removes and returns the document for `id`, if present.
    ///
    /// The id is never recycled, so undo can restore it through
    /// [`DocumentStore::reattach`] without risk of a later collision.
    pub fn detach(&mut self, id: AnnotationId) -> Option<Document> {
        self.documents.remove(&id)
    }

    /// Returns a shared reference to the document for `id`.
    ///
    /// # Errors
    /// Returns [`EngineError::UnknownAnnotationId`] when `id` names no live
    /// document.
    pub fn get(&self, id: AnnotationId) -> Result<&Document, EngineError> {
        self.documents
            .get(&id)
            .ok_or(EngineError::UnknownAnnotationId(id))
    }

    /// Returns a mutable reference to the document for `id`.
    ///
    /// # Errors
    /// Returns [`EngineError::UnknownAnnotationId`] when `id` names no live
    /// document.
    pub fn get_mut(&mut self, id: AnnotationId) -> Result<&mut Document, EngineError> {
        self.documents
            .get_mut(&id)
            .ok_or(EngineError::UnknownAnnotationId(id))
    }

    /// Returns every live id in ascending order.
    ///
    /// The order is deterministic so label search and [`crate::Engine::state_hash`]
    /// fold documents the same way regardless of attachment history.
    #[must_use]
    pub fn ids_sorted(&self) -> Vec<AnnotationId> {
        let mut ids: Vec<AnnotationId> = self.documents.keys().copied().collect();
        ids.sort_unstable();
        ids
    }
}
