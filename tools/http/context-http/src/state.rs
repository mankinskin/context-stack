//! Shared server state: the session, the event channel and the discovery
//! files of open graphs.

use std::{
    collections::{
        BTreeMap,
        BTreeSet,
    },
    sync::{
        Arc,
        Mutex,
        MutexGuard,
        OnceLock,
    },
};

use context_api::{
    store::{
        GraphLocation,
        ServerInfo,
    },
    types::Limits,
    ApiError,
    Command,
    CommandResult,
    ErrorResponse,
    Event,
    Request,
    Response,
    Session,
};
use tokio::{
    sync::{
        broadcast,
        watch,
    },
    task::JoinHandle,
};

use crate::{
    Error,
    ServerConfig,
    EVENT_CAPACITY,
};

/// State shared by all handlers (cheap to clone).
#[derive(Clone)]
pub struct AppState {
    inner: Arc<Inner>,
}

struct Inner {
    config: ServerConfig,
    session: Arc<Session>,
    events: broadcast::Sender<Event>,
    /// `http://ADDR` once bound; discovery files are written only then.
    url: OnceLock<String>,
    /// Open graphs by name, with the reference that opened them.
    registered: Mutex<BTreeMap<String, Registered>>,
    shutdown: watch::Sender<bool>,
}

struct Registered {
    graph: String,
    location: GraphLocation,
    /// Whether this server wrote the discovery file.
    discovery: bool,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

impl AppState {
    /// Validate the configuration and create the session (graphs open
    /// lazily or by [`crate::Server::bind`]'s preload).
    pub fn new(config: ServerConfig) -> Result<Self, Error> {
        config.validate()?;
        let (events, _) = broadcast::channel(EVENT_CAPACITY);
        let mut session = Session::new(config.session_config());
        let sender = events.clone();
        session.set_event_sink(Arc::new(move |event: &Event| {
            let _ = sender.send(event.clone());
        }));
        let (shutdown, _) = watch::channel(false);
        Ok(Self {
            inner: Arc::new(Inner {
                config,
                session: Arc::new(session),
                events,
                url: OnceLock::new(),
                registered: Mutex::new(BTreeMap::new()),
                shutdown,
            }),
        })
    }

    pub fn config(&self) -> &ServerConfig {
        &self.inner.config
    }

    pub fn session(&self) -> &Arc<Session> {
        &self.inner.session
    }

    /// The server URL written to discovery files.
    pub fn url(&self) -> Option<&str> {
        self.inner.url.get().map(String::as_str)
    }

    pub(crate) fn set_url(
        &self,
        url: String,
    ) {
        let _ = self.inner.url.set(url);
    }

    /// Receive all session events.
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.inner.events.subscribe()
    }

    pub(crate) fn shutdown_receiver(&self) -> watch::Receiver<bool> {
        self.inner.shutdown.subscribe()
    }

    pub(crate) fn notify_shutdown(&self) {
        self.inner.shutdown.send_replace(true);
    }

    /// Execute a request on the blocking pool and update discovery files.
    pub async fn execute(
        &self,
        request: Request,
    ) -> Response {
        let id = request.id;
        let state = self.clone();
        tokio::task::spawn_blocking(move || state.execute_blocking(request))
            .await
            .unwrap_or_else(|err| {
                Response::from_result(
                    id,
                    Err(ApiError::Internal(format!(
                        "execution task failed: {err}"
                    ))),
                )
            })
    }

    fn execute_blocking(
        &self,
        request: Request,
    ) -> Response {
        let graph = request.command.graph().map(str::to_string);
        let response = self.inner.session.execute(request);
        self.sync_discovery(graph.as_deref());
        response
    }

    /// Names of the open graphs.
    pub async fn open_graphs(&self) -> Vec<String> {
        let session = self.inner.session.clone();
        tokio::task::spawn_blocking(move || session.open_graphs())
            .await
            .unwrap_or_default()
    }

    /// Register newly opened graphs (writing `<graph>.ctx.server.json`) and
    /// unregister closed or deleted ones. `graph` is the reference of the
    /// command just executed, so graphs opened by path are located exactly.
    fn sync_discovery(
        &self,
        graph: Option<&str>,
    ) {
        let store = self.inner.session.store();
        let mut registered = lock(&self.inner.registered);
        let open: BTreeSet<String> =
            self.inner.session.open_graphs().into_iter().collect();
        registered.retain(|name, entry| {
            let keep = open.contains(name);
            if !keep && entry.discovery {
                store.remove_server_info(&entry.location);
            }
            keep
        });
        let mut candidates: Vec<String> =
            graph.into_iter().map(str::to_string).collect();
        candidates.extend(open.iter().cloned());
        for graph in candidates {
            let Ok(location) = store.resolve(&graph) else {
                continue;
            };
            if !open.contains(&location.name)
                || registered.contains_key(&location.name)
            {
                continue;
            }
            let discovery = self.url().is_some_and(|url| {
                let info = ServerInfo {
                    url: url.to_string(),
                    pid: std::process::id(),
                    token_required: self.inner.config.token.is_some(),
                };
                match store.write_server_info(&location, &info) {
                    Ok(()) => true,
                    Err(err) => {
                        tracing::warn!(graph = %location.name, %err, "cannot write discovery file");
                        false
                    },
                }
            });
            registered.insert(
                location.name.clone(),
                Registered {
                    graph,
                    location,
                    discovery,
                },
            );
        }
    }

    /// Open the configured preload graphs (any query opens a graph when
    /// graphs are kept open).
    pub(crate) async fn preload(&self) -> Result<(), Error> {
        for graph in self.inner.config.preload.clone() {
            let request = Request::new(Command::Inputs {
                graph: graph.clone(),
                limits: Limits {
                    limit: Some(0),
                    ..Limits::default()
                },
            });
            if let Err(error) = self.execute(request).await.into_result() {
                return Err(Error::Preload { graph, error });
            }
        }
        Ok(())
    }

    /// Start the periodic save task of `save_interval`.
    pub(crate) fn spawn_autosave(&self) -> Option<JoinHandle<()>> {
        let period = self.inner.config.save_interval?;
        let state = self.clone();
        Some(tokio::spawn(async move {
            let mut interval = tokio::time::interval(period);
            interval.set_missed_tick_behavior(
                tokio::time::MissedTickBehavior::Delay,
            );
            interval.tick().await;
            loop {
                interval.tick().await;
                let state = state.clone();
                let _ = tokio::task::spawn_blocking(move || state.save_dirty())
                    .await;
            }
        }))
    }

    /// Save every open graph whose `info` reports unsaved changes.
    fn save_dirty(&self) {
        let graphs: Vec<String> = lock(&self.inner.registered)
            .values()
            .map(|entry| entry.graph.clone())
            .collect();
        for graph in graphs {
            let info =
                self.inner.session.execute(Request::new(Command::Info {
                    graph: graph.clone(),
                }));
            if let Ok(CommandResult::Graph(summary)) = info.into_result() {
                if summary.open && summary.dirty {
                    let saved = self.inner.session.execute(Request::new(
                        Command::Save {
                            graph: graph.clone(),
                        },
                    ));
                    if let Err(err) = saved.into_result() {
                        tracing::warn!(%graph, %err, "periodic save failed");
                    }
                }
            }
        }
    }

    /// Save dirty graphs, release them and remove the discovery files.
    pub(crate) async fn shutdown(&self) -> Result<(), Error> {
        self.notify_shutdown();
        let state = self.clone();
        tokio::task::spawn_blocking(move || {
            let saved = state.inner.session.shutdown();
            let store = state.inner.session.store();
            for (_, entry) in
                std::mem::take(&mut *lock(&state.inner.registered))
            {
                if entry.discovery {
                    store.remove_server_info(&entry.location);
                }
            }
            saved.map_err(|err| Error::Shutdown(ErrorResponse::from(&err)))
        })
        .await
        .unwrap_or_else(|err| {
            Err(Error::Shutdown(ErrorResponse::from(&ApiError::Internal(
                err.to_string(),
            ))))
        })
    }
}
