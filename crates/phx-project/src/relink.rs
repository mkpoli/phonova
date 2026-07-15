//! Resolving external media against the filesystem, with hash-based re-linking.

use crate::media::{ContentHash, MediaCandidate, MediaGap, MediaRef, MediaResolution};
use crate::model::Project;
use crate::storage::{Storage, join, parent_dir};

/// Resolves every recording in `project` against storage, re-linking moved files.
///
/// `project_path` is where the project file lives; media paths are relative to
/// its directory. For each recording:
///
/// - if the referenced file is present and its hash matches, it is [`Present`];
/// - otherwise every directory in `search_dirs` is scanned for a byte-identical
///   file. Exactly one match re-links the reference in place ([`Relinked`]);
/// - zero matches, or more than one, yield a [`MediaGap`] for the caller to
///   resolve with the user. An ambiguous gap carries the matching candidates.
///
/// Returns the per-recording outcomes when all resolve, or the gaps otherwise.
///
/// [`Present`]: MediaResolution::Present
/// [`Relinked`]: MediaResolution::Relinked
pub fn resolve_media<S: Storage>(
    project: &mut Project,
    project_path: &str,
    search_dirs: &[&str],
    store: &S,
) -> Result<Vec<MediaResolution>, Vec<MediaGap>> {
    let base = parent_dir(project_path).to_string();
    let mut resolutions = Vec::new();
    let mut gaps = Vec::new();

    for media in &mut project.media {
        match resolve_one(media, &base, search_dirs, store) {
            Ok(resolution) => resolutions.push(resolution),
            Err(gap) => gaps.push(gap),
        }
    }

    if gaps.is_empty() {
        Ok(resolutions)
    } else {
        Err(gaps)
    }
}

fn resolve_one<S: Storage>(
    media: &mut MediaRef,
    base: &str,
    search_dirs: &[&str],
    store: &S,
) -> Result<MediaResolution, MediaGap> {
    let current = join(base, &media.relative_path);
    if let Ok(bytes) = store.read(&current)
        && ContentHash::of(&bytes) == media.hash
    {
        return Ok(MediaResolution::Present(media.id));
    }

    let candidates = scan_candidates(media.hash, base, search_dirs, store);
    match candidates.as_slice() {
        [only] => {
            let from = std::mem::replace(&mut media.relative_path, only.relative_path.clone());
            Ok(MediaResolution::Relinked {
                media: media.id,
                from,
                to: media.relative_path.clone(),
            })
        }
        _ => Err(MediaGap {
            media: media.id,
            original_path: media.relative_path.clone(),
            expected_hash: media.hash,
            candidates,
        }),
    }
}

/// Lists files under `search_dirs` whose content hash equals `expected`.
///
/// Paths are returned relative to `base` so they can be dropped straight into a
/// [`MediaRef::relative_path`].
fn scan_candidates<S: Storage>(
    expected: ContentHash,
    base: &str,
    search_dirs: &[&str],
    store: &S,
) -> Vec<MediaCandidate> {
    let mut matches = Vec::new();
    for dir in search_dirs {
        let Ok(names) = store.list_dir(dir) else {
            continue;
        };
        for name in names {
            let full = join(dir, &name);
            let Ok(bytes) = store.read(&full) else {
                continue;
            };
            let hash = ContentHash::of(&bytes);
            if hash == expected {
                matches.push(MediaCandidate {
                    relative_path: relative_to(base, &full),
                    hash,
                });
            }
        }
    }
    matches
}

/// Expresses `full` relative to `base`, or returns it unchanged when it is not
/// beneath `base`.
fn relative_to(base: &str, full: &str) -> String {
    if base.is_empty() {
        return full.to_string();
    }
    let prefix = format!("{}/", base.trim_end_matches('/'));
    full.strip_prefix(&prefix).unwrap_or(full).to_string()
}
