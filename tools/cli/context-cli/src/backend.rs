//! Command execution backends: in process ([`LocalBackend`]) or on a
//! server ([`RemoteBackend`]), and the selection between them (plan 04
//! §4.3).

use std::path::{
    Path,
    PathBuf,
};

pub use context_api::Backend;
use context_api::{
    Command,
    CommandResult,
    EventSink,
    Outcome,
    Request,
    Response,
    SaveMode,
    Session,
    SessionConfig,
    client::{
        self,
        RemoteBackend,
    },
    store::{
        GraphStore,
        is_path_ref,
    },
    types::{
        Input,
        ReadReport,
    },
};

/// Texts above this size are sent through the streamed read route.
pub const STREAM_THRESHOLD: usize = 1 << 20;

/// An in-process [`Session`] for one-shot commands: every command opens the
/// graphs it uses in the mode it needs (shared for queries, exclusive for
/// mutations), saves mutations immediately and releases the locks.
pub struct LocalBackend {
    session: Session,
}

impl LocalBackend {
    pub fn new(
        store: PathBuf,
        events: Option<EventSink>,
    ) -> Self {
        let mut config = local_config(store);
        config.keep_open = false;
        config.save = SaveMode::OnWrite;
        let mut session = Session::new(config);
        if let Some(sink) = events {
            session.set_event_sink(sink);
        }
        Self { session }
    }
}

impl Backend for LocalBackend {
    fn execute(
        &self,
        request: Request,
    ) -> Response {
        self.session.execute(request)
    }
}

/// The session configuration of in-process CLI use.
pub fn local_config(store: PathBuf) -> SessionConfig {
    let mut config = SessionConfig::new(store);
    // Files are read by the CLI and sent as text.
    config.allow_fs = None;
    // The local user may address graph files anywhere.
    config.allow_graph_paths = true;
    config
}

/// Where commands run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    Local,
    Remote {
        url: String,
        token: Option<String>,
        /// Pid of the discovered server (`None`: configured explicitly).
        discovered: Option<u32>,
    },
}

impl Selection {
    /// `--local` → local; `--server`/`CONTEXT_SERVER` → remote; a live
    /// server holding `graph` → remote; else local.
    pub fn choose(
        local: bool,
        server: Option<&str>,
        token: Option<&str>,
        store: &Path,
        graph: Option<&str>,
    ) -> Self {
        let token = token.filter(|t| !t.is_empty()).map(str::to_string);
        if local {
            return Selection::Local;
        }
        if let Some(url) = server.filter(|url| !url.is_empty()) {
            return Selection::Remote {
                url: url.to_string(),
                token,
                discovered: None,
            };
        }
        // Servers address graphs by name; path references stay local.
        graph
            .filter(|graph| !is_path_ref(graph))
            .and_then(|graph| client::discover(&GraphStore::new(store), graph))
            .map_or(Selection::Local, |info| Selection::Remote {
                url: info.url,
                token,
                discovered: Some(info.pid),
            })
    }

    /// The stderr note of an auto-discovered server.
    pub fn note(&self) -> Option<String> {
        match self {
            Selection::Remote {
                url,
                discovered: Some(pid),
                ..
            } => Some(format!("using server {url} (graph held by pid {pid})")),
            _ => None,
        }
    }
}

/// Run a command on a remote backend. A `read` with texts larger than
/// [`STREAM_THRESHOLD`] sends each of them through the streamed read
/// route (consecutive small inputs together through `/v1/execute`), in
/// order, and merges the reports into one.
pub fn execute_remote(
    remote: &RemoteBackend,
    request: Request,
) -> Response {
    execute_remote_with(remote, request, STREAM_THRESHOLD)
}

fn execute_remote_with(
    remote: &RemoteBackend,
    request: Request,
    threshold: usize,
) -> Response {
    let id = request.id;
    let Command::Read {
        graph,
        inputs,
        split,
        create,
        save,
        steps: false,
        dry_run: false,
    } = &request.command
    else {
        return remote.execute(request);
    };
    let large = |input: &Input| matches!(input, Input::Text { text, .. } if text.len() > threshold);
    if !inputs.iter().any(large) {
        return remote.execute(request);
    }
    let mut merged: Option<ReadReport> = None;
    let mut batch: Vec<Input> = Vec::new();
    let mut groups: Vec<Vec<Input>> = Vec::new();
    for input in inputs {
        if large(input) {
            if !batch.is_empty() {
                groups.push(std::mem::take(&mut batch));
            }
            groups.push(vec![input.clone()]);
        } else {
            batch.push(input.clone());
        }
    }
    if !batch.is_empty() {
        groups.push(batch);
    }
    for group in groups {
        let response = match group.as_slice() {
            [Input::Text { text, label }] if text.len() > threshold =>
                remote.read_text(graph, label.as_deref(), text, *split, *create),
            _ => remote.execute(Request {
                id,
                command: Command::Read {
                    graph: graph.clone(),
                    inputs: group,
                    split: *split,
                    create: *create,
                    save: *save,
                    steps: false,
                    dry_run: false,
                },
            }),
        };
        let report = match response.outcome {
            Outcome::Ok(CommandResult::Read(report)) => report,
            outcome => return Response { id, outcome },
        };
        merged = Some(match merged {
            None => report,
            Some(mut merged) => {
                merged.inputs.extend(report.inputs);
                merged.skipped.extend(report.skipped);
                merged.vertex_count_after = report.vertex_count_after;
                merged.saved = report.saved;
                merged
            },
        });
    }
    Response {
        id,
        outcome: Outcome::Ok(CommandResult::Read(
            merged.expect("at least one large input"),
        )),
    }
}

#[cfg(all(test, feature = "server"))]
mod tests {
    use context_api::types::{
        ExportFormat,
        Gate,
        SavePolicy,
        Split,
    };

    use super::*;

    fn text(
        text: &str,
        label: &str,
    ) -> Input {
        Input::Text {
            text: text.into(),
            label: Some(label.into()),
        }
    }

    fn read(inputs: Vec<Input>) -> Request {
        Request::with_id(
            7,
            Command::Read {
                graph: "g".into(),
                inputs,
                split: Split::Lines,
                create: true,
                save: SavePolicy::Auto,
                steps: false,
                dry_run: false,
            },
        )
    }

    fn export(backend: &dyn Backend) -> String {
        let response = backend.execute(Request::new(Command::Export {
            graph: "g".into(),
            format: ExportFormat::InterchangeV3,
            gate: Gate {
                force: true,
                ..Gate::default()
            },
        }));
        match response.into_result().unwrap() {
            CommandResult::Export(export) => export.data,
            other => panic!("{other:?}"),
        }
    }

    /// Large texts go through the streamed route one by one, small ones
    /// together; the merged report equals a local read of all inputs.
    #[test]
    fn large_inputs_are_streamed_in_order() {
        let dir = tempfile::tempdir().unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let mut config =
            context_http::ServerConfig::new(dir.path().join("remote"));
        config.addr = "127.0.0.1:0".parse().unwrap();
        let server = runtime
            .block_on(context_http::Server::bind(config))
            .unwrap();
        let remote = RemoteBackend::new(server.url(), None);
        let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
        let task = runtime.spawn(server.run(async {
            let _ = stopped.await;
        }));
        let local = LocalBackend::new(dir.path().join("local"), None);

        let inputs = vec![
            text("ab", "a"),
            text("abcabca\nbcab", "big1"),
            text("", "empty"),
            text("cd", "c"),
            text("xyzxyz\nzy", "big2"),
        ];
        let local_report =
            match local.execute(read(inputs.clone())).into_result() {
                Ok(CommandResult::Read(report)) => report,
                other => panic!("{other:?}"),
            };
        let response = execute_remote_with(&remote, read(inputs), 5);
        assert_eq!(response.id, Some(7));
        let remote_report = match response.into_result() {
            Ok(CommandResult::Read(report)) => report,
            other => panic!("{other:?}"),
        };
        assert_eq!(remote_report.inputs, local_report.inputs);
        assert_eq!(remote_report.skipped, local_report.skipped);
        assert_eq!(remote_report.vertex_count_before, 0);
        assert_eq!(
            remote_report.vertex_count_after,
            local_report.vertex_count_after
        );
        assert!(remote_report.saved.is_some());
        assert_eq!(export(&remote), export(&local));

        // An error stops the sequence and is returned as is.
        let response = execute_remote_with(
            &remote,
            Request::new(Command::Read {
                graph: "missing".into(),
                inputs: vec![text("abcdefgh", "big")],
                split: Split::None,
                create: false,
                save: SavePolicy::Auto,
                steps: false,
                dry_run: false,
            }),
            5,
        );
        assert!(!response.is_ok());

        stop.send(()).unwrap();
        runtime.block_on(task).unwrap().unwrap();
    }
}
