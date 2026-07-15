//! The versioned ZIP container: `manifest.json`, `annotations/*.json`,
//! `profiles.json`, `view.json`.

use crate::media::{MediaId, MediaRef};
use crate::model::{Profile, Project};
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
/// [`load`] accepts this version and refuses a higher one with
/// [`ProjectError::UnsupportedVersion`], so an older build reports a clear
/// error instead of silently dropping fields it cannot represent.
pub const FORMAT_VERSION: u32 = 1;

const MANIFEST_ENTRY: &str = "manifest.json";
const PROFILES_ENTRY: &str = "profiles.json";
const VIEW_ENTRY: &str = "view.json";

fn annotation_entry(id: MediaId) -> String {
    format!("annotations/{id}.json")
}

/// The `manifest.json` payload.
#[derive(Serialize, Deserialize)]
struct Manifest {
    format: String,
    version: u32,
    saved_at: u64,
    name: String,
    media: Vec<MediaRef>,
}

/// Serializes a project into the versioned ZIP container.
///
/// The byte stream is deterministic for a given project value: entries are
/// written in a fixed order, media in their vector order, and annotations in
/// media-id order.
pub fn save(project: &Project) -> Vec<u8> {
    // Writing to an in-memory cursor never fails, so the byte-producing path is
    // infallible from the caller's view; only malformed input could fault, and
    // the project type makes that unrepresentable.
    save_inner(project).expect("in-memory container writing cannot fail")
}

fn save_inner(project: &Project) -> Result<Vec<u8>, ZipError> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    let manifest = Manifest {
        format: FORMAT_TAG.to_string(),
        version: FORMAT_VERSION,
        saved_at: project.saved_at,
        name: project.name.clone(),
        media: project.media.clone(),
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

    Ok(Project {
        name: manifest.name,
        saved_at: manifest.saved_at,
        media: manifest.media,
        annotations,
        profiles,
        view,
    })
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
