//! Helpers of the CLI integration tests: a temporary store and the binary.

#![allow(dead_code)]

use std::{
    path::{
        Path,
        PathBuf,
    },
    time::Duration,
};

use context_api::{
    ApiError,
    Command,
    CommandResult,
    ErrorKind,
    Outcome,
    Request,
    Response,
    Session,
    store::{
        GraphStore,
        OpenMode,
    },
};
use context_trace::positioned::interchange::PositionedGraphData;

pub struct Store {
    pub dir: tempfile::TempDir,
}

impl Store {
    pub fn new() -> Self {
        Self {
            dir: tempfile::tempdir().unwrap(),
        }
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    /// `<store>/<name>.ctx`.
    pub fn graph_path(
        &self,
        name: &str,
    ) -> PathBuf {
        self.path().join(format!("{name}.ctx"))
    }

    /// `context-cli --store <store>` with a clean environment.
    pub fn cmd(&self) -> assert_cmd::Command {
        let mut cmd =
            assert_cmd::Command::new(env!("CARGO_BIN_EXE_context-cli"));
        cmd.env_remove("CONTEXT_SERVER")
            .env_remove("CONTEXT_STORE")
            .arg("--store")
            .arg(self.path());
        cmd
    }

    /// Run with `--format json`; stdout must be exactly one `Response`.
    pub fn json(
        &self,
        args: &[&str],
    ) -> Response {
        let output = self
            .cmd()
            .arg("--format")
            .arg("json")
            .args(args)
            .output()
            .unwrap();
        parse_response(&output.stdout)
    }

    /// The result of a successful `--format json` run.
    pub fn ok(
        &self,
        args: &[&str],
    ) -> CommandResult {
        let response = self.json(args);
        response
            .into_result()
            .unwrap_or_else(|err| panic!("{args:?} failed: {err}"))
    }

    /// The saved graph as content-keyed data.
    pub fn graph_data(
        &self,
        name: &str,
    ) -> PositionedGraphData {
        graph_data(&GraphStore::new(self.path()), name)
    }
}

pub fn graph_data(
    store: &GraphStore,
    name: &str,
) -> PositionedGraphData {
    let open = retry_locked(|| store.open(name, OpenMode::Shared)).unwrap();
    open.graph.read().to_interchange()
}

/// Retry while the graph is locked. Children spawned by concurrent tests
/// inherit this process's lock file descriptors between fork and exec, so
/// an in-process lock can stay held for a moment after it was released.
pub fn retry_locked<T>(
    mut f: impl FnMut() -> Result<T, ApiError>
) -> Result<T, ApiError> {
    for _ in 0..100 {
        match f() {
            Err(ApiError::Locked { .. }) =>
                std::thread::sleep(Duration::from_millis(20)),
            other => return other,
        }
    }
    f()
}

/// [`retry_locked`] for a session request.
pub fn execute(
    session: &Session,
    command: Command,
) -> Response {
    for _ in 0..100 {
        let response = session.execute(Request::new(command.clone()));
        match &response.outcome {
            Outcome::Error(err) if err.kind == ErrorKind::Locked =>
                std::thread::sleep(Duration::from_millis(20)),
            _ => return response,
        }
    }
    session.execute(Request::new(command))
}

pub fn parse_response(stdout: &[u8]) -> Response {
    let text = std::str::from_utf8(stdout).unwrap();
    assert_eq!(
        text.trim_end().lines().count(),
        1,
        "compact JSON expected: {text}"
    );
    serde_json::from_str(text)
        .unwrap_or_else(|e| panic!("not a Response ({e}): {text}"))
}
