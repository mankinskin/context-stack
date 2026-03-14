//! Workspace manager — creates, opens, closes, saves, lists, and deletes
//! workspaces.
//!
//! `WorkspaceManager` is the top-level entry point for all workspace lifecycle
//! operations. It holds a map of currently open workspaces and a base directory
//! path under which all workspace data is stored.
//!
//! ## Usage
//!
//! ```rust,no_run
//! use context_api::workspace::manager::WorkspaceManager;
//!
//! let mut mgr = WorkspaceManager::current_dir().unwrap();
//! let info = mgr.create_workspace("demo").unwrap();
//! println!("Created: {}", info.name);
//! mgr.save_workspace("demo").unwrap();
//! mgr.close_workspace("demo").unwrap();
//! ```

use std::{
    collections::HashMap,
    fs,
    path::{
        Path,
        PathBuf,
    },
    sync::{
        Arc,
        atomic::{
            AtomicBool,
            Ordering,
        },
    },
    thread,
    time::{
        Duration,
        Instant,
    },
};

use ngrams::{
    Status,
    graph::{
        Corpus,
        StatusHandle,
        parse_corpus,
        traversal::pass::CancelReason,
    },
};

use context_trace::graph::{
    Hypergraph,
    HypergraphRef,
    kind::BaseGraphKind,
};

use crate::{
    error::WorkspaceError,
    types::WorkspaceInfo,
    workspace::{
        Workspace,
        metadata::WorkspaceMetadata,
        persistence,
    },
};

/// Manages the lifecycle of hypergraph workspaces.
///
/// Workspaces are stored under `<base_dir>/.context-engine/<name>/` and can
/// be independently created, opened, saved, closed, and deleted. Only one
/// process should hold a workspace open at a time (enforced by advisory file
/// locks via `fs2`).
#[derive(Debug)]
pub struct WorkspaceManager {
    /// The project root (or any directory) under which `.context-engine/`
    /// lives.
    base_dir: PathBuf,

    /// Currently open workspaces, keyed by name.
    workspaces: HashMap<String, Workspace>,
}

impl WorkspaceManager {
    // -------------------------------------------------------------------
    // Construction
    // -------------------------------------------------------------------

    /// Create a new manager rooted at `base_dir`.
    ///
    /// The `.context-engine/` subdirectory is created lazily when the first
    /// workspace is created.
    pub fn new(base_dir: PathBuf) -> Self {
        Self {
            base_dir,
            workspaces: HashMap::new(),
        }
    }

    /// Create a manager rooted at the current working directory.
    pub fn current_dir() -> Result<Self, WorkspaceError> {
        let cwd = std::env::current_dir().map_err(WorkspaceError::IoError)?;
        Ok(Self::new(cwd))
    }

    /// Return the base directory this manager is rooted at.
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    // -------------------------------------------------------------------
    // Workspace lifecycle
    // -------------------------------------------------------------------

    /// Create a brand-new workspace with the given name.
    ///
    /// Creates the on-disk directory structure, writes an empty graph and
    /// fresh metadata, acquires an exclusive lock, and opens the workspace
    /// in memory.
    ///
    /// # Errors
    ///
    /// - `WorkspaceError::AlreadyExists` if the directory already exists.
    /// - `WorkspaceError::IoError` on filesystem failures.
    pub fn create_workspace(
        &mut self,
        name: &str,
    ) -> Result<WorkspaceInfo, WorkspaceError> {
        let dir = persistence::workspace_dir(&self.base_dir, name);

        if dir.exists() {
            return Err(WorkspaceError::AlreadyExists {
                name: name.to_string(),
            });
        }

        fs::create_dir_all(&dir).map_err(WorkspaceError::IoError)?;

        let graph = Hypergraph::<BaseGraphKind>::default();
        let metadata = WorkspaceMetadata::new(name);

        // Persist the initial (empty) state.
        persistence::save_graph(&dir, &graph)?;
        persistence::save_metadata(&dir, &metadata)?;

        // Acquire write lock.
        let lock = persistence::acquire_write_lock(&dir)?;

        let ws = Workspace {
            name: name.to_string(),
            dir,
            graph: HypergraphRef::from(graph),
            metadata,
            lock: Some(lock),
            dirty: false,
        };

        let info = ws.to_info();
        self.workspaces.insert(name.to_string(), ws);
        Ok(info)
    }

    /// Create a workspace by parsing a text with the slow but canonical ngrams algorithm.
    ///
    /// This is primarily intended for validation workflows against `context-read`.
    /// A timeout is mandatory at call sites and should default to 60s in adapters.
    pub fn create_workspace_from_ngrams_text(
        &mut self,
        name: &str,
        text: &str,
        timeout_secs: u64,
    ) -> Result<WorkspaceInfo, WorkspaceError> {
        let timeout_secs = timeout_secs.max(1);
        self.create_workspace(name)?;

        let texts = vec![text.to_string()];
        let corpus = Corpus::new(format!("ngrams_{}", name), texts.clone());
        let status = StatusHandle::from(Status::new(texts));

        let cancelled = Arc::new(AtomicBool::new(false));
        let done = Arc::new(AtomicBool::new(false));

        let cancelled_for_watchdog = Arc::clone(&cancelled);
        let done_for_watchdog = Arc::clone(&done);
        let watchdog = thread::spawn(move || {
            let start = Instant::now();
            let timeout = Duration::from_secs(timeout_secs);
            while !done_for_watchdog.load(Ordering::SeqCst) {
                if start.elapsed() >= timeout {
                    cancelled_for_watchdog.store(true, Ordering::SeqCst);
                    break;
                }
                thread::sleep(Duration::from_millis(50));
            }
        });

        let parse_result = parse_corpus(
            corpus,
            status,
            Arc::clone(&cancelled),
        );

        done.store(true, Ordering::SeqCst);
        let _ = watchdog.join();

        let parsed = match parse_result {
            Ok(p) => p,
            Err(CancelReason::Cancelled) => {
                self.rollback_workspace_creation(name);
                return Err(WorkspaceError::NgramsTimeout {
                    seconds: timeout_secs,
                });
            },
            Err(other) => {
                self.rollback_workspace_creation(name);
                return Err(WorkspaceError::NgramsFailed {
                    reason: format!("{:?}", other),
                });
            },
        };

        {
            let ws = self.get_workspace_mut(name)?;
            ws.graph = HypergraphRef::from(parsed.graph);
            ws.metadata.touch();
            ws.dirty = true;
        }

        self.save_workspace(name)?;
        Ok(self.get_workspace(name)?.to_info())
    }

    /// Open an existing workspace from disk.
    ///
    /// Loads the graph (bincode) and metadata (JSON), acquires an exclusive
    /// lock, and keeps the workspace in memory until it is closed.
    ///
    /// # Errors
    ///
    /// - `WorkspaceError::AlreadyOpen` if already open in this manager.
    /// - `WorkspaceError::NotFound` if no workspace with that name exists on
    ///   disk.
    /// - `WorkspaceError::LockConflict` if another process holds the lock.
    pub fn open_workspace(
        &mut self,
        name: &str,
    ) -> Result<WorkspaceInfo, WorkspaceError> {
        if self.workspaces.contains_key(name) {
            return Err(WorkspaceError::AlreadyOpen {
                name: name.to_string(),
            });
        }

        let dir = persistence::workspace_dir(&self.base_dir, name);

        if !persistence::workspace_exists(&self.base_dir, name) {
            return Err(WorkspaceError::NotFound {
                name: name.to_string(),
            });
        }

        // Lock first — fail fast if another process has it.
        let lock = persistence::acquire_write_lock(&dir)?;

        let graph = persistence::load_graph(&dir)?;
        let metadata = persistence::load_metadata(&dir)?;

        let ws = Workspace {
            name: name.to_string(),
            dir,
            graph: HypergraphRef::from(graph),
            metadata,
            lock: Some(lock),
            dirty: false,
        };

        let info = ws.to_info();
        self.workspaces.insert(name.to_string(), ws);
        Ok(info)
    }

    /// Persist the current in-memory state of a workspace to disk.
    ///
    /// Writes both `graph.bin` and `metadata.json` atomically. Updates
    /// `modified_at` and clears the dirty flag.
    ///
    /// # Errors
    ///
    /// - `WorkspaceError::NotOpen` if the workspace is not currently open.
    /// - `WorkspaceError::IoError` / `WorkspaceError::SerializationError` on
    ///   write failures.
    pub fn save_workspace(
        &mut self,
        name: &str,
    ) -> Result<(), WorkspaceError> {
        let ws = self.get_workspace_mut(name)?;
        ws.metadata.touch();
        persistence::save_graph(&ws.dir, &ws.graph)?;
        persistence::save_metadata(&ws.dir, &ws.metadata)?;
        ws.dirty = false;
        Ok(())
    }

    /// Close an open workspace, releasing its file lock.
    ///
    /// **Does NOT auto-save.** If there are unsaved changes a warning is
    /// logged. Callers should `save_workspace` first if they want to persist
    /// changes.
    ///
    /// # Errors
    ///
    /// - `WorkspaceError::NotOpen` if the workspace is not currently open.
    pub fn close_workspace(
        &mut self,
        name: &str,
    ) -> Result<(), WorkspaceError> {
        let ws = self.workspaces.remove(name).ok_or_else(|| {
            WorkspaceError::NotOpen {
                name: name.to_string(),
            }
        })?;

        if ws.dirty {
            tracing::warn!(
                workspace = name,
                "closing workspace with unsaved changes"
            );
        }

        // The `WorkspaceLock` is dropped here, releasing the advisory lock.
        drop(ws);
        Ok(())
    }

    /// List all workspaces — both currently open and those only on disk.
    ///
    /// Open workspaces return live statistics (vertex count, atom count, etc.).
    /// Closed workspaces show counts as 0 (loading the full graph just to count
    /// would be expensive for large workspaces).
    pub fn list_workspaces(
        &self
    ) -> Result<Vec<WorkspaceInfo>, WorkspaceError> {
        let mut result: Vec<WorkspaceInfo> = Vec::new();

        // Include open workspaces (live data).
        for ws in self.workspaces.values() {
            result.push(ws.to_info());
        }

        // Include closed workspaces from disk (metadata only).
        for name in persistence::list_workspace_names(&self.base_dir)? {
            if !self.workspaces.contains_key(&name) {
                let dir = persistence::workspace_dir(&self.base_dir, &name);
                if let Ok(metadata) = persistence::load_metadata(&dir) {
                    result.push(WorkspaceInfo {
                        name: name.clone(),
                        vertex_count: 0,
                        atom_count: 0,
                        pattern_count: 0,
                        created_at: metadata.created_at.to_rfc3339(),
                        modified_at: metadata.modified_at.to_rfc3339(),
                    });
                }
            }
        }

        result.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(result)
    }

    /// Delete a workspace from disk.
    ///
    /// If the workspace is currently open it is closed first (without saving).
    /// The entire workspace directory is removed.
    ///
    /// # Errors
    ///
    /// - `WorkspaceError::NotFound` if no workspace with that name exists on
    ///   disk.
    pub fn delete_workspace(
        &mut self,
        name: &str,
    ) -> Result<(), WorkspaceError> {
        // Close without saving if open.
        self.workspaces.remove(name);

        let dir = persistence::workspace_dir(&self.base_dir, name);
        if !dir.exists() {
            return Err(WorkspaceError::NotFound {
                name: name.to_string(),
            });
        }

        fs::remove_dir_all(&dir).map_err(WorkspaceError::IoError)?;
        Ok(())
    }

    fn rollback_workspace_creation(
        &mut self,
        name: &str,
    ) {
        self.workspaces.remove(name);
        let dir = persistence::workspace_dir(&self.base_dir, name);
        let _ = fs::remove_dir_all(dir);
    }

    // -------------------------------------------------------------------
    // Internal accessors
    // -------------------------------------------------------------------

    /// Get an immutable reference to an open workspace.
    ///
    /// # Errors
    ///
    /// - `WorkspaceError::NotOpen` if the workspace is not currently open.
    pub(crate) fn get_workspace(
        &self,
        name: &str,
    ) -> Result<&Workspace, WorkspaceError> {
        self.workspaces
            .get(name)
            .ok_or_else(|| WorkspaceError::NotOpen {
                name: name.to_string(),
            })
    }

    /// Get a mutable reference to an open workspace.
    ///
    /// # Errors
    ///
    /// - `WorkspaceError::NotOpen` if the workspace is not currently open.
    pub(crate) fn get_workspace_mut(
        &mut self,
        name: &str,
    ) -> Result<&mut Workspace, WorkspaceError> {
        self.workspaces
            .get_mut(name)
            .ok_or_else(|| WorkspaceError::NotOpen {
                name: name.to_string(),
            })
    }

    /// Check whether a workspace with the given name is currently open.
    pub fn is_open(
        &self,
        name: &str,
    ) -> bool {
        self.workspaces.contains_key(name)
    }

    /// Get the log directory path for a workspace.
    ///
    /// Creates the directory if it doesn't exist. The log directory lives
    /// inside the workspace's persistence directory at `<workspace_dir>/logs/`.
    ///
    /// # Errors
    ///
    /// - `WorkspaceError::NotFound` if no workspace directory exists on disk.
    /// - `WorkspaceError::IoError` on filesystem failures.
    pub fn log_dir(
        &self,
        workspace_name: &str,
    ) -> Result<std::path::PathBuf, WorkspaceError> {
        let ws_dir =
            super::persistence::workspace_dir(&self.base_dir, workspace_name);
        if !ws_dir.exists() {
            return Err(WorkspaceError::NotFound {
                name: workspace_name.to_string(),
            });
        }
        let dir = ws_dir.join("logs");
        std::fs::create_dir_all(&dir).map_err(WorkspaceError::IoError)?;
        Ok(dir)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a `WorkspaceManager` backed by a temporary directory.
    fn tmp_manager() -> (tempfile::TempDir, WorkspaceManager) {
        let tmp = tempfile::tempdir().expect("failed to create temp dir");
        let mgr = WorkspaceManager::new(tmp.path().to_path_buf());
        (tmp, mgr)
    }

    // -- create -------------------------------------------------------------

    #[test]
    fn create_workspace_succeeds() {
        let (_tmp, mut mgr) = tmp_manager();
        let info = mgr.create_workspace("demo").unwrap();
        assert_eq!(info.name, "demo");
        assert_eq!(info.vertex_count, 0);
        assert!(mgr.is_open("demo"));
    }

    #[test]
    fn create_workspace_creates_files_on_disk() {
        let (tmp, mut mgr) = tmp_manager();
        mgr.create_workspace("files").unwrap();

        let dir = persistence::workspace_dir(tmp.path(), "files");
        assert!(dir.join(persistence::GRAPH_FILE).exists());
        assert!(dir.join(persistence::METADATA_FILE).exists());
    }

    #[test]
    fn create_duplicate_workspace_fails() {
        let (_tmp, mut mgr) = tmp_manager();
        mgr.create_workspace("dup").unwrap();
        // Close first so that the lock is released, then try to re-create.
        mgr.close_workspace("dup").unwrap();

        let err = mgr.create_workspace("dup").unwrap_err();
        match err {
            WorkspaceError::AlreadyExists { name } => {
                assert_eq!(name, "dup");
            },
            other => panic!("expected AlreadyExists, got: {other}"),
        }
    }

    // -- open ---------------------------------------------------------------

    #[test]
    fn open_workspace_round_trip() {
        let (_tmp, mut mgr) = tmp_manager();
        mgr.create_workspace("rt").unwrap();
        mgr.save_workspace("rt").unwrap();
        mgr.close_workspace("rt").unwrap();

        assert!(!mgr.is_open("rt"));

        let info = mgr.open_workspace("rt").unwrap();
        assert_eq!(info.name, "rt");
        assert!(mgr.is_open("rt"));
    }

    #[test]
    fn open_nonexistent_workspace_fails() {
        let (_tmp, mut mgr) = tmp_manager();
        let err = mgr.open_workspace("ghost").unwrap_err();
        match err {
            WorkspaceError::NotFound { name } => assert_eq!(name, "ghost"),
            other => panic!("expected NotFound, got: {other}"),
        }
    }

    #[test]
    fn open_already_open_workspace_fails() {
        let (_tmp, mut mgr) = tmp_manager();
        mgr.create_workspace("dup-open").unwrap();

        let err = mgr.open_workspace("dup-open").unwrap_err();
        match err {
            WorkspaceError::AlreadyOpen { name } => {
                assert_eq!(name, "dup-open")
            },
            other => panic!("expected AlreadyOpen, got: {other}"),
        }
    }

    // -- save ---------------------------------------------------------------

    #[test]
    fn save_clears_dirty_flag() {
        let (_tmp, mut mgr) = tmp_manager();
        mgr.create_workspace("saveme").unwrap();

        // Mutate the graph to set dirty.
        {
            let ws = mgr.get_workspace_mut("saveme").unwrap();
            let _ = ws.graph_mut(); // marks dirty
            assert!(ws.is_dirty());
        }

        mgr.save_workspace("saveme").unwrap();

        let ws = mgr.get_workspace("saveme").unwrap();
        assert!(!ws.is_dirty());
    }

    #[test]
    fn save_not_open_fails() {
        let (_tmp, mut mgr) = tmp_manager();
        let err = mgr.save_workspace("nope").unwrap_err();
        match err {
            WorkspaceError::NotOpen { name } => assert_eq!(name, "nope"),
            other => panic!("expected NotOpen, got: {other}"),
        }
    }

    // -- close --------------------------------------------------------------

    #[test]
    fn close_removes_from_manager() {
        let (_tmp, mut mgr) = tmp_manager();
        mgr.create_workspace("closeme").unwrap();
        assert!(mgr.is_open("closeme"));

        mgr.close_workspace("closeme").unwrap();
        assert!(!mgr.is_open("closeme"));
    }

    #[test]
    fn close_not_open_fails() {
        let (_tmp, mut mgr) = tmp_manager();
        let err = mgr.close_workspace("nope").unwrap_err();
        match err {
            WorkspaceError::NotOpen { name } => assert_eq!(name, "nope"),
            other => panic!("expected NotOpen, got: {other}"),
        }
    }

    // -- list ---------------------------------------------------------------

    #[test]
    fn list_empty() {
        let (_tmp, mgr) = tmp_manager();
        let list = mgr.list_workspaces().unwrap();
        assert!(list.is_empty());
    }

    #[test]
    fn list_includes_open_and_closed() {
        let (_tmp, mut mgr) = tmp_manager();
        mgr.create_workspace("alpha").unwrap();
        mgr.save_workspace("alpha").unwrap();
        mgr.close_workspace("alpha").unwrap();

        mgr.create_workspace("beta").unwrap(); // still open

        let list = mgr.list_workspaces().unwrap();
        let names: Vec<&str> = list.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, vec!["alpha", "beta"]);

        // The open workspace ("beta") should have vertex_count == 0 (empty graph).
        // The closed workspace ("alpha") also shows 0 because we don't load the graph.
        for info in &list {
            assert_eq!(info.vertex_count, 0);
        }
    }

    // -- delete -------------------------------------------------------------

    #[test]
    fn delete_removes_directory() {
        let (tmp, mut mgr) = tmp_manager();
        mgr.create_workspace("byebye").unwrap();
        mgr.close_workspace("byebye").unwrap();

        let dir = persistence::workspace_dir(tmp.path(), "byebye");
        assert!(dir.exists());

        mgr.delete_workspace("byebye").unwrap();
        assert!(!dir.exists());
    }

    #[test]
    fn delete_open_workspace_closes_first() {
        let (_tmp, mut mgr) = tmp_manager();
        mgr.create_workspace("open-del").unwrap();
        assert!(mgr.is_open("open-del"));

        mgr.delete_workspace("open-del").unwrap();
        assert!(!mgr.is_open("open-del"));
    }

    #[test]
    fn delete_nonexistent_fails() {
        let (_tmp, mut mgr) = tmp_manager();
        let err = mgr.delete_workspace("void").unwrap_err();
        match err {
            WorkspaceError::NotFound { name } => assert_eq!(name, "void"),
            other => panic!("expected NotFound, got: {other}"),
        }
    }

    // -- persistence round-trip with data -----------------------------------

    #[test]
    fn data_survives_save_close_open_cycle() {
        let (_tmp, mut mgr) = tmp_manager();
        mgr.create_workspace("persist").unwrap();

        // Insert some atoms.
        {
            use context_trace::graph::vertex::atom::Atom;
            let ws = mgr.get_workspace_mut("persist").unwrap();
            let graph = ws.graph_mut();
            graph.insert_atom(Atom::Element('a'));
            graph.insert_atom(Atom::Element('b'));
        }

        mgr.save_workspace("persist").unwrap();
        mgr.close_workspace("persist").unwrap();

        let info = mgr.open_workspace("persist").unwrap();
        assert_eq!(info.vertex_count, 2, "atoms should survive round-trip");
    }

    // -- is_open ------------------------------------------------------------

    #[test]
    fn is_open_tracks_state() {
        let (_tmp, mut mgr) = tmp_manager();
        assert!(!mgr.is_open("x"));

        mgr.create_workspace("x").unwrap();
        assert!(mgr.is_open("x"));

        mgr.close_workspace("x").unwrap();
        assert!(!mgr.is_open("x"));
    }
}
