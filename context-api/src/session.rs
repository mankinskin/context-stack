//! The in-process executor shared by every transport.

use std::{
    collections::HashMap,
    fs,
    panic::{
        self,
        AssertUnwindSafe,
    },
    path::PathBuf,
    sync::{
        Arc,
        Mutex,
        MutexGuard,
        RwLock,
        atomic::{
            AtomicBool,
            Ordering,
        },
    },
};

use context_trace::positioned::{
    GraphRead,
    NodeRef,
    PositionedGraphRef,
    VertexRead,
};

use crate::{
    command::{
        Command,
        CommandResult,
        Request,
        Response,
    },
    error::ApiError,
    event::{
        Event,
        EventSink,
    },
    ops::{
        self,
        View,
        read::Item,
    },
    store::{
        GraphLocation,
        GraphStore,
        OpenGraph,
        OpenMode,
        file::GraphMeta,
    },
    types::{
        GraphSummary,
        ReadReport,
        SavePolicy,
    },
};

/// When `SavePolicy::Auto` saves.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SaveMode {
    /// Save after every mutating command (one-shot CLI, default server).
    #[default]
    OnWrite,
    /// Save only on `save`, `close` and [`Session::shutdown`].
    Manual,
}

#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// Directory of `<name>.ctx` graphs.
    pub store_dir: PathBuf,
    pub save: SaveMode,
    /// Keep graphs open (exclusively locked) between commands (server,
    /// shell). When false, every command opens and closes the graphs it
    /// uses (one-shot CLI).
    pub keep_open: bool,
    /// `Input::File` paths are allowed below this directory; `None` refuses
    /// file inputs.
    pub allow_fs: Option<PathBuf>,
    /// Hard vertex cap of render/export that `Gate::force` cannot lift.
    pub render_cap: Option<usize>,
    /// Upper bound for `Limits::max_visited`.
    pub max_visited_cap: Option<usize>,
    /// Accept graph references that are paths (`*.ctx`) anywhere. When false,
    /// paths are accepted only below `allow_fs`; names always work.
    pub allow_graph_paths: bool,
}

impl SessionConfig {
    pub fn new(store_dir: impl Into<PathBuf>) -> Self {
        Self {
            store_dir: store_dir.into(),
            save: SaveMode::OnWrite,
            keep_open: false,
            allow_fs: None,
            render_cap: None,
            max_visited_cap: None,
            allow_graph_paths: false,
        }
    }
}

/// An open graph shared by concurrent commands.
///
/// Mutations hold `open` (the writer slot) for their whole duration;
/// queries never take it: they read `graph` (the reader commits per step
/// under its write lock) and the metadata snapshot published after every
/// mutation.
struct GraphSlot {
    location: GraphLocation,
    graph: PositionedGraphRef,
    meta: RwLock<Arc<GraphMeta>>,
    dirty: AtomicBool,
    open: Mutex<OpenGraph>,
}

impl GraphSlot {
    fn new(open: OpenGraph) -> Self {
        Self {
            location: open.location.clone(),
            graph: open.graph.clone(),
            meta: RwLock::new(Arc::new(open.meta.clone())),
            dirty: AtomicBool::new(open.is_dirty()),
            open: Mutex::new(open),
        }
    }

    fn meta(&self) -> Arc<GraphMeta> {
        self.meta.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn publish(
        &self,
        open: &OpenGraph,
    ) {
        *self.meta.write().unwrap_or_else(|e| e.into_inner()) = Arc::new(open.meta.clone());
        self.dirty.store(open.is_dirty(), Ordering::SeqCst);
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// Executes [`Request`]s against the graphs of one store.
///
/// `execute` takes `&self`, so a server can run queries concurrently.
pub struct Session {
    config: SessionConfig,
    store: GraphStore,
    events: Option<EventSink>,
    graphs: Mutex<HashMap<PathBuf, Arc<GraphSlot>>>,
}

impl Session {
    pub fn new(config: SessionConfig) -> Self {
        Self {
            store: GraphStore::new(config.store_dir.clone()),
            config,
            events: None,
            graphs: Mutex::new(HashMap::new()),
        }
    }

    pub fn config(&self) -> &SessionConfig {
        &self.config
    }

    pub fn store(&self) -> &GraphStore {
        &self.store
    }

    /// Receive events of mutating commands (read progress, graph changed,
    /// saved).
    pub fn set_event_sink(
        &mut self,
        sink: EventSink,
    ) {
        self.events = Some(sink);
    }

    fn emit(
        &self,
        event: Event,
    ) {
        if let Some(sink) = &self.events {
            sink(&event);
        }
    }

    /// Run a request. Panics of the algorithms are caught: the in-memory
    /// state of the affected graph is dropped (the next command reloads its
    /// last save) and an internal error is returned.
    pub fn execute(
        &self,
        request: Request,
    ) -> Response {
        let Request { id, command } = request;
        let graph = command.graph().map(str::to_string);
        let checked = match &graph {
            Some(graph) => self.check_graph_ref(graph),
            None => Ok(()),
        };
        if let Err(err) = checked {
            return Response::from_result(id, Err(err));
        }
        let result = panic::catch_unwind(AssertUnwindSafe(|| self.run(id, command)))
            .unwrap_or_else(|payload| {
                let message = payload
                    .downcast_ref::<&str>()
                    .map(|s| s.to_string())
                    .or_else(|| payload.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "unknown panic".into());
                if let Some(graph) = &graph {
                    self.forget(graph);
                }
                Err(ApiError::Internal(format!(
                    "command panicked: {message}; unsaved changes of the graph were dropped"
                )))
            });
        Response::from_result(id, result)
    }

    /// Save every dirty open graph and close all graphs.
    pub fn shutdown(&self) -> Result<(), ApiError> {
        let slots: Vec<_> = lock(&self.graphs).drain().map(|(_, slot)| slot).collect();
        let mut first_error = None;
        for slot in slots {
            let mut open = lock(&slot.open);
            if open.is_dirty() {
                match open.save() {
                    Ok(_) => self.emit(Event::Saved {
                        graph: slot.location.name.clone(),
                        generation: open.meta.generation,
                    }),
                    Err(err) => {
                        first_error.get_or_insert(err);
                    },
                }
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    /// Names of the graphs open in this session.
    pub fn open_graphs(&self) -> Vec<String> {
        let mut names: Vec<_> = lock(&self.graphs)
            .values()
            .map(|slot| slot.location.name.clone())
            .collect();
        names.sort();
        names
    }

    fn forget(
        &self,
        graph: &str,
    ) {
        if let Ok(location) = self.store.resolve(graph) {
            lock(&self.graphs).remove(&location.path);
        }
    }

    // ── slots ───────────────────────────────────────────────────────

    fn open_slot(
        &self,
        location: &GraphLocation,
    ) -> Option<Arc<GraphSlot>> {
        lock(&self.graphs).get(&location.path).cloned()
    }

    /// The open graph, opening it (shared or exclusive by `mode`; always
    /// exclusive when graphs are kept open).
    fn slot(
        &self,
        graph: &str,
        mode: OpenMode,
    ) -> Result<Arc<GraphSlot>, ApiError> {
        let location = self.store.resolve(graph)?;
        if let Some(slot) = self.open_slot(&location) {
            // Not under the `graphs` lock: a running read holds `open`.
            if mode == OpenMode::Exclusive {
                lock(&slot.open).upgrade(&self.store)?;
            }
            return Ok(slot);
        }
        let mut graphs = lock(&self.graphs);
        if let Some(slot) = graphs.get(&location.path) {
            // Opened concurrently; kept-open slots are exclusive.
            return Ok(slot.clone());
        }
        let mode = if self.config.keep_open {
            OpenMode::Exclusive
        } else {
            mode
        };
        let slot = Arc::new(GraphSlot::new(self.store.open(graph, mode)?));
        if self.config.keep_open {
            graphs.insert(location.path, slot.clone());
        }
        Ok(slot)
    }

    /// Close this session's slot of a graph that is about to be replaced or
    /// deleted; `Busy` (and the slot kept) while another command uses it.
    fn release_slot(
        &self,
        location: &GraphLocation,
    ) -> Result<(), ApiError> {
        let mut graphs = lock(&self.graphs);
        if let Some(slot) = graphs.get(&location.path) {
            // Clones are only made under `graphs`, so the count cannot grow here.
            if Arc::strong_count(slot) > 1 {
                return Err(ApiError::Busy {
                    graph: location.name.clone(),
                });
            }
            graphs.remove(&location.path);
        }
        Ok(())
    }

    /// Graph references that are paths must be allowed by the config.
    fn check_graph_ref(
        &self,
        graph: &str,
    ) -> Result<(), ApiError> {
        let location = self.store.resolve(graph)?;
        if !crate::store::is_path_ref(graph) || self.config.allow_graph_paths {
            return Ok(());
        }
        let refused = || {
            ApiError::FileAccess(format!(
                "graph paths are not allowed here; use a graph name instead of '{graph}'"
            ))
        };
        let root = self.config.allow_fs.as_ref().ok_or_else(refused)?.canonicalize()?;
        let parent = location
            .path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or(std::path::Path::new("."));
        let parent = parent.canonicalize().map_err(|_| refused())?;
        if parent.starts_with(&root) {
            Ok(())
        } else {
            Err(refused())
        }
    }

    /// Create a graph (empty, saved) and open it.
    fn create_slot(
        &self,
        graph: &str,
        overwrite: bool,
    ) -> Result<Arc<GraphSlot>, ApiError> {
        self.create_slot_with(graph, overwrite, true)
    }

    /// [`Self::create_slot`]; with `save` false the file is written by the
    /// command's first save (atomic `create`).
    fn create_slot_with(
        &self,
        graph: &str,
        overwrite: bool,
        save: bool,
    ) -> Result<Arc<GraphSlot>, ApiError> {
        let location = self.store.resolve(graph)?;
        if self.open_slot(&location).is_some() {
            if !overwrite {
                return Err(ApiError::GraphExists {
                    graph: location.name,
                });
            }
            self.release_slot(&location)?;
        }
        let mut graphs = lock(&self.graphs);
        let open = if save {
            self.store.create(graph, overwrite)?
        } else {
            self.store.create_unsaved(graph, overwrite)?
        };
        let slot = Arc::new(GraphSlot::new(open));
        if self.config.keep_open {
            graphs.insert(location.path, slot.clone());
        }
        Ok(slot)
    }

    /// Summary of a slot this command already holds.
    fn slot_summary(slot: &GraphSlot) -> GraphSummary {
        let meta = slot.meta();
        let guard = slot.graph.read();
        let view = View {
            name: &slot.location.name,
            graph: &guard,
            meta: &meta,
        };
        Self::live_summary(&view, slot)
    }

    fn with_view<T>(
        &self,
        graph: &str,
        f: impl FnOnce(&View<'_>, &GraphSlot) -> Result<T, ApiError>,
    ) -> Result<T, ApiError> {
        let slot = self.slot(graph, OpenMode::Shared)?;
        let meta = slot.meta();
        let guard = slot.graph.read();
        let view = View {
            name: &slot.location.name,
            graph: &guard,
            meta: &meta,
        };
        f(&view, &slot)
    }

    fn live_summary(
        view: &View<'_>,
        slot: &GraphSlot,
    ) -> GraphSummary {
        let vertex_count = view.graph.vertex_count();
        let atom_count = view
            .graph
            .vertices()
            .filter(|vertex| matches!(vertex.node(), NodeRef::Atom(_)))
            .count();
        GraphSummary {
            name: slot.location.name.clone(),
            path: slot.location.path.display().to_string(),
            vertex_count,
            atom_count,
            composite_count: vertex_count - atom_count,
            input_count: view.meta.inputs.len(),
            total_input_chars: view.meta.total_input_chars(),
            generation: view.graph.generation().base,
            created_at: view.meta.created_at.clone(),
            modified_at: view.meta.modified_at.clone(),
            file_size: fs::metadata(&slot.location.path).ok().map(|m| m.len()),
            open: true,
            dirty: slot.dirty.load(Ordering::SeqCst),
        }
    }

    fn summary(
        &self,
        graph: &str,
    ) -> Result<GraphSummary, ApiError> {
        let location = self.store.resolve(graph)?;
        match self.open_slot(&location) {
            Some(_) => self.with_view(graph, |view, slot| Ok(Self::live_summary(view, slot))),
            None => self.store.summary(graph),
        }
    }

    fn should_save(
        &self,
        policy: SavePolicy,
    ) -> bool {
        match policy {
            SavePolicy::Now => true,
            SavePolicy::Defer => false,
            SavePolicy::Auto => self.config.save == SaveMode::OnWrite,
        }
    }

    // ── commands ────────────────────────────────────────────────────

    fn run(
        &self,
        id: Option<u64>,
        command: Command,
    ) -> Result<CommandResult, ApiError> {
        match command {
            Command::New { graph, overwrite } => {
                let slot = self.create_slot(&graph, overwrite)?;
                Ok(CommandResult::Graph(Self::slot_summary(&slot)))
            },
            Command::List => {
                let mut graphs = self.store.list()?;
                for summary in &mut graphs {
                    if let Ok(location) = self.store.resolve(&summary.name) {
                        if self.open_slot(&location).is_some() {
                            *summary = self.summary(&summary.name)?;
                        }
                    }
                }
                Ok(CommandResult::Graphs { graphs })
            },
            Command::Info { graph } => Ok(CommandResult::Graph(self.summary(&graph)?)),
            Command::Delete { graph } => {
                self.release_slot(&self.store.resolve(&graph)?)?;
                self.store.delete(&graph)?;
                Ok(CommandResult::Deleted {
                    graph: self.store.resolve(&graph)?.name,
                })
            },
            Command::Save { graph } => {
                let slot = self.slot(&graph, OpenMode::Exclusive)?;
                let mut open = lock(&slot.open);
                let report = open.save()?;
                slot.publish(&open);
                self.emit(Event::Saved {
                    graph: slot.location.name.clone(),
                    generation: report.generation,
                });
                Ok(CommandResult::Saved(report))
            },
            Command::Close { graph, save } => {
                let location = self.store.resolve(&graph)?;
                let slot = lock(&self.graphs).remove(&location.path);
                let mut saved = None;
                if let Some(slot) = slot {
                    let mut open = lock(&slot.open);
                    if save && open.is_dirty() {
                        saved = Some(open.save()?);
                    }
                }
                Ok(CommandResult::Closed {
                    graph: location.name,
                    saved,
                })
            },
            Command::Read {
                graph,
                inputs,
                split,
                create,
                save,
                steps,
                dry_run,
            } => {
                let items =
                    ops::read::resolve_inputs(inputs, split, self.config.allow_fs.as_deref())?;
                self.read(id, &graph, items, create, save, steps, dry_run)
                    .map(CommandResult::Read)
            },
            Command::Create {
                graph,
                inputs,
                split,
                overwrite,
            } => {
                let items =
                    ops::read::resolve_inputs(inputs, split, self.config.allow_fs.as_deref())?;
                let slot = self.create_slot_with(&graph, overwrite, false)?;
                // Not kept open: the read must reuse this slot and its lock.
                let created = (!self.config.keep_open).then_some(slot);
                self.read_into(id, &graph, created, items, false, SavePolicy::Now, false, false)
                    .map(CommandResult::Read)
            },
            Command::Search {
                graph,
                query,
                view: search_view,
                limits,
            } => self.with_view(&graph, |view, _| {
                ops::search::search(view, &query, search_view, &limits, self.config.max_visited_cap)
                    .map(CommandResult::Search)
            }),
            Command::Vertex {
                graph,
                vertex,
                limits,
            } => self.with_view(&graph, |view, _| {
                ops::info::vertex(view, &vertex, &limits).map(CommandResult::Vertex)
            }),
            Command::Inputs { graph, limits } => self.with_view(&graph, |view, _| {
                Ok(CommandResult::Inputs(ops::info::inputs(view, &limits)))
            }),
            Command::Stats { graph } => self.with_view(&graph, |view, slot| {
                let summary = Self::live_summary(view, slot);
                Ok(CommandResult::Stats(ops::info::stats(view, summary)))
            }),
            Command::Validate { graph } => self.with_view(&graph, |view, _| {
                Ok(CommandResult::Validation(ops::info::validate(view)))
            }),
            Command::Render {
                graph,
                format,
                root,
                depth,
                gate,
                limits,
            } => self.with_view(&graph, |view, _| {
                ops::render::render(
                    view,
                    format,
                    root.as_ref(),
                    depth,
                    &gate,
                    &limits,
                    self.config.render_cap,
                )
                .map(CommandResult::Render)
            }),
            Command::Export {
                graph,
                format,
                gate,
            } => self.with_view(&graph, |view, _| {
                ops::export::export(view, format, &gate, self.config.render_cap)
                    .map(CommandResult::Export)
            }),
            Command::Import {
                graph,
                data,
                overwrite,
            } => {
                let location = self.store.resolve(&graph)?;
                let (meta, imported) = ops::export::import(&location.name, data)?;
                let slot = self.create_slot(&graph, overwrite)?;
                let mut open = lock(&slot.open);
                *slot.graph.write() = imported;
                open.meta.inputs = meta.inputs;
                open.mark_dirty();
                open.save()?;
                slot.publish(&open);
                drop(open);
                Ok(CommandResult::Graph(Self::slot_summary(&slot)))
            },
            #[cfg(feature = "logs")]
            command => self.run_log_command(command),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn read(
        &self,
        id: Option<u64>,
        graph: &str,
        items: Vec<Item>,
        create: bool,
        save: SavePolicy,
        steps: bool,
        dry_run: bool,
    ) -> Result<ReadReport, ApiError> {
        self.read_into(id, graph, None, items, create, save, steps, dry_run)
    }

    /// [`Self::read`] into `slot` if given (a slot this command created),
    /// else into the session's slot of `graph`.
    #[allow(clippy::too_many_arguments)]
    fn read_into(
        &self,
        id: Option<u64>,
        graph: &str,
        slot: Option<Arc<GraphSlot>>,
        items: Vec<Item>,
        create: bool,
        save: SavePolicy,
        steps: bool,
        dry_run: bool,
    ) -> Result<ReadReport, ApiError> {
        let mode = if dry_run {
            OpenMode::Shared
        } else {
            OpenMode::Exclusive
        };
        let found = match slot {
            Some(slot) => Ok(slot),
            None => self.slot(graph, mode),
        };
        let slot = match found {
            Err(ApiError::GraphNotFound { .. }) if create && !dry_run =>
                self.create_slot(graph, false)?,
            Err(ApiError::GraphNotFound { graph: name }) if create => {
                // Dry run on a missing graph: read into an empty graph.
                let mut meta = GraphMeta::new(&name);
                let graph = PositionedGraphRef::default();
                let outcome = ops::read::read_items(&graph, &mut meta, items, steps, &name, id, None);
                return Ok(ReadReport {
                    graph: name,
                    inputs: outcome.inputs,
                    skipped: outcome.skipped,
                    vertex_count_before: 0,
                    vertex_count_after: graph.read().vertex_count(),
                    saved: None,
                    dry_run: true,
                });
            },
            result => result?,
        };
        let name = slot.location.name.clone();

        if dry_run {
            let graph = ops::read::copy(&slot.graph);
            let mut meta = GraphMeta::clone(&slot.meta());
            let before = graph.read().vertex_count();
            let outcome = ops::read::read_items(&graph, &mut meta, items, steps, &name, id, None);
            return Ok(ReadReport {
                graph: name,
                inputs: outcome.inputs,
                skipped: outcome.skipped,
                vertex_count_before: before,
                vertex_count_after: graph.read().vertex_count(),
                saved: None,
                dry_run: true,
            });
        }

        let mut open = lock(&slot.open);
        let before = slot.graph.read().vertex_count();
        let emit = |event: Event| self.emit(event);
        let emit: Option<&dyn Fn(Event)> = self.events.as_ref().map(|_| &emit as &dyn Fn(Event));
        let mut meta = open.meta.clone();
        let outcome = ops::read::read_items(&slot.graph, &mut meta, items, steps, &name, id, emit);
        open.meta = meta;
        let (after, generation) = {
            let graph = slot.graph.read();
            (graph.vertex_count(), graph.generation().base)
        };
        if !outcome.inputs.is_empty() {
            open.mark_dirty();
            slot.publish(&open);
            self.emit(Event::GraphChanged {
                graph: name.clone(),
                generation,
                vertex_count: after,
                new_inputs: outcome.inputs.iter().map(|input| input.id).collect(),
            });
        }
        let saved = if open.is_dirty() && self.should_save(save) {
            let report = open.save()?;
            slot.publish(&open);
            self.emit(Event::Saved {
                graph: name.clone(),
                generation: report.generation,
            });
            Some(report)
        } else {
            None
        };
        Ok(ReadReport {
            graph: name,
            inputs: outcome.inputs,
            skipped: outcome.skipped,
            vertex_count_before: before,
            vertex_count_after: after,
            saved,
            dry_run: false,
        })
    }

    #[cfg(feature = "logs")]
    fn run_log_command(
        &self,
        command: Command,
    ) -> Result<CommandResult, ApiError> {
        use crate::logs;
        let dir = |graph: &str| -> Result<PathBuf, ApiError> {
            Ok(self.store.resolve(graph)?.logs_dir())
        };
        Ok(match command {
            Command::ListLogs {
                graph,
                pattern,
                limit,
            } => CommandResult::LogList {
                logs: logs::list_logs(&dir(&graph)?, pattern.as_deref(), limit)?,
            },
            Command::GetLog {
                graph,
                filename,
                filter,
                limit,
                offset,
            } => {
                let (entries, total) =
                    logs::get_log(&dir(&graph)?, &filename, filter.as_deref(), limit, offset)?;
                CommandResult::LogEntries {
                    filename,
                    total,
                    offset,
                    limit,
                    returned: entries.len(),
                    entries,
                }
            },
            Command::QueryLog {
                graph,
                filename,
                query,
                limit,
            } => {
                let (entries, matches) = logs::query_log(&dir(&graph)?, &filename, &query, limit)?;
                CommandResult::LogQueryResult {
                    query,
                    matches,
                    entries,
                }
            },
            Command::AnalyzeLog { graph, filename } =>
                CommandResult::LogAnalysis(logs::analyze_log(&dir(&graph)?, &filename)?),
            Command::SearchLogs {
                graph,
                query,
                limit_per_file,
            } => {
                let (results, files_with_matches) =
                    logs::search_logs(&dir(&graph)?, &query, limit_per_file)?;
                CommandResult::LogSearchResult {
                    query,
                    files_with_matches,
                    results,
                }
            },
            Command::DeleteLog { graph, filename } => {
                logs::delete_log(&dir(&graph)?, &filename)?;
                CommandResult::LogDeleteResult(crate::types::logs::LogDeleteResult {
                    deleted_count: 1,
                    freed_bytes: 0,
                })
            },
            Command::DeleteLogs {
                graph,
                older_than_days,
            } => CommandResult::LogDeleteResult(logs::delete_logs(&dir(&graph)?, older_than_days)?),
            other => unreachable!("not a log command: {}", other.name()),
        })
    }
}

/// Executes requests in process or on a server. Implementations are
/// shared between threads and run requests concurrently.
pub trait Backend: Send + Sync {
    fn execute(
        &self,
        request: Request,
    ) -> Response;
}

impl Backend for Session {
    fn execute(
        &self,
        request: Request,
    ) -> Response {
        Session::execute(self, request)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex as StdMutex;

    use super::*;
    use crate::{
        error::ErrorKind,
        types::{
            Input,
            Split,
        },
    };

    fn session(keep_open: bool) -> (tempfile::TempDir, Session) {
        let dir = tempfile::tempdir().unwrap();
        let mut config = SessionConfig::new(dir.path());
        config.keep_open = keep_open;
        (dir, Session::new(config))
    }

    fn run(
        session: &Session,
        command: Command,
    ) -> Result<CommandResult, crate::ErrorResponse> {
        session.execute(Request::new(command)).into_result()
    }

    fn text(t: &str) -> Input {
        Input::Text {
            text: t.into(),
            label: None,
        }
    }

    fn read(
        graph: &str,
        inputs: Vec<Input>,
    ) -> Command {
        Command::Read {
            graph: graph.into(),
            inputs,
            split: Split::None,
            create: false,
            save: SavePolicy::Auto,
            steps: false,
            dry_run: false,
        }
    }

    fn info(
        session: &Session,
        graph: &str,
    ) -> GraphSummary {
        match run(session, Command::Info { graph: graph.into() }).unwrap() {
            CommandResult::Graph(summary) => summary,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn one_shot_create_read_info() {
        let (_dir, session) = session(false);
        run(&session, Command::New { graph: "g".into(), overwrite: false }).unwrap();
        let report = match run(&session, read("g", vec![text("abcabca"), text("")])).unwrap() {
            CommandResult::Read(report) => report,
            other => panic!("{other:?}"),
        };
        assert_eq!(report.inputs.len(), 1);
        assert_eq!(report.inputs[0].token.label, "abcabca");
        assert_eq!(report.skipped, ["<text>"]);
        assert!(report.saved.is_some());
        let summary = info(&session, "g");
        assert_eq!((summary.input_count, summary.total_input_chars), (1, 7));
        assert_eq!(summary.vertex_count, report.vertex_count_after);
        assert!(!summary.open);
    }

    #[test]
    fn errors_map_to_kinds() {
        let (_dir, session) = session(false);
        assert_eq!(run(&session, read("missing", vec![text("a")])).unwrap_err().kind, ErrorKind::NotFound);
        let mut create = read("missing", vec![text("ab")]);
        if let Command::Read { create, .. } = &mut create {
            *create = true;
        }
        run(&session, create).unwrap();
        let err = run(&session, Command::New { graph: "missing".into(), overwrite: false }).unwrap_err();
        assert_eq!(err.kind, ErrorKind::AlreadyExists);
        let err = run(&session, read("missing", vec![Input::File { path: "/etc/hostname".into() }])).unwrap_err();
        assert_eq!(err.kind, ErrorKind::Forbidden);
    }

    #[test]
    fn dry_run_changes_nothing() {
        let (_dir, session) = session(false);
        run(&session, Command::Create { graph: "g".into(), inputs: vec![text("abab")], split: Split::None, overwrite: false }).unwrap();
        let before = info(&session, "g");
        let mut dry = read("g", vec![text("abcabc")]);
        if let Command::Read { dry_run, .. } = &mut dry {
            *dry_run = true;
        }
        match run(&session, dry).unwrap() {
            CommandResult::Read(report) => {
                assert!(report.dry_run && report.saved.is_none());
                assert!(report.vertex_count_after > report.vertex_count_before);
            },
            other => panic!("{other:?}"),
        }
        let after = info(&session, "g");
        assert_eq!((before.vertex_count, before.input_count), (after.vertex_count, after.input_count));
    }

    #[test]
    fn kept_open_graphs_lock_other_sessions_and_save_on_shutdown() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = SessionConfig::new(dir.path());
        config.keep_open = true;
        config.save = SaveMode::Manual;
        let mut server = Session::new(config);
        let events = Arc::new(StdMutex::new(Vec::new()));
        let sink = events.clone();
        server.set_event_sink(Arc::new(move |event: &Event| sink.lock().unwrap().push(event.clone())));
        run(&server, Command::New { graph: "g".into(), overwrite: false }).unwrap();
        run(&server, read("g", vec![text("abcabc")])).unwrap();
        assert!(info(&server, "g").dirty);

        let (_other_dir, _) = session(false);
        let client = Session::new(SessionConfig::new(dir.path()));
        let err = run(&client, read("g", vec![text("x")])).unwrap_err();
        assert_eq!(err.kind, ErrorKind::Locked);

        server.shutdown().unwrap();
        assert_eq!(info(&client, "g").input_count, 1);
        let events = events.lock().unwrap();
        assert!(events.iter().any(|e| matches!(e, Event::GraphChanged { new_inputs, .. } if new_inputs == &[0])));
        assert!(events.iter().any(|e| matches!(e, Event::ReadProgress { chars: 6, total: 6, .. })));
        assert!(events.iter().any(|e| matches!(e, Event::Saved { .. })));
    }

    #[test]
    fn import_in_one_shot_mode() {
        let (_dir, session) = session(false);
        run(&session, Command::Create { graph: "a".into(), inputs: vec![text("abcabc")], split: Split::None, overwrite: false }).unwrap();
        let data = match run(&session, Command::Export { graph: "a".into(), format: crate::types::ExportFormat::Native, gate: Default::default() }).unwrap() {
            CommandResult::Export(export) => export.data,
            other => panic!("{other:?}"),
        };
        let imported = run(&session, Command::Import { graph: "b".into(), data: crate::types::ImportData::Native { base64: data }, overwrite: false }).unwrap();
        assert!(matches!(imported, CommandResult::Graph(summary) if summary.input_count == 1));
        assert_eq!(info(&session, "b").vertex_count, info(&session, "a").vertex_count);
    }

    #[test]
    fn busy_graphs_are_kept_open() {
        let (_dir, session) = session(true);
        let mut config = session.config().clone();
        config.save = SaveMode::Manual;
        let session = Session::new(config);
        run(&session, Command::New { graph: "g".into(), overwrite: false }).unwrap();
        run(&session, read("g", vec![text("abab")])).unwrap();
        let in_use = session.slot("g", OpenMode::Shared).unwrap();
        for command in [
            Command::Delete { graph: "g".into() },
            Command::New { graph: "g".into(), overwrite: true },
        ] {
            let err = run(&session, command).unwrap_err();
            assert_eq!(err.kind, ErrorKind::Locked);
            assert!(err.message.contains("in use"), "{}", err.message);
        }
        drop(in_use);
        let summary = info(&session, "g");
        assert!(summary.open && summary.dirty && summary.input_count == 1);
        run(&session, Command::Delete { graph: "g".into() }).unwrap();
    }

    #[test]
    fn graph_paths_need_permission() {
        let (dir, session) = session(false);
        let outside = tempfile::tempdir().unwrap();
        let path = outside.path().join("x.ctx").display().to_string();
        let err = run(&session, Command::New { graph: path.clone(), overwrite: false }).unwrap_err();
        assert_eq!(err.kind, ErrorKind::Forbidden);
        run(&session, Command::New { graph: "notes-ctx".into(), overwrite: false }).unwrap();

        let mut config = SessionConfig::new(dir.path());
        config.allow_fs = Some(dir.path().to_path_buf());
        let below = Session::new(config.clone());
        let inside = dir.path().join("sub.ctx").display().to_string();
        run(&below, Command::New { graph: inside, overwrite: false }).unwrap();
        assert_eq!(run(&below, Command::New { graph: path.clone(), overwrite: false }).unwrap_err().kind, ErrorKind::Forbidden);

        config.allow_graph_paths = true;
        run(&Session::new(config), Command::New { graph: path, overwrite: false }).unwrap();
    }

    #[test]
    fn vertex_inputs_validate() {
        let (_dir, session) = session(false);
        run(&session, Command::Create { graph: "g".into(), inputs: vec![text("abcabca"), text("bcab")], split: Split::None, overwrite: false }).unwrap();
        match run(&session, Command::Vertex { graph: "g".into(), vertex: crate::types::VertexRef::Text("abca".into()), limits: Default::default() }).unwrap() {
            CommandResult::Vertex(vertex) => {
                assert_eq!(vertex.token.label, "abca");
                assert!(!vertex.is_atom && !vertex.parents.is_empty());
            },
            other => panic!("{other:?}"),
        }
        match run(&session, Command::Validate { graph: "g".into() }).unwrap() {
            CommandResult::Validation(report) => assert!(report.valid, "{report:?}"),
            other => panic!("{other:?}"),
        }
        match run(&session, Command::Inputs { graph: "g".into(), limits: Default::default() }).unwrap() {
            CommandResult::Inputs(list) => assert_eq!(list.items.iter().map(|i| i.width).collect::<Vec<_>>(), [7, 4]),
            other => panic!("{other:?}"),
        }
    }
}
