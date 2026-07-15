//! Debounced autosave snapshots and crash recovery.
//!
//! The autosaver holds no timer and spawns no thread: it is a debounce state
//! machine the host drives from its own event loop, so it never blocks the
//! engine. The host reports edits with [`Autosaver::note_change`], asks
//! [`Autosaver::due`] whenever convenient (e.g. on a coarse tick), and calls
//! [`Autosaver::flush`] when a write is due. A flush writes a full snapshot to
//! a sidecar next to the project file. On the web the same calls write the
//! sidecar to OPFS through the host's [`Storage`] implementation.
//!
//! Recovery does not depend on filesystem timestamps. Each snapshot embeds the
//! millisecond it was written ([`Project::saved_at`]); [`detect_recovery`]
//! compares the sidecar's stamp against the project file's and offers the newer
//! one, which behaves identically on native storage and OPFS.

use crate::container::{ProjectError, load, save};
use crate::model::Project;
use crate::storage::Storage;

/// Suffix appended to a project path to name its autosave sidecar.
pub const AUTOSAVE_SUFFIX: &str = ".autosave";

/// Default quiet period after the last edit before a snapshot is due, in ms.
pub const DEFAULT_DEBOUNCE_MS: u64 = 2_000;

/// Default ceiling on how long unbroken editing defers a snapshot, in ms.
pub const DEFAULT_MAX_WAIT_MS: u64 = 15_000;

/// Returns the sidecar path for a project path.
pub fn autosave_path(project_path: &str) -> String {
    format!("{project_path}{AUTOSAVE_SUFFIX}")
}

/// A debounce state machine that writes autosave snapshots to a sidecar.
#[derive(Debug, Clone)]
pub struct Autosaver {
    project_path: String,
    sidecar_path: String,
    debounce_ms: u64,
    max_wait_ms: u64,
    first_pending: Option<u64>,
    last_change: u64,
}

impl Autosaver {
    /// Creates an autosaver for `project_path` with the default timing.
    pub fn new(project_path: impl Into<String>) -> Self {
        let project_path = project_path.into();
        let sidecar_path = autosave_path(&project_path);
        Self {
            project_path,
            sidecar_path,
            debounce_ms: DEFAULT_DEBOUNCE_MS,
            max_wait_ms: DEFAULT_MAX_WAIT_MS,
            first_pending: None,
            last_change: 0,
        }
    }

    /// Overrides the debounce quiet period and maximum deferral, in ms.
    pub fn with_timing(mut self, debounce_ms: u64, max_wait_ms: u64) -> Self {
        self.debounce_ms = debounce_ms;
        self.max_wait_ms = max_wait_ms;
        self
    }

    /// The project file this autosaver guards.
    pub fn project_path(&self) -> &str {
        &self.project_path
    }

    /// The sidecar path snapshots are written to.
    pub fn sidecar_path(&self) -> &str {
        &self.sidecar_path
    }

    /// Whether a snapshot is currently pending (edits since the last flush).
    pub fn is_pending(&self) -> bool {
        self.first_pending.is_some()
    }

    /// Records that the project changed at `now_ms` (milliseconds since epoch).
    pub fn note_change(&mut self, now_ms: u64) {
        self.first_pending.get_or_insert(now_ms);
        self.last_change = now_ms;
    }

    /// Reports whether a snapshot is due at `now_ms`.
    ///
    /// A snapshot is due once editing has been quiet for the debounce period,
    /// or once the oldest pending edit has waited past the maximum, so a steady
    /// stream of edits still gets checkpointed.
    pub fn due(&self, now_ms: u64) -> bool {
        match self.first_pending {
            None => false,
            Some(first) => {
                let quiet = now_ms.saturating_sub(self.last_change) >= self.debounce_ms;
                let waited = now_ms.saturating_sub(first) >= self.max_wait_ms;
                quiet || waited
            }
        }
    }

    /// Writes a snapshot of `project` to the sidecar, stamped at `now_ms`.
    ///
    /// The written snapshot carries `saved_at = now_ms` so recovery can compare
    /// it against the project file. Clears the pending state on success.
    pub fn flush<S: Storage>(
        &mut self,
        store: &S,
        project: &Project,
        now_ms: u64,
    ) -> Result<(), S::Error> {
        let mut snapshot = project.clone();
        snapshot.saved_at = now_ms;
        store.write(&self.sidecar_path, &save(&snapshot))?;
        self.first_pending = None;
        Ok(())
    }

    /// Removes the sidecar and clears pending state.
    ///
    /// The host calls this after an explicit save promotes the project file, so
    /// a stale sidecar never triggers a spurious recovery prompt.
    pub fn discard<S: Storage>(&mut self, store: &S) -> Result<(), S::Error> {
        store.remove(&self.sidecar_path)?;
        self.first_pending = None;
        Ok(())
    }
}

/// The recovery situation found for a project on open.
#[derive(Debug, Clone, PartialEq)]
pub enum Recovery {
    /// No sidecar is present; nothing to recover.
    None,
    /// A sidecar exists but is not newer than the project file.
    UpToDate,
    /// A sidecar newer than the project file holds unsaved work.
    Recoverable(Box<Project>),
}

/// Inspects a project path and its sidecar for recoverable unsaved work.
///
/// Reads the sidecar and, when present, the project file, comparing their
/// [`Project::saved_at`] stamps. A sidecar strictly newer than the project file
/// — or a sidecar with no project file beside it — is [`Recovery::Recoverable`].
pub fn detect_recovery<S: Storage>(
    store: &S,
    project_path: &str,
) -> Result<Recovery, ProjectError> {
    let sidecar_path = autosave_path(project_path);
    if !store.exists(&sidecar_path) {
        return Ok(Recovery::None);
    }

    let sidecar_bytes = store
        .read(&sidecar_path)
        .map_err(|err| ProjectError::Container(err.to_string()))?;
    let sidecar = load(&sidecar_bytes)?;

    if !store.exists(project_path) {
        return Ok(Recovery::Recoverable(Box::new(sidecar)));
    }

    let project_bytes = store
        .read(project_path)
        .map_err(|err| ProjectError::Container(err.to_string()))?;
    let project = load(&project_bytes)?;

    if sidecar.saved_at > project.saved_at {
        Ok(Recovery::Recoverable(Box::new(sidecar)))
    } else {
        Ok(Recovery::UpToDate)
    }
}
