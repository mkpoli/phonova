//! The versioned ZIP container: `manifest.json`, `annotations/*.json`,
//! `profiles.json`, `view.json`.

use crate::media::{MediaId, MediaRef};
use crate::model::{LibraryNode, Profile, Project};
use phx_annot::Annotation;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use std::io::{Cursor, Read, Write};
use zip::result::ZipError;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

/// Format tag written into every manifest.
pub const FORMAT_TAG: &str = "phonix-project";

/// Current container schema version.
///
/// [`load`] accepts this version and lower, and refuses a higher one with
/// [`ProjectError::UnsupportedVersion`], so an older build reports a clear
/// error instead of silently dropping fields it cannot represent. Version 2
/// adds the library tree and descriptive metadata; version 1 files load with
/// those defaulted (see `docs/formats/project.md`).
pub const FORMAT_VERSION: u32 = 2;

const MANIFEST_ENTRY: &str = "manifest.json";
const PROFILES_ENTRY: &str = "profiles.json";
const VIEW_ENTRY: &str = "view.json";
const MEDIA_DIR: &str = "media/";

fn annotation_entry(id: MediaId) -> String {
    format!("annotations/{id}.json")
}

/// Entry name a self-contained bundle stores a recording's bytes under.
fn media_entry(id: MediaId) -> String {
    format!("{MEDIA_DIR}{id}.wav")
}

/// The `manifest.json` payload.
///
/// Fields serialize in declaration order. `description`, `authors`, `tags`, and
/// `groups` arrived in version 2; a version 1 manifest omits them and they
/// default here, so an old file loads without loss.
#[derive(Serialize, Deserialize)]
struct Manifest {
    format: String,
    version: u32,
    saved_at: u64,
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    authors: Vec<String>,
    #[serde(default)]
    tags: Vec<String>,
    media: Vec<MediaRef>,
    #[serde(default)]
    groups: Vec<LibraryNode>,
}

/// Serializes a project into the versioned ZIP container, references-only.
///
/// The byte stream is deterministic for a given project value: entries are
/// written in a fixed order, media in their vector order, and annotations in
/// media-id order. Media stays external — a recording is a `relative_path` plus
/// content hash, not embedded bytes. Use [`save_bundle`] to embed the media.
pub fn save(project: &Project) -> Vec<u8> {
    // Writing to an in-memory cursor never fails, so the byte-producing path is
    // infallible from the caller's view; only malformed input could fault, and
    // the project type makes that unrepresentable.
    save_inner(project, &[]).expect("in-memory container writing cannot fail")
}

/// Serializes a project as a self-contained bundle, embedding each recording's
/// bytes alongside the references-only container.
///
/// `media` supplies the bytes for the recordings to embed, keyed by
/// [`MediaId`]; an id absent from `media` is written references-only (its
/// `relative_path` still resolves against the filesystem on read). Embedded
/// entries land under `media/<id>.wav`, in ascending id order, after the
/// annotations, so the bundle serializes deterministically. Embedding is
/// orthogonal to the schema version: a reader that ignores the `media/` entries
/// reads the same version-2 project and falls back to re-linking, so a bundle
/// carries no version bump (see `docs/formats/project.md`).
pub fn save_bundle(project: &Project, media: &[(MediaId, Vec<u8>)]) -> Vec<u8> {
    save_inner(project, media).expect("in-memory container writing cannot fail")
}

fn save_inner(project: &Project, media: &[(MediaId, Vec<u8>)]) -> Result<Vec<u8>, ZipError> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    let manifest = Manifest {
        format: FORMAT_TAG.to_string(),
        version: FORMAT_VERSION,
        saved_at: project.saved_at,
        name: project.name.clone(),
        description: project.description.clone(),
        authors: project.authors.clone(),
        tags: project.tags.clone(),
        media: project.media.clone(),
        groups: project.groups.clone(),
    };

    writer.start_file(MANIFEST_ENTRY, options)?;
    writer.write_all(&to_json(&manifest))?;

    writer.start_file(PROFILES_ENTRY, options)?;
    writer.write_all(&to_json(&project.profiles))?;

    writer.start_file(VIEW_ENTRY, options)?;
    writer.write_all(&to_json(&project.view))?;

    for (id, annotation) in &project.annotations {
        writer.start_file(annotation_entry(*id), options)?;
        writer.write_all(&to_json(annotation))?;
    }

    // Embedded media closes the archive, in ascending id order so the bundle is
    // deterministic regardless of the order the host supplies the bytes.
    let mut embedded: Vec<&(MediaId, Vec<u8>)> = media.iter().collect();
    embedded.sort_by_key(|(id, _)| *id);
    for (id, bytes) in embedded {
        writer.start_file(media_entry(*id), options)?;
        writer.write_all(bytes)?;
    }

    Ok(writer.finish()?.into_inner())
}

fn to_json<T: Serialize>(value: &T) -> Vec<u8> {
    serde_json::to_vec_pretty(value).expect("project parts serialize to JSON")
}

/// Parses a versioned ZIP container back into a project.
pub fn load(bytes: &[u8]) -> Result<Project, ProjectError> {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(ProjectError::from_zip)?;

    let manifest: Manifest = read_json(&mut archive, MANIFEST_ENTRY)?
        .ok_or_else(|| ProjectError::MissingEntry(MANIFEST_ENTRY.to_string()))?;

    if manifest.format != FORMAT_TAG {
        return Err(ProjectError::UnknownFormat(manifest.format));
    }
    if manifest.version > FORMAT_VERSION {
        return Err(ProjectError::UnsupportedVersion {
            found: manifest.version,
            supported: FORMAT_VERSION,
        });
    }

    let profiles: Vec<Profile> = read_json(&mut archive, PROFILES_ENTRY)?.unwrap_or_default();
    let view: Value = read_json(&mut archive, VIEW_ENTRY)?.unwrap_or(Value::Null);

    let mut annotations = std::collections::BTreeMap::new();
    for media in &manifest.media {
        if let Some(annotation) =
            read_json::<Annotation>(&mut archive, &annotation_entry(media.id))?
        {
            annotations.insert(media.id, annotation);
        }
    }

    let mut project = Project {
        name: manifest.name,
        saved_at: manifest.saved_at,
        description: manifest.description,
        authors: manifest.authors,
        tags: manifest.tags,
        media: manifest.media,
        groups: manifest.groups,
        annotations,
        profiles,
        view,
    };
    // The tree is an organizational overlay on the flat media list. Repair it so
    // every recording appears exactly once: a v1 file arrives with no tree and
    // lays out flat in manifest order; a v2 file with a dangling or duplicate
    // leaf is corrected rather than rejected (see docs/formats/project.md).
    project.normalize_library();
    Ok(project)
}

/// Extracts the media a self-contained bundle embedded, keyed by [`MediaId`].
///
/// Returns the recordings stored under `media/<id>.wav`, ascending by id, each
/// with the exact bytes the bundle carried. A references-only container has no
/// such entries and yields an empty vector, which is how a caller tells a
/// self-contained bundle from a references-only one. Entries whose name does not
/// match `media/<id>.wav` are ignored.
pub fn load_embedded_media(bytes: &[u8]) -> Result<Vec<(MediaId, Vec<u8>)>, ProjectError> {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(ProjectError::from_zip)?;
    let mut media = Vec::new();
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(ProjectError::from_zip)?;
        let name = file.name().to_string();
        let Some(id) = parse_media_entry(&name) else {
            continue;
        };
        let mut buf = Vec::with_capacity(file.size() as usize);
        std::io::Read::read_to_end(&mut file, &mut buf)
            .map_err(|err| ProjectError::Container(err.to_string()))?;
        media.push((id, buf));
    }
    media.sort_by_key(|(id, _)| *id);
    Ok(media)
}

/// Parses `media/<id>.wav` into its [`MediaId`], or `None` for any other name.
fn parse_media_entry(name: &str) -> Option<MediaId> {
    let rest = name.strip_prefix(MEDIA_DIR)?.strip_suffix(".wav")?;
    rest.parse::<u64>().ok().map(MediaId::new)
}

fn read_json<T: for<'de> Deserialize<'de>>(
    archive: &mut ZipArchive<Cursor<&[u8]>>,
    entry: &str,
) -> Result<Option<T>, ProjectError> {
    let mut file = match archive.by_name(entry) {
        Ok(file) => file,
        Err(ZipError::FileNotFound) => return Ok(None),
        Err(err) => return Err(ProjectError::from_zip(err)),
    };
    let mut text = String::new();
    file.read_to_string(&mut text)
        .map_err(|err| ProjectError::Container(err.to_string()))?;
    let value = serde_json::from_str(&text).map_err(|err| ProjectError::MalformedEntry {
        entry: entry.to_string(),
        detail: err.to_string(),
    })?;
    Ok(Some(value))
}

/// Why a project could not be loaded.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectError {
    /// The bytes are not a readable ZIP container, or an entry could not be read.
    Container(String),
    /// A required entry was absent from the container.
    MissingEntry(String),
    /// An entry held JSON that did not match the expected schema.
    MalformedEntry {
        /// The entry name.
        entry: String,
        /// Parser detail.
        detail: String,
    },
    /// The manifest's format tag was not this crate's.
    UnknownFormat(String),
    /// The container was written by a newer schema version than this build reads.
    UnsupportedVersion {
        /// The version found in the manifest.
        found: u32,
        /// The highest version this build reads.
        supported: u32,
    },
}

impl ProjectError {
    fn from_zip(err: ZipError) -> Self {
        Self::Container(err.to_string())
    }
}

impl fmt::Display for ProjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Container(detail) => write!(f, "not a readable project container: {detail}"),
            Self::MissingEntry(entry) => write!(f, "container is missing {entry}"),
            Self::MalformedEntry { entry, detail } => {
                write!(f, "{entry} is malformed: {detail}")
            }
            Self::UnknownFormat(tag) => write!(f, "unknown container format tag: {tag}"),
            Self::UnsupportedVersion { found, supported } => write!(
                f,
                "container version {found} is newer than supported version {supported}"
            ),
        }
    }
}

impl std::error::Error for ProjectError {}
