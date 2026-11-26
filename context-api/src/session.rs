//! The in-process executor shared by every transport.

use std::path::PathBuf;

use crate::{
    command::{
        Request,
        Response,
    },
    error::ApiError,
    event::EventSink,
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
    /// Keep graphs open between commands (server, shell). When false, every
    /// command opens and closes the graphs it uses (one-shot CLI).
    pub keep_open: bool,
    /// `Input::File` paths are allowed below this directory; `None` refuses
    /// file inputs.
    pub allow_fs: Option<PathBuf>,
    /// Hard vertex cap of render/export that `Gate::force` cannot lift.
    pub render_cap: Option<usize>,
    /// Upper bound for `Limits::max_visited`.
    pub max_visited_cap: Option<usize>,
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
        }
    }
}

/// Executes [`Request`]s against the graphs of one store.
///
/// `execute` takes `&self`: graphs live in per-graph slots (a writer mutex
/// for mutations, read guards of `PositionedGraphRef` for queries), so a
/// server can run queries concurrently.
pub struct Session {
    config: SessionConfig,
    events: Option<EventSink>,
}

impl Session {
    pub fn new(config: SessionConfig) -> Self {
        Self {
            config,
            events: None,
        }
    }

    pub fn config(&self) -> &SessionConfig {
        &self.config
    }

    /// Receive events of mutating commands (read progress, graph changed,
    /// saved).
    pub fn set_event_sink(
        &mut self,
        sink: EventSink,
    ) {
        self.events = Some(sink);
    }

    pub fn execute(
        &self,
        request: Request,
    ) -> Response {
        let Request { id, command } = request;
        let _ = &self.events;
        Response::from_result(
            id,
            Err(ApiError::Unsupported(format!(
                "command '{}' is not implemented yet",
                command.name()
            ))),
        )
    }

    /// Save every dirty open graph and release all locks.
    pub fn shutdown(&self) -> Result<(), ApiError> {
        Ok(())
    }
}

/// Executes requests in process or on a server.
pub trait Backend {
    fn execute(
        &mut self,
        request: Request,
    ) -> Response;
}

impl Backend for Session {
    fn execute(
        &mut self,
        request: Request,
    ) -> Response {
        Session::execute(self, request)
    }
}
