//! Client mode (plan 04 §4.5): `--server`, auto-discovery, the shell, events
//! and tokens, against `context-cli serve` child processes.

#![cfg(feature = "server")]

mod common;

use std::{
    fs,
    io::{
        BufRead,
        BufReader,
        Write,
    },
    path::Path,
    process::Stdio,
    sync::mpsc,
    time::Duration,
};

use common::{
    ServerProcess,
    Store,
    normalize,
};
use context_api::{
    Backend,
    Command,
    CommandResult,
    ErrorKind,
    Event,
    Outcome,
    Request,
    Topic,
    client::{
        RemoteBackend,
        discover,
    },
    store::GraphStore,
    types::{
        Input,
        SavePolicy,
        Split,
    },
};
use predicates::prelude::*;

/// Output of one CLI run, normalized.
#[derive(Debug, PartialEq, Eq)]
struct Run {
    code: i32,
    stdout: String,
    stderr: String,
}

fn run(
    mut cmd: assert_cmd::Command,
    args: &[&str],
    stores: &[&Path],
) -> Run {
    let output = cmd.args(args).output().unwrap();
    Run {
        code: output.status.code().unwrap_or(-1),
        stdout: normalize(&String::from_utf8_lossy(&output.stdout), stores),
        stderr: normalize(&String::from_utf8_lossy(&output.stderr), stores),
    }
}

/// The stage-2 scenarios of one graph, in order (mutations included).
fn scenario(
    graph: &str,
    file: &str,
) -> Vec<Vec<String>> {
    let lines = [
        format!("create {graph} abcabca bcab -f {file}"),
        format!("read {graph} -t xyzxyz -t the_brown_fox"),
        format!("read {graph} --dry-run -t qqq"),
        format!("search {graph} bca"),
        format!("search {graph} bca --view occurrences --width 3"),
        format!("search {graph} bca --view contexts"),
        format!("search {graph} bca --view parents"),
        format!("search {graph} bca --view path"),
        format!("search {graph} zzz --fail-if-missing"),
        format!("vertex {graph} bca"),
        format!("vertex {graph} #3"),
        format!("vertex {graph} cabc"),
        format!("inputs {graph}"),
        format!("info {graph}"),
        format!("stats {graph}"),
        format!("validate {graph}"),
        format!("show {graph} --root abcabca --format mermaid"),
        format!("show {graph} --root bcab --format json"),
        format!("show {graph}"),
        format!("export {graph} --format v3"),
        "list".to_string(),
        format!("search {graph}-missing x"),
        format!("new {graph}-tmp"),
        format!("new {graph}-tmp"),
        format!("delete {graph}-tmp"),
        format!("read {graph}-new --create -t abab"),
    ];
    let mut steps: Vec<Vec<String>> = lines
        .iter()
        .map(|line| line.split(' ').map(str::to_string).collect())
        .collect();
    steps.insert(
        2,
        ["read", graph, "--split", "lines", "-t", "one\ntwo"]
            .map(str::to_string)
            .to_vec(),
    );
    steps
}

#[test]
fn client_equivalence() {
    let local = Store::new();
    let remote = Store::new();
    let files = tempfile::tempdir().unwrap();
    let file = files.path().join("notes.txt");
    fs::write(&file, "the brown fox\nthe brown dog").unwrap();
    let file = file.display().to_string();
    let server = ServerProcess::start(remote.path(), &[]);
    let stores = [local.path(), remote.path()];
    for (graph, format) in [("t", "text"), ("j", "json")] {
        for words in scenario(graph, &file) {
            let args: Vec<&str> = ["--format", format]
                .into_iter()
                .chain(words.iter().map(String::as_str))
                .collect();
            let mut local_cmd = local.cmd();
            local_cmd.arg("--local");
            let mut remote_cmd = remote.cmd();
            remote_cmd.args(["--server", &server.url]);
            // The one session-specific field: whether the executing
            // session has the graph open (a server keeps graphs open).
            let open = |mut run: Run| {
                run.stdout = run
                    .stdout
                    .replace(r#""open":true"#, r#""open":<OPEN>"#)
                    .replace(r#""open":false"#, r#""open":<OPEN>"#);
                run
            };
            let local_run = open(run(local_cmd, &args, &stores));
            let remote_run = open(run(remote_cmd, &args, &stores));
            assert_eq!(local_run, remote_run, "{args:?}");
        }
    }
}

#[test]
fn auto_discovery() {
    let store = Store::new();
    store
        .cmd()
        .args(["create", "g", "abcabca", "bcab"])
        .assert()
        .success();
    let server = ServerProcess::start(store.path(), &["g"]);
    let info =
        discover(&GraphStore::new(store.path()), "g").expect("discovered");
    assert_eq!(info.url, server.url);
    let note = format!(
        "using server {} (graph held by pid {})",
        server.url, server.pid
    );
    store
        .cmd()
        .args(["search", "g", "bca", "--view", "occurrences"])
        .assert()
        .success()
        .stdout(predicate::str::contains("3 occurrences"))
        .stderr(predicate::str::contains(note.clone()));
    // A mutation goes to the server too (no `Locked`); -q hides the note.
    store
        .cmd()
        .args(["-q", "read", "g", "-t", "xyz"])
        .assert()
        .success()
        .stderr(predicate::str::is_empty());
    // --local bypasses the server and fails on its lock.
    store
        .cmd()
        .args(["--local", "read", "g", "-t", "xyz"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("locked"));
    server.stop();

    let location = GraphStore::new(store.path()).resolve("g").unwrap();
    assert!(!location.server_path().exists());
    // Stale discovery files: dead pid; live pid without a server.
    let closed_port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    for (url, pid) in [
        ("http://127.0.0.1:7878".to_string(), 999_999_999u32),
        (
            format!("http://127.0.0.1:{closed_port}"),
            std::process::id(),
        ),
    ] {
        fs::write(
            location.server_path(),
            format!(r#"{{"url":"{url}","pid":{pid},"token_required":false}}"#),
        )
        .unwrap();
        store
            .cmd()
            .args(["search", "g", "xyz"])
            .assert()
            .success()
            .stdout(predicate::str::contains("found"))
            .stderr(predicate::str::contains("using server").not());
    }
}

#[test]
fn client_files_are_local() {
    let store = Store::new();
    let server = ServerProcess::start(store.path(), &[]);
    let files = tempfile::tempdir().unwrap();
    let file = files.path().join("notes.txt");
    fs::write(&file, "the brown fox").unwrap();
    // The server refuses file inputs (no --allow-fs) ...
    let remote = RemoteBackend::new(server.url.clone(), None);
    let response = remote.execute(Request::new(Command::Read {
        graph: "g".into(),
        inputs: vec![Input::File {
            path: file.display().to_string(),
        }],
        split: Split::None,
        create: true,
        save: SavePolicy::Auto,
        steps: false,
        dry_run: false,
    }));
    match response.outcome {
        Outcome::Error(error) => assert_eq!(error.kind, ErrorKind::Forbidden),
        other => panic!("{other:?}"),
    }
    // ... but the CLI reads the file itself and sends its text.
    store
        .cmd()
        .args(["--server", &server.url, "read", "g", "--create", "-f"])
        .arg(&file)
        .assert()
        .success();
    let response = remote.execute(Request::new(Command::Inputs {
        graph: "g".into(),
        limits: Default::default(),
    }));
    match response.into_result().unwrap() {
        CommandResult::Inputs(list) => {
            assert_eq!(list.items.len(), 1);
            assert_eq!(list.items[0].label, file.display().to_string());
        },
        other => panic!("{other:?}"),
    }
}

const SCRIPT: &str = "\
# a comment, then an empty line

create s abcabca bcab
use s
search bca --view occurrences
read -t xyzxyz
:format json
info
:format text
vertex bca
search s-missing x
:bogus
inputs
";

#[test]
fn shell_script() {
    let local = Store::new();
    let remote = Store::new();
    let server = ServerProcess::start(remote.path(), &[]);
    let stores = [local.path(), remote.path()];
    let mut local_cmd = local.cmd();
    local_cmd.args(["--local", "shell"]).write_stdin(SCRIPT);
    let mut remote_cmd = remote.cmd();
    remote_cmd
        .args(["--server", &server.url, "shell"])
        .write_stdin(SCRIPT);
    let local_run = run(local_cmd, &[], &stores);
    let remote_run = run(remote_cmd, &[], &stores);
    assert_eq!(local_run, remote_run);
    // The last failing command sets the exit code (unknown setting: usage).
    assert_eq!(local_run.code, 2, "{local_run:?}");
    let stdout = &local_run.stdout;
    assert!(stdout.starts_with("created s from 2 inputs"), "{stdout}");
    assert!(stdout.contains("3 occurrences in 2 inputs"), "{stdout}");
    assert!(stdout.contains("read 1 input into s"), "{stdout}");
    assert!(stdout.contains(r#"{"ok":{"graph":{"name":"s""#), "{stdout}");
    assert!(stdout.contains("[2] <text>"), "{stdout}");
    assert!(local_run.stderr.contains("graph 's-missing' not found"));
    assert!(
        local_run
            .stderr
            .contains("unknown or incomplete shell command ':bogus'")
    );
    // The local shell saved and released the graph on exit.
    match local.ok(&["inputs", "s"]) {
        CommandResult::Inputs(list) => assert_eq!(list.items.len(), 3),
        other => panic!("{other:?}"),
    }
}

#[test]
fn shell_events() {
    let store = Store::new();
    store
        .cmd()
        .args(["create", "g", "abcabca"])
        .assert()
        .success();
    let server = ServerProcess::start(store.path(), &["g"]);

    // Client API: a read by one client reaches another's subscription.
    let reader = RemoteBackend::new(server.url.clone(), None);
    let mut events = RemoteBackend::new(server.url.clone(), None)
        .subscribe("g", &[Topic::GraphChanged])
        .unwrap();
    store
        .cmd()
        .args(["--server", &server.url, "read", "g", "-t", "bcab"])
        .assert()
        .success();
    match events.recv_timeout(Some(Duration::from_secs(10))).unwrap() {
        Some(Event::GraphChanged {
            graph, new_inputs, ..
        }) => {
            assert_eq!(graph, "g");
            assert_eq!(new_inputs, [1]);
        },
        other => panic!("{other:?}"),
    }
    drop(reader);

    // Shell: events of other clients are printed, its own are not.
    let mut shell =
        std::process::Command::new(env!("CARGO_BIN_EXE_context-cli"))
            .env_remove("CONTEXT_SERVER")
            .env_remove("CONTEXT_TOKEN")
            .args(["--store"])
            .arg(store.path())
            .args(["--server", &server.url, "shell", "g"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
    let mut stdin = shell.stdin.take().unwrap();
    let (tx, lines) = mpsc::channel::<String>();
    let stdout = shell.stdout.take().unwrap();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { return };
            if tx.send(line).is_err() {
                return;
            }
        }
    });
    let wait_for = |prefix: &str| -> String {
        loop {
            let line = lines
                .recv_timeout(Duration::from_secs(20))
                .unwrap_or_else(|_| panic!("no line starting with {prefix:?}"));
            if line.starts_with(prefix) {
                return line;
            }
        }
    };
    writeln!(stdin, ":events on").unwrap();
    writeln!(stdin, "read -t mine").unwrap();
    wait_for("read 1 input into g");
    store
        .cmd()
        .args(["--server", &server.url, "read", "g", "-t", "theirs"])
        .assert()
        .success();
    let event = wait_for("[event] graph_changed g");
    assert!(event.ends_with("new inputs [3]"), "{event}");
    drop(stdin);
    assert!(shell.wait().unwrap().success());
}

#[test]
fn token() {
    let store = Store::new();
    store.cmd().args(["create", "g", "abc"]).assert().success();
    let server =
        ServerProcess::start(store.path(), &["g", "--token", "secret"]);
    for token in [Some("wrong"), None] {
        let mut cmd = store.cmd();
        cmd.args(["--server", &server.url]);
        if let Some(token) = token {
            cmd.args(["--token", token]);
        }
        cmd.args(["info", "g"])
            .assert()
            .code(1)
            .stderr(predicate::str::contains("missing or invalid bearer token"))
            .stderr(predicate::str::contains("--token"));
    }
    // Auto-discovery with a token from the environment.
    store
        .cmd()
        .env("CONTEXT_TOKEN", "secret")
        .args(["info", "g"])
        .assert()
        .success()
        .stdout(predicate::str::contains("1 inputs"));
    // The WebSocket requires it too.
    let err = RemoteBackend::new(server.url.clone(), Some("wrong".into()))
        .subscribe("g", &[])
        .err()
        .expect("refused");
    assert_eq!(err.kind, ErrorKind::Forbidden);
    assert!(
        RemoteBackend::new(server.url.clone(), Some("secret".into()))
            .subscribe("g", &[])
            .is_ok()
    );
}
