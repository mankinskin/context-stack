//! Persistence layer for workspace storage.
//!
//! Handles the on-disk layout, serialization (bincode for graphs, JSON for
//! metadata), atomic writes, and file locking (fs2) for multi-reader /
//! single-writer concurrency.
//!
//! ## Directory Layout
//!
//! ```text
//! .context-engine/<workspace-name>/
//! ├── graph.bin       — bincode-serialized Hypergraph<BaseGraphKind>
//! ├── metadata.json   — serde_json WorkspaceMetadata
//! └── .lock           — file lock (fs2)
//! ```

use std::{
    fs,
    path::{
        Path,
        PathBuf,
    },
};

use context_trace::graph::{
    kind::BaseGraphKind,
    Hypergraph,
};
use fs2::FileExt;

use crate::{
    error::WorkspaceError,
    workspace::metadata::WorkspaceMetadata,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Top-level directory name (created inside the project root / base dir).
pub const CONTEXT_DIR: &str = ".context-engine";

/// Bincode-serialized hypergraph file.
pub const GRAPH_FILE: &str = "graph.bin";

/// Human-readable workspace metadata (JSON).
pub const METADATA_FILE: &str = "metadata.json";

/// Lock file used by `fs2` for advisory locking.
pub const LOCK_FILE: &str = ".lock";

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

/// Return the directory path for a named workspace.
///
/// `<base>/.context-engine/<name>/`
pub fn workspace_dir(
    base: &Path,
    name: &str,
) -> PathBuf {
    base.join(CONTEXT_DIR).join(name)
}

/// Check whether a workspace with the given name exists on disk.
///
/// We test for the presence of `metadata.json` rather than just the directory,
/// so partially-created workspaces (e.g. only the dir exists) are not
/// considered valid.
pub fn workspace_exists(
    base: &Path,
    name: &str,
) -> bool {
    workspace_dir(base, name).join(METADATA_FILE).exists()
}

/// List the names of all workspaces found under `<base>/.context-engine/`.
///
/// Only directories that contain a `metadata.json` are included. Results are
/// returned in sorted order.
pub fn list_workspace_names(
    base: &Path
) -> Result<Vec<String>, WorkspaceError> {
    let ctx_dir = base.join(CONTEXT_DIR);
    if !ctx_dir.exists() {
        return Ok(Vec::new());
    }

    let mut names = Vec::new();
    let entries = fs::read_dir(&ctx_dir).map_err(WorkspaceError::IoError)?;

    for entry in entries {
        let entry = entry.map_err(WorkspaceError::IoError)?;
        let path = entry.path();
        if path.is_dir() && path.join(METADATA_FILE).exists() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                names.push(name.to_string());
            }
        }
    }

    names.sort();
    Ok(names)
}

// ---------------------------------------------------------------------------
// Graph persistence (bincode)
// ---------------------------------------------------------------------------

/// Serialize a hypergraph to `graph.bin` inside the given workspace directory.
///
/// Uses atomic write: data is first written to a `.tmp` file and then renamed,
/// so a crash mid-write won't corrupt the existing file.
pub fn save_graph(
    dir: &Path,
    graph: &Hypergraph<BaseGraphKind>,
) -> Result<(), WorkspaceError> {
    let bytes = bincode::serialize(graph).map_err(|e| {
        WorkspaceError::SerializationError(format!("bincode serialize: {e}"))
    })?;

    atomic_write(&dir.join(GRAPH_FILE), &bytes)
}

/// Deserialize a hypergraph from `graph.bin` inside the given workspace directory.
pub fn load_graph(
    dir: &Path
) -> Result<Hypergraph<BaseGraphKind>, WorkspaceError> {
    let path = dir.join(GRAPH_FILE);
    let bytes = fs::read(&path).map_err(WorkspaceError::IoError)?;

    bincode::deserialize(&bytes).map_err(|e| {
        WorkspaceError::SerializationError(format!("bincode deserialize: {e}"))
    })
}

// ---------------------------------------------------------------------------
// Metadata persistence (JSON)
// ---------------------------------------------------------------------------

/// Serialize workspace metadata to `metadata.json` (pretty-printed).
///
/// Uses the same atomic-write strategy as `save_graph`.
pub fn save_metadata(
    dir: &Path,
    metadata: &WorkspaceMetadata,
) -> Result<(), WorkspaceError> {
    let json = serde_json::to_string_pretty(metadata).map_err(|e| {
        WorkspaceError::SerializationError(format!("json serialize: {e}"))
    })?;

    atomic_write(&dir.join(METADATA_FILE), json.as_bytes())
}

/// Deserialize workspace metadata from `metadata.json`.
pub fn load_metadata(dir: &Path) -> Result<WorkspaceMetadata, WorkspaceError> {
    let path = dir.join(METADATA_FILE);
    let json = fs::read_to_string(&path).map_err(WorkspaceError::IoError)?;

    serde_json::from_str(&json).map_err(|e| {
        WorkspaceError::SerializationError(format!("json deserialize: {e}"))
    })
}

// ---------------------------------------------------------------------------
// File locking
// ---------------------------------------------------------------------------

/// RAII guard that holds a file lock. The lock is released when this value
/// is dropped (the underlying `File` is closed).
#[derive(Debug)]
pub struct WorkspaceLock {
    /// Kept alive solely to maintain the advisory lock.
    _file: fs::File,
}

/// Acquire an **exclusive** (write) lock on the workspace directory.
///
/// Returns `Err(WorkspaceError::LockConflict)` if another process already
/// holds a shared or exclusive lock.
pub fn acquire_write_lock(dir: &Path) -> Result<WorkspaceLock, WorkspaceError> {
    let lock_path = dir.join(LOCK_FILE);
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(WorkspaceError::IoError)?;

    file.try_lock_exclusive()
        .map_err(|_| WorkspaceError::LockConflict {
            name: dir_name(dir),
        })?;

    Ok(WorkspaceLock { _file: file })
}

/// Acquire a **shared** (read) lock on the workspace directory.
///
/// Multiple readers can hold shared locks simultaneously, but acquiring a
/// shared lock will fail if an exclusive lock is already held.
#[allow(dead_code)]
pub fn acquire_read_lock(dir: &Path) -> Result<WorkspaceLock, WorkspaceError> {
    let lock_path = dir.join(LOCK_FILE);
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(WorkspaceError::IoError)?;

    file.try_lock_shared()
        .map_err(|_| WorkspaceError::LockConflict {
            name: dir_name(dir),
        })?;

    Ok(WorkspaceLock { _file: file })
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Atomically write `data` to `path` by writing to a sibling `.tmp` file
/// first and then renaming.
///
/// On most filesystems `rename` is atomic with respect to readers, so a
/// crash will leave either the old or the new file — never a half-written one.
fn atomic_write(
    path: &Path,
    data: &[u8],
) -> Result<(), WorkspaceError> {
    let tmp_path = path.with_extension("tmp");
    fs::write(&tmp_path, data).map_err(WorkspaceError::IoError)?;
    fs::rename(&tmp_path, path).map_err(WorkspaceError::IoError)?;
    Ok(())
}

/// Extract the last component of a directory path as a `String`.
///
/// Used for human-readable error messages (e.g. lock conflict).
fn dir_name(dir: &Path) -> String {
    dir.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| dir.display().to_string())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use context_trace::graph::vertex::atom::Atom;

    /// Helper: create a temporary base directory that is automatically cleaned
    /// up when the returned `TempDir` is dropped.
    fn tmp_base() -> tempfile::TempDir {
        tempfile::tempdir().expect("failed to create temp dir")
    }

    // -- Path helpers -------------------------------------------------------

    #[test]
    fn workspace_dir_layout() {
        let base = Path::new("/projects/my-app");
        let dir = workspace_dir(base, "demo");
        assert_eq!(dir, PathBuf::from("/projects/my-app/.context-engine/demo"));
    }

    #[test]
    fn workspace_exists_false_when_nothing() {
        let base = tmp_base();
        assert!(!workspace_exists(base.path(), "nope"));
    }

    #[test]
    fn workspace_exists_true_after_save() {
        let base = tmp_base();
        let name = "test-ws";
        let dir = workspace_dir(base.path(), name);
        fs::create_dir_all(&dir).unwrap();

        // Just directory → not valid
        assert!(!workspace_exists(base.path(), name));

        // Write metadata → valid
        let meta = WorkspaceMetadata::new(name);
        save_metadata(&dir, &meta).unwrap();
        assert!(workspace_exists(base.path(), name));
    }

    // -- list ---------------------------------------------------------------

    #[test]
    fn list_empty_when_no_context_dir() {
        let base = tmp_base();
        let names = list_workspace_names(base.path()).unwrap();
        assert!(names.is_empty());
    }

    #[test]
    fn list_returns_sorted_names() {
        let base = tmp_base();

        for name in &["cherry", "apple", "banana"] {
            let dir = workspace_dir(base.path(), name);
            fs::create_dir_all(&dir).unwrap();
            save_metadata(&dir, &WorkspaceMetadata::new(name)).unwrap();
        }

        // Create a spurious directory without metadata — should be excluded
        let spurious = workspace_dir(base.path(), "broken");
        fs::create_dir_all(&spurious).unwrap();

        let names = list_workspace_names(base.path()).unwrap();
        assert_eq!(names, vec!["apple", "banana", "cherry"]);
    }

    // -- Graph round-trip ---------------------------------------------------

    #[test]
    fn graph_round_trip_empty() {
        let base = tmp_base();
        let dir = workspace_dir(base.path(), "empty");
        fs::create_dir_all(&dir).unwrap();

        let original = Hypergraph::<BaseGraphKind>::default();
        save_graph(&dir, &original).unwrap();

        let loaded = load_graph(&dir).unwrap();
        assert_eq!(loaded.vertex_count(), 0);
    }

    #[test]
    fn graph_round_trip_with_atoms() {
        let base = tmp_base();
        let dir = workspace_dir(base.path(), "atoms");
        fs::create_dir_all(&dir).unwrap();

        let original = Hypergraph::<BaseGraphKind>::default();
        let _ta = original.insert_atom(Atom::Element('a'));
        let _tb = original.insert_atom(Atom::Element('b'));
        assert_eq!(original.vertex_count(), 2);

        save_graph(&dir, &original).unwrap();
        let loaded = load_graph(&dir).unwrap();
        assert_eq!(loaded.vertex_count(), 2);
    }

    #[test]
    fn graph_round_trip_with_pattern() {
        let base = tmp_base();
        let dir = workspace_dir(base.path(), "pattern");
        fs::create_dir_all(&dir).unwrap();

        let original = Hypergraph::<BaseGraphKind>::default();
        let ta = original.insert_atom(Atom::Element('x'));
        let tb = original.insert_atom(Atom::Element('y'));
        let _p = original.insert_pattern(vec![ta, tb]);
        assert_eq!(original.vertex_count(), 3);

        save_graph(&dir, &original).unwrap();
        let loaded = load_graph(&dir).unwrap();
        assert_eq!(loaded.vertex_count(), 3);
    }

    // -- Metadata round-trip ------------------------------------------------

    #[test]
    fn metadata_round_trip() {
        let base = tmp_base();
        let dir = workspace_dir(base.path(), "meta-test");
        fs::create_dir_all(&dir).unwrap();

        let mut original = WorkspaceMetadata::new("meta-test");
        original.description = Some("unit test workspace".to_string());

        save_metadata(&dir, &original).unwrap();
        let loaded = load_metadata(&dir).unwrap();

        assert_eq!(loaded.name, original.name);
        assert_eq!(loaded.description, original.description);
        assert_eq!(loaded.created_at, original.created_at);
        assert_eq!(loaded.modified_at, original.modified_at);
    }

    // -- Locking ------------------------------------------------------------

    #[test]
    fn write_lock_prevents_second_write_lock() {
        let base = tmp_base();
        let dir = workspace_dir(base.path(), "locked");
        fs::create_dir_all(&dir).unwrap();

        let _lock1 =
            acquire_write_lock(&dir).expect("first lock should succeed");

        // Second exclusive lock on the same directory should fail
        let result = acquire_write_lock(&dir);
        assert!(
            result.is_err(),
            "second write lock should fail while first is held"
        );
        match result.unwrap_err() {
            WorkspaceError::LockConflict { name } => {
                assert_eq!(name, "locked");
            },
            other => panic!("expected LockConflict, got: {other}"),
        }
    }

    #[test]
    fn lock_released_on_drop() {
        let base = tmp_base();
        let dir = workspace_dir(base.path(), "drop-lock");
        fs::create_dir_all(&dir).unwrap();

        {
            let _lock = acquire_write_lock(&dir).unwrap();
            // lock held within this scope
        }
        // After drop, we should be able to lock again
        let _lock2 = acquire_write_lock(&dir)
            .expect("lock should be available after drop");
    }

    #[test]
    fn multiple_read_locks_allowed() {
        let base = tmp_base();
        let dir = workspace_dir(base.path(), "multi-read");
        fs::create_dir_all(&dir).unwrap();

        let _r1 = acquire_read_lock(&dir).expect("first read lock");
        let _r2 = acquire_read_lock(&dir)
            .expect("second read lock should also succeed");
    }

    // -- Atomic write -------------------------------------------------------

    #[test]
    fn atomic_write_no_leftover_tmp() {
        let base = tmp_base();
        let dir = workspace_dir(base.path(), "atomic");
        fs::create_dir_all(&dir).unwrap();

        let target = dir.join("test.bin");
        atomic_write(&target, b"hello world").unwrap();

        assert!(target.exists(), "target file should exist");
        assert!(
            !target.with_extension("tmp").exists(),
            "tmp file should have been renamed away"
        );
        assert_eq!(fs::read(&target).unwrap(), b"hello world");
    }

    #[test]
    fn atomic_write_overwrites_existing() {
        let base = tmp_base();
        let dir = workspace_dir(base.path(), "overwrite");
        fs::create_dir_all(&dir).unwrap();

        let target = dir.join("data.bin");
        atomic_write(&target, b"first").unwrap();
        atomic_write(&target, b"second").unwrap();

        assert_eq!(fs::read(&target).unwrap(), b"second");
    }

    // -- Error cases --------------------------------------------------------

    #[test]
    fn load_graph_missing_file() {
        let base = tmp_base();
        let dir = workspace_dir(base.path(), "no-graph");
        fs::create_dir_all(&dir).unwrap();

        let result = load_graph(&dir);
        assert!(result.is_err());
    }

    #[test]
    fn load_metadata_missing_file() {
        let base = tmp_base();
        let dir = workspace_dir(base.path(), "no-meta");
        fs::create_dir_all(&dir).unwrap();

        let result = load_metadata(&dir);
        assert!(result.is_err());
    }
}
