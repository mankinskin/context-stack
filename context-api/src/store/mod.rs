//! The graph store: `<store>/<name>.ctx` files, their locks and server
//! discovery files.
//!
//! ```text
//! <store>/corpus.ctx              graph file (see [`file`])
//! <store>/corpus.ctx.lock         advisory lock: exclusive while open for writing
//! <store>/corpus.ctx.server.json  written by a server holding the graph
//! ```

pub mod file;

use std::{
    fs::{
        self,
        File,
        OpenOptions,
    },
    path::{
        Path,
        PathBuf,
    },
};

use context_trace::positioned::{
    GraphRead,
    NodeRef,
    PositionedGraph,
    PositionedGraphRef,
    VertexRead,
};
use fs2::FileExt;
use serde::{
    Deserialize,
    Serialize,
};

use self::file::GraphMeta;
use crate::{
    error::ApiError,
    types::{
        GraphSummary,
        SaveReport,
    },
};

/// File extension of graph files.
pub const EXTENSION: &str = "ctx";

/// Whether a graph reference is a path (ends in `.ctx`) rather than a name.
pub fn is_path_ref(graph: &str) -> bool {
    Path::new(graph).extension().is_some_and(|ext| ext == EXTENSION)
}

/// A resolved graph reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphLocation {
    /// Display name: the store name, or the file stem of a path.
    pub name: String,
    pub path: PathBuf,
}

impl GraphLocation {
    fn with_suffix(
        &self,
        suffix: &str,
    ) -> PathBuf {
        let mut name = self.path.file_name().unwrap_or_default().to_os_string();
        name.push(suffix);
        self.path.with_file_name(name)
    }

    pub fn lock_path(&self) -> PathBuf {
        self.with_suffix(".lock")
    }

    pub fn server_path(&self) -> PathBuf {
        self.with_suffix(".server.json")
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.with_suffix(".logs")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenMode {
    /// Queries; other readers may hold the graph too.
    Shared,
    /// Mutations; no other process may open the graph.
    Exclusive,
}

/// Discovery file of a server that holds a graph open.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerInfo {
    pub url: String,
    pub pid: u32,
    #[serde(default)]
    pub token_required: bool,
}

impl ServerInfo {
    /// Whether the recorded process still runs (always true where this
    /// cannot be checked).
    pub fn is_alive(&self) -> bool {
        if cfg!(target_os = "linux") {
            Path::new("/proc").join(self.pid.to_string()).exists()
        } else {
            true
        }
    }
}

/// The graphs of one store directory.
#[derive(Debug, Clone)]
pub struct GraphStore {
    dir: PathBuf,
}

impl GraphStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Resolve a name (`[A-Za-z0-9._-]+`, not starting with `.`) to
    /// `<store>/<name>.ctx`, or a path ending in `.ctx` to itself.
    pub fn resolve(
        &self,
        graph: &str,
    ) -> Result<GraphLocation, ApiError> {
        let as_path = Path::new(graph);
        if is_path_ref(graph) {
            let name = as_path
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .filter(|stem| !stem.is_empty())
                .ok_or_else(|| ApiError::InvalidInput(format!("invalid graph path '{graph}'")))?;
            return Ok(GraphLocation {
                name,
                path: as_path.to_path_buf(),
            });
        }
        let valid = !graph.is_empty()
            && !graph.starts_with('.')
            && graph
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
        if !valid {
            return Err(ApiError::InvalidInput(format!(
                "invalid graph name '{graph}': use letters, digits, '.', '_', '-' or a path ending in .{EXTENSION}"
            )));
        }
        Ok(GraphLocation {
            name: graph.to_string(),
            path: self.dir.join(format!("{graph}.{EXTENSION}")),
        })
    }

    /// Fail with `LegacyWorkspace` for a row-model workspace directory.
    fn check_legacy(
        &self,
        graph: &str,
    ) -> Result<(), ApiError> {
        if self.dir.join(graph).join("metadata.json").is_file() {
            return Err(ApiError::LegacyWorkspace {
                graph: graph.to_string(),
            });
        }
        Ok(())
    }

    pub fn exists(
        &self,
        location: &GraphLocation,
    ) -> bool {
        location.path.is_file()
    }

    /// The live server holding the graph, if any.
    pub fn server(
        &self,
        location: &GraphLocation,
    ) -> Option<ServerInfo> {
        let bytes = fs::read(location.server_path()).ok()?;
        let info: ServerInfo = serde_json::from_slice(&bytes).ok()?;
        info.is_alive().then_some(info)
    }

    pub fn write_server_info(
        &self,
        location: &GraphLocation,
        info: &ServerInfo,
    ) -> Result<(), ApiError> {
        let bytes = serde_json::to_vec_pretty(info)
            .map_err(|e| ApiError::Internal(e.to_string()))?;
        file::write_atomic(&location.server_path(), &bytes)
    }

    pub fn remove_server_info(
        &self,
        location: &GraphLocation,
    ) {
        let _ = fs::remove_file(location.server_path());
    }

    fn lock(
        &self,
        location: &GraphLocation,
        mode: OpenMode,
    ) -> Result<File, ApiError> {
        if let Some(parent) = location.path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
        }
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(location.lock_path())?;
        let locked = match mode {
            OpenMode::Shared => FileExt::try_lock_shared(&lock),
            OpenMode::Exclusive => FileExt::try_lock_exclusive(&lock),
        };
        locked.map_err(|_| self.locked(location))?;
        Ok(lock)
    }

    fn locked(
        &self,
        location: &GraphLocation,
    ) -> ApiError {
        ApiError::Locked {
            graph: location.name.clone(),
            holder: self.server(location).map(|info| info.url),
        }
    }

    /// Create an empty graph, saved immediately, open exclusively.
    pub fn create(
        &self,
        graph: &str,
        overwrite: bool,
    ) -> Result<OpenGraph, ApiError> {
        let mut open = self.create_unsaved(graph, overwrite)?;
        open.save()?;
        Ok(open)
    }

    /// Create an empty graph open exclusively, without writing the file:
    /// the first save creates it (or replaces an overwritten graph), so an
    /// interrupted creation leaves the store unchanged.
    pub fn create_unsaved(
        &self,
        graph: &str,
        overwrite: bool,
    ) -> Result<OpenGraph, ApiError> {
        let location = self.resolve(graph)?;
        self.check_legacy(graph)?;
        let lock = self.lock(&location, OpenMode::Exclusive)?;
        if self.exists(&location) && !overwrite {
            return Err(ApiError::GraphExists {
                graph: location.name,
            });
        }
        Ok(OpenGraph {
            meta: GraphMeta::new(&location.name),
            location,
            graph: PositionedGraphRef::new(PositionedGraph::default()),
            mode: OpenMode::Exclusive,
            lock,
            dirty: true,
        })
    }

    pub fn open(
        &self,
        graph: &str,
        mode: OpenMode,
    ) -> Result<OpenGraph, ApiError> {
        let location = self.resolve(graph)?;
        self.check_legacy(graph)?;
        if !self.exists(&location) {
            return Err(ApiError::GraphNotFound {
                graph: location.name,
            });
        }
        let lock = self.lock(&location, mode)?;
        let (meta, graph) = file::read(&location.path)?;
        Ok(OpenGraph {
            location,
            meta,
            graph: PositionedGraphRef::new(graph),
            mode,
            lock,
            dirty: false,
        })
    }

    /// Summary from the file's metadata, without decoding the graph.
    pub fn summary(
        &self,
        graph: &str,
    ) -> Result<GraphSummary, ApiError> {
        let location = self.resolve(graph)?;
        self.check_legacy(graph)?;
        if !self.exists(&location) {
            return Err(ApiError::GraphNotFound {
                graph: location.name,
            });
        }
        let meta = file::read_meta(&location.path)?;
        let file_size = fs::metadata(&location.path).ok().map(|m| m.len());
        Ok(summary(&location, &meta, file_size, false, false))
    }

    /// Graphs in the store directory, by name. Unreadable files are skipped.
    pub fn list(&self) -> Result<Vec<GraphSummary>, ApiError> {
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return Ok(Vec::new());
        };
        let mut graphs = Vec::new();
        for entry in entries {
            let path = entry?.path();
            let is_graph = path.is_file()
                && path.extension().is_some_and(|ext| ext == EXTENSION);
            let name = path.file_stem().map(|s| s.to_string_lossy().into_owned());
            if let (true, Some(name)) = (is_graph, name) {
                if let Ok(summary) = self.summary(&name) {
                    graphs.push(summary);
                }
            }
        }
        graphs.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(graphs)
    }

    /// Delete a graph file and its lock, server and log files. Fails while
    /// another process holds the graph.
    pub fn delete(
        &self,
        graph: &str,
    ) -> Result<(), ApiError> {
        let location = self.resolve(graph)?;
        self.check_legacy(graph)?;
        if !self.exists(&location) {
            return Err(ApiError::GraphNotFound {
                graph: location.name,
            });
        }
        let lock = self.lock(&location, OpenMode::Exclusive)?;
        fs::remove_file(&location.path)?;
        let _ = fs::remove_dir_all(location.logs_dir());
        let _ = fs::remove_file(location.lock_path());
        drop(lock);
        Ok(())
    }
}

fn summary(
    location: &GraphLocation,
    meta: &GraphMeta,
    file_size: Option<u64>,
    open: bool,
    dirty: bool,
) -> GraphSummary {
    GraphSummary {
        name: location.name.clone(),
        path: location.path.display().to_string(),
        vertex_count: meta.vertex_count,
        atom_count: meta.atom_count,
        composite_count: meta.vertex_count - meta.atom_count,
        input_count: meta.inputs.len(),
        total_input_chars: meta.total_input_chars(),
        generation: meta.generation,
        created_at: meta.created_at.clone(),
        modified_at: meta.modified_at.clone(),
        file_size,
        open,
        dirty,
    }
}

/// A graph loaded in memory with its lock.
#[derive(Debug)]
pub struct OpenGraph {
    pub location: GraphLocation,
    pub meta: GraphMeta,
    pub graph: PositionedGraphRef,
    mode: OpenMode,
    /// Held for its lock (released on drop).
    lock: File,
    dirty: bool,
}

impl OpenGraph {
    pub fn name(&self) -> &str {
        &self.location.name
    }

    pub fn mode(&self) -> OpenMode {
        self.mode
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Upgrade a shared open to exclusive (no reload needed: writers need
    /// the exclusive lock, so the file did not change meanwhile).
    pub fn upgrade(
        &mut self,
        store: &GraphStore,
    ) -> Result<(), ApiError> {
        if self.mode == OpenMode::Shared {
            if FileExt::try_lock_exclusive(&self.lock).is_err() {
                // A failed flock conversion drops the shared lock; take it again.
                FileExt::try_lock_shared(&self.lock).map_err(|_| {
                    ApiError::Internal(format!(
                        "lost the shared lock of graph '{}'; reopen it",
                        self.name()
                    ))
                })?;
                return Err(store.locked(&self.location));
            }
            self.mode = OpenMode::Exclusive;
        }
        Ok(())
    }

    /// Refresh the metadata counts from the graph.
    fn update_counts(&mut self) {
        let graph = self.graph.read();
        self.meta.vertex_count = graph.vertex_count();
        self.meta.atom_count = graph
            .vertices()
            .filter(|vertex| matches!(vertex.node(), NodeRef::Atom(_)))
            .count();
        self.meta.generation = graph.generation().base;
    }

    /// Atomically write the graph and its metadata.
    pub fn save(&mut self) -> Result<SaveReport, ApiError> {
        if self.mode != OpenMode::Exclusive {
            return Err(ApiError::Internal(format!(
                "graph '{}' is not open for writing",
                self.name()
            )));
        }
        self.update_counts();
        self.meta.modified_at = file::now();
        let bytes = {
            let graph = self.graph.read();
            file::encode(&self.meta, &graph)?
        };
        file::write_atomic(&self.location.path, &bytes)?;
        self.dirty = false;
        Ok(SaveReport {
            path: self.location.path.display().to_string(),
            bytes: bytes.len() as u64,
            generation: self.meta.generation,
        })
    }

    /// Summary of the in-memory state.
    pub fn summary(&mut self) -> GraphSummary {
        self.update_counts();
        let file_size = fs::metadata(&self.location.path).ok().map(|m| m.len());
        summary(&self.location, &self.meta, file_size, true, self.dirty)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, GraphStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = GraphStore::new(dir.path().join("store"));
        (dir, store)
    }

    #[test]
    fn resolve_names_and_paths() {
        let store = GraphStore::new("/s");
        assert_eq!(store.resolve("corpus").unwrap().path, Path::new("/s/corpus.ctx"));
        let by_path = store.resolve("../x/notes.ctx").unwrap();
        assert_eq!((by_path.name.as_str(), by_path.path.as_path()), ("notes", Path::new("../x/notes.ctx")));
        for bad in ["", ".hidden", "a/b", "a b", "../x"] {
            assert!(matches!(store.resolve(bad), Err(ApiError::InvalidInput(_))), "{bad}");
        }
        let location = store.resolve("g").unwrap();
        assert_eq!(location.lock_path(), Path::new("/s/g.ctx.lock"));
        assert_eq!(location.server_path(), Path::new("/s/g.ctx.server.json"));
    }

    #[test]
    fn create_open_save_list_delete() {
        let (_dir, store) = store();
        let mut g = store.create("g", false).unwrap();
        context_read::read(&g.graph, "abcabc".chars());
        g.mark_dirty();
        g.save().unwrap();
        let summary = g.summary();
        drop(g);
        assert!(matches!(store.create("g", false), Err(ApiError::GraphExists { .. })));
        let g = store.open("g", OpenMode::Shared).unwrap();
        assert_eq!(g.graph.read().vertex_count(), summary.vertex_count);
        drop(g);
        let listed = store.list().unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].vertex_count, summary.vertex_count);
        assert_eq!(listed[0].atom_count, 3);
        store.delete("g").unwrap();
        assert!(matches!(store.open("g", OpenMode::Shared), Err(ApiError::GraphNotFound { .. })));
        assert!(store.list().unwrap().is_empty());
    }

    #[test]
    fn unsaved_creation_writes_on_first_save() {
        let (_dir, store) = store();
        let g = store.create_unsaved("g", false).unwrap();
        assert!(!store.exists(&g.location));
        drop(g);
        assert!(store.list().unwrap().is_empty());

        let mut g = store.create("g", false).unwrap();
        context_read::read(&g.graph, "abab".chars());
        g.save().unwrap();
        drop(g);
        let before = fs::read(store.resolve("g").unwrap().path).unwrap();
        let mut replaced = store.create_unsaved("g", true).unwrap();
        assert_eq!(fs::read(&replaced.location.path).unwrap(), before);
        replaced.save().unwrap();
        assert_ne!(fs::read(&replaced.location.path).unwrap(), before);
    }

    #[test]
    fn locks_conflict() {
        let (_dir, store) = store();
        let g = store.create("g", false).unwrap();
        assert!(matches!(store.open("g", OpenMode::Shared), Err(ApiError::Locked { holder: None, .. })));
        drop(g);
        let a = store.open("g", OpenMode::Shared).unwrap();
        let mut b = store.open("g", OpenMode::Shared).unwrap();
        assert!(matches!(store.open("g", OpenMode::Exclusive), Err(ApiError::Locked { .. })));
        assert!(matches!(b.upgrade(&store), Err(ApiError::Locked { .. })));
        // The failed upgrade kept b's shared lock: no writer can get in.
        drop(a);
        assert!(matches!(store.open("g", OpenMode::Exclusive), Err(ApiError::Locked { .. })));
        b.upgrade(&store).unwrap();
        assert_eq!(b.mode(), OpenMode::Exclusive);
    }

    #[test]
    fn locked_error_names_live_server() {
        let (_dir, store) = store();
        let g = store.create("g", false).unwrap();
        let info = ServerInfo {
            url: "http://127.0.0.1:7878".into(),
            pid: std::process::id(),
            token_required: false,
        };
        store.write_server_info(&g.location, &info).unwrap();
        match store.open("g", OpenMode::Exclusive) {
            Err(ApiError::Locked { holder, .. }) => assert_eq!(holder.as_deref(), Some("http://127.0.0.1:7878")),
            other => panic!("{other:?}"),
        }
        store.remove_server_info(&g.location);
        assert!(store.server(&g.location).is_none());
    }

    #[test]
    fn legacy_workspace_detected() {
        let (_dir, store) = store();
        fs::create_dir_all(store.dir().join("old")).unwrap();
        fs::write(store.dir().join("old/metadata.json"), "{}").unwrap();
        assert!(matches!(store.open("old", OpenMode::Shared), Err(ApiError::LegacyWorkspace { .. })));
        assert!(matches!(store.create("old", false), Err(ApiError::LegacyWorkspace { .. })));
    }
}
