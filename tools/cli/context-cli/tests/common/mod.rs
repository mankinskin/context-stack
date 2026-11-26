//! Helpers of the CLI integration tests: a temporary store and the binary.

#![allow(dead_code)]

use std::{
    io::{
        BufRead,
        BufReader,
    },
    path::{
        Path,
        PathBuf,
    },
    process::{
        Child,
        Stdio,
    },
    time::{
        Duration,
        Instant,
    },
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
            .env_remove("CONTEXT_TOKEN")
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

/// A `context-cli serve` child process on an ephemeral port.
pub struct ServerProcess {
    pub url: String,
    pub pid: u32,
    child: Option<Child>,
}

impl ServerProcess {
    /// `context-cli --store STORE serve --http 127.0.0.1:0 ARGS...`; waits
    /// for the listening line.
    pub fn start(
        store: &Path,
        args: &[&str],
    ) -> Self {
        let mut child =
            std::process::Command::new(env!("CARGO_BIN_EXE_context-cli"))
                .env_remove("CONTEXT_SERVER")
                .env_remove("CONTEXT_STORE")
                .env_remove("CONTEXT_TOKEN")
                .env("RUST_LOG", "warn")
                .arg("--store")
                .arg(store)
                .args(["serve", "--http", "127.0.0.1:0"])
                .args(args)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .spawn()
                .expect("spawn context-cli serve");
        let mut line = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        let listening: serde_json::Value = serde_json::from_str(&line)
            .unwrap_or_else(|e| panic!("no listening line ({e}): {line:?}"));
        Self {
            url: listening["listening"].as_str().unwrap().to_string(),
            pid: listening["pid"].as_u64().unwrap() as u32,
            child: Some(child),
        }
    }

    /// Graceful shutdown (SIGINT): saves graphs, removes discovery files.
    pub fn stop(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        let Some(mut child) = self.child.take() else {
            return;
        };
        let _ = std::process::Command::new("kill")
            .args(["-INT", &self.pid.to_string()])
            .status();
        let deadline = Instant::now() + Duration::from_secs(30);
        while Instant::now() < deadline {
            if let Ok(Some(_)) = child.try_wait() {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = child.kill();
        let _ = child.wait();
    }
}

impl Drop for ServerProcess {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Replace store paths by `<STORE>` and timestamps by `<TIME>`.
pub fn normalize(
    text: &str,
    stores: &[&Path],
) -> String {
    let mut text = text.to_string();
    for store in stores {
        text = text.replace(&store.display().to_string(), "<STORE>");
    }
    let time = regex::Regex::new(
        r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?(Z|[+-]\d{2}:\d{2})",
    )
    .unwrap();
    time.replace_all(&text, "<TIME>").into_owned()
}
