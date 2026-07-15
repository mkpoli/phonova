//! The filesystem abstraction the container and autosaver work through.
//!
//! Bytes-in / bytes-out is the whole surface: [`save`](crate::save) and
//! [`load`](crate::load) never touch storage, and everything that does — the
//! autosave sidecar, re-linking, recovery — goes through [`Storage`]. A native
//! build backs it with [`FsStore`] over `std::fs`; a web build implements the
//! same trait over OPFS. Paths are opaque strings the implementation defines.

use std::fmt;

/// Read/write access to a flat namespace of byte blobs keyed by path.
///
/// Directory separators inside a path are the implementation's business; this
/// crate joins with `/` (see [`join`]) and asks the implementation to list a
/// directory's immediate entries by name.
pub trait Storage {
    /// Error type the backing store reports.
    type Error: std::error::Error + 'static;

    /// Reads the whole file at `path`.
    fn read(&self, path: &str) -> Result<Vec<u8>, Self::Error>;

    /// Writes `bytes` to `path`, replacing any existing content.
    fn write(&self, path: &str, bytes: &[u8]) -> Result<(), Self::Error>;

    /// Reports whether a file exists at `path`.
    fn exists(&self, path: &str) -> bool;

    /// Removes the file at `path`; absence is not an error.
    fn remove(&self, path: &str) -> Result<(), Self::Error>;

    /// Lists the immediate file entries of `dir` by name (not full path).
    fn list_dir(&self, dir: &str) -> Result<Vec<String>, Self::Error>;
}

/// Joins a directory and a relative path with `/`.
///
/// An empty directory yields the relative path unchanged, so a project stored
/// beside its media at the namespace root resolves cleanly.
pub fn join(dir: &str, rel: &str) -> String {
    if dir.is_empty() {
        rel.to_string()
    } else {
        format!("{}/{}", dir.trim_end_matches('/'), rel)
    }
}

/// Returns the directory portion of a `/`-separated path, or `""` at the root.
pub fn parent_dir(path: &str) -> &str {
    match path.rfind('/') {
        Some(i) => &path[..i],
        None => "",
    }
}

/// An in-memory store for tests and for hosts that manage their own bytes.
#[derive(Debug, Default)]
pub struct MemStore {
    files: std::sync::Mutex<std::collections::BTreeMap<String, Vec<u8>>>,
}

impl MemStore {
    /// Creates an empty store.
    pub fn new() -> Self {
        Self::default()
    }
}

/// Error type for [`MemStore`]; a read of an absent path.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct MemStoreError(String);

impl fmt::Display for MemStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "no such entry: {}", self.0)
    }
}

impl std::error::Error for MemStoreError {}

impl Storage for MemStore {
    type Error = MemStoreError;

    fn read(&self, path: &str) -> Result<Vec<u8>, Self::Error> {
        self.files
            .lock()
            .unwrap()
            .get(path)
            .cloned()
            .ok_or_else(|| MemStoreError(path.to_string()))
    }

    fn write(&self, path: &str, bytes: &[u8]) -> Result<(), Self::Error> {
        self.files
            .lock()
            .unwrap()
            .insert(path.to_string(), bytes.to_vec());
        Ok(())
    }

    fn exists(&self, path: &str) -> bool {
        self.files.lock().unwrap().contains_key(path)
    }

    fn remove(&self, path: &str) -> Result<(), Self::Error> {
        self.files.lock().unwrap().remove(path);
        Ok(())
    }

    fn list_dir(&self, dir: &str) -> Result<Vec<String>, Self::Error> {
        let prefix = if dir.is_empty() {
            String::new()
        } else {
            format!("{}/", dir.trim_end_matches('/'))
        };
        let mut out = Vec::new();
        for key in self.files.lock().unwrap().keys() {
            if let Some(rest) = key.strip_prefix(&prefix) {
                // Only immediate entries, not files in nested directories.
                if !rest.is_empty() && !rest.contains('/') {
                    out.push(rest.to_string());
                }
            }
        }
        Ok(out)
    }
}

/// A `std::fs`-backed store rooted at a base directory. Native builds only.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone)]
pub struct FsStore {
    root: std::path::PathBuf,
}

#[cfg(not(target_arch = "wasm32"))]
impl FsStore {
    /// Roots a store at `root`; every path is resolved beneath it.
    pub fn new(root: impl Into<std::path::PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn resolve(&self, path: &str) -> std::path::PathBuf {
        let mut full = self.root.clone();
        for part in path.split('/').filter(|p| !p.is_empty()) {
            full.push(part);
        }
        full
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Storage for FsStore {
    type Error = std::io::Error;

    fn read(&self, path: &str) -> Result<Vec<u8>, Self::Error> {
        std::fs::read(self.resolve(path))
    }

    fn write(&self, path: &str, bytes: &[u8]) -> Result<(), Self::Error> {
        let full = self.resolve(path);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(full, bytes)
    }

    fn exists(&self, path: &str) -> bool {
        self.resolve(path).exists()
    }

    fn remove(&self, path: &str) -> Result<(), Self::Error> {
        match std::fs::remove_file(self.resolve(path)) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err),
        }
    }

    fn list_dir(&self, dir: &str) -> Result<Vec<String>, Self::Error> {
        let full = self.resolve(dir);
        let mut out = Vec::new();
        let entries = match std::fs::read_dir(&full) {
            Ok(entries) => entries,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(out),
            Err(err) => return Err(err),
        };
        for entry in entries {
            let entry = entry?;
            if entry.file_type()?.is_file()
                && let Some(name) = entry.file_name().to_str()
            {
                out.push(name.to_string());
            }
        }
        Ok(out)
    }
}
