//! The in-memory project and its parts.

use crate::media::{GroupId, MediaId, MediaRef};
use phx_annot::Annotation;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// A complete editing session: recordings, their annotations, named analysis
/// profiles, and an opaque view-state blob.
///
/// The project is the unit that [`crate::save`] and [`crate::load`] round-trip.
/// Loading a saved project reproduces the same recordings, per-recording
/// annotations, parameter profiles, and view state that were present when it
/// was saved.
///
/// Analysis parameters and view state are carried as [`serde_json::Value`]
/// blobs owned by the caller. The project persists and returns them unchanged;
/// it does not depend on the pitch, formant, or rendering crates that define
/// their shape.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Project {
    /// Human-facing project name.
    pub name: String,
    /// Milliseconds since the Unix epoch when this state was last written.
    ///
    /// Recovery compares this value between a project file and its autosave
    /// sidecar to decide which holds the newer state.
    pub saved_at: u64,
    /// Free-form description of the project. Empty when unset.
    pub description: String,
    /// Contributors credited for the project, in listing order.
    pub authors: Vec<String>,
    /// Free-form tags applied to the project, in listing order.
    pub tags: Vec<String>,
    /// Referenced recordings, in a stable presentation order.
    ///
    /// This is the flat source of truth for which recordings the project holds.
    /// [`groups`](Self::groups) organizes the same recordings into a tree by
    /// reference; a reader that ignores the tree still sees every recording here.
    pub media: Vec<MediaRef>,
    /// The library tree: the ordered root of a nesting of groups and recordings.
    ///
    /// Every recording in [`media`](Self::media) appears as exactly one leaf
    /// somewhere in this tree; [`normalize_library`](Self::normalize_library)
    /// restores that invariant after edits, and [`crate::load`] applies it on
    /// read.
    pub groups: Vec<LibraryNode>,
    /// Annotations keyed by the recording they belong to.
    pub annotations: BTreeMap<MediaId, Annotation>,
    /// Named parameter profiles (e.g. per speaker), each an opaque blob.
    pub profiles: Vec<Profile>,
    /// Opaque view state owned by the UI (zoom span, palette, visible tracks).
    pub view: Value,
}

impl Project {
    /// Creates an empty project with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }

    /// Returns the reference for a recording, if present.
    pub fn media_ref(&self, id: MediaId) -> Option<&MediaRef> {
        self.media.iter().find(|m| m.id == id)
    }

    /// Media ids as the library tree lists them, depth-first in display order.
    pub fn library_media_ids(&self) -> Vec<MediaId> {
        let mut out = Vec::new();
        collect_media_ids(&self.groups, &mut out);
        out
    }

    /// Repairs the library tree so every recording appears exactly once.
    ///
    /// Walking [`groups`](Self::groups) in order, the first leaf naming each
    /// recording present in [`media`](Self::media) is kept. A leaf naming a
    /// recording absent from `media`, and any further leaf for a recording
    /// already kept, is dropped. Recordings in `media` that no leaf names are
    /// appended to the root in `media` order. Groups are preserved even when
    /// empty. The result satisfies the tree-integrity invariant and applying it
    /// again changes nothing.
    ///
    /// A v1 file loads with no tree, so this lays every recording out flat at
    /// the root in manifest order.
    pub fn normalize_library(&mut self) {
        let known: BTreeSet<MediaId> = self.media.iter().map(|m| m.id).collect();
        let mut seen: BTreeSet<MediaId> = BTreeSet::new();
        retain_media_leaves(&mut self.groups, &known, &mut seen);
        for media in &self.media {
            if seen.insert(media.id) {
                self.groups.push(LibraryNode::Media(media.id));
            }
        }
    }
}

fn collect_media_ids(nodes: &[LibraryNode], out: &mut Vec<MediaId>) {
    for node in nodes {
        match node {
            LibraryNode::Media(id) => out.push(*id),
            LibraryNode::Group(group) => collect_media_ids(&group.children, out),
        }
    }
}

fn retain_media_leaves(
    nodes: &mut Vec<LibraryNode>,
    known: &BTreeSet<MediaId>,
    seen: &mut BTreeSet<MediaId>,
) {
    nodes.retain_mut(|node| match node {
        LibraryNode::Media(id) => known.contains(id) && seen.insert(*id),
        LibraryNode::Group(group) => {
            retain_media_leaves(&mut group.children, known, seen);
            true
        }
    });
}

/// A node in the library tree: either a recording leaf or a nested group.
///
/// A `Media` node references a recording by its [`MediaId`] from
/// [`Project::media`]; the recording itself lives in that flat list, not in the
/// tree. A `Group` node carries its own children, so the tree nests to any
/// depth.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum LibraryNode {
    /// A recording, referenced by id.
    Media(MediaId),
    /// A group with its own ordered children.
    Group(Group),
}

/// A named group of library nodes.
///
/// Groups organize recordings for display; they nest by holding child nodes,
/// which may themselves be groups. A group owns no recording — its media
/// children reference [`Project::media`] by id.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Group {
    /// Identifier for this group within the project.
    pub id: GroupId,
    /// Display name of the group.
    pub name: String,
    /// Ordered child nodes, groups and recordings intermixed.
    pub children: Vec<LibraryNode>,
}

impl Group {
    /// Creates an empty group with the given id and name.
    pub fn new(id: GroupId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            children: Vec::new(),
        }
    }
}

/// A named set of analysis parameters, stored as an opaque blob.
///
/// The name is what the toolbar shows and switches between; `params` is the
/// caller's serialized parameter set (pitch floor/ceiling, formant ceiling,
/// palette, and so on), which this crate stores and returns without inspecting.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    /// Display name of the profile.
    pub name: String,
    /// Opaque serialized parameters.
    pub params: Value,
}
