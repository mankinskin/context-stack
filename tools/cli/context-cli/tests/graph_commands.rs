//! `new`, `create`, `read`, `inputs`, `info`, locks and exit codes.

mod common;

use std::fs;

use common::Store;
use context_api::{
    Command,
    CommandResult,
    Session,
    SessionConfig,
    store::{
        GraphStore,
        OpenMode,
    },
    types::{
        Input,
        SavePolicy,
        Split,
    },
};
use context_trace::positioned::diff;
use predicates::prelude::*;

fn input_labels(
    store: &Store,
    graph: &str,
) -> Vec<String> {
    match store.ok(&["inputs", graph]) {
        CommandResult::Inputs(list) =>
            list.items.into_iter().map(|i| i.label).collect(),
        other => panic!("{other:?}"),
    }
}

#[test]
fn new_then_info() {
    let store = Store::new();
    store
        .cmd()
        .args(["new", "g"])
        .assert()
        .success()
        .stdout(predicate::str::contains("created g"));
    assert!(store.graph_path("g").is_file());
    match store.ok(&["info", "g"]) {
        CommandResult::Graph(summary) => {
            assert_eq!(summary.vertex_count, 0);
            assert_eq!(summary.input_count, 0);
        },
        other => panic!("{other:?}"),
    }
    // The lock was released when the process exited.
    GraphStore::new(store.path())
        .open("g", OpenMode::Exclusive)
        .unwrap();
    // A second `new` refuses to overwrite; --force replaces.
    store
        .cmd()
        .args(["new", "g"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("already exists"));
    store.cmd().args(["new", "g", "--force"]).assert().success();
}

#[test]
fn create_from_args_files_stdin() {
    let store = Store::new();
    let file = store.path().join("notes.txt");
    fs::write(&file, "the brown fox\nthe brown dog").unwrap();
    let file_label = file.display().to_string();
    store
        .cmd()
        .args([
            "create",
            "g",
            "abcabca",
            "-f",
            file.to_str().unwrap(),
            "-",
            "-t",
            "-bcab",
        ])
        .write_stdin("brown bear")
        .assert()
        .success()
        .stdout(predicate::str::contains("created g from 4 inputs"));
    assert_eq!(
        input_labels(&store, "g"),
        ["<text>", file_label.as_str(), "<stdin>", "<text>"]
    );

    // Same graph as `Create` through the API.
    let other = tempfile::tempdir().unwrap();
    let session = Session::new(SessionConfig::new(other.path()));
    let text = |text: &str, label: Option<&str>| Input::Text {
        text: text.into(),
        label: label.map(str::to_string),
    };
    let response = common::execute(
        &session,
        Command::Create {
            graph: "g".into(),
            inputs: vec![
                text("abcabca", None),
                text("the brown fox\nthe brown dog", Some(&file_label)),
                text("brown bear", Some("<stdin>")),
                text("-bcab", None),
            ],
            split: Split::None,
            // A retry after `Locked` may find the empty graph of the failed attempt.
            overwrite: true,
        },
    );
    assert!(response.is_ok(), "{response:?}");
    let expected = common::graph_data(&GraphStore::new(other.path()), "g");
    let d = diff(&expected, &store.graph_data("g"));
    assert!(d.is_empty(), "{d}");

    // `create` on an existing graph needs --force.
    store
        .cmd()
        .args(["create", "g", "x"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("already exists"));
    store
        .cmd()
        .args(["create", "g", "x", "--force"])
        .assert()
        .success();
    assert_eq!(input_labels(&store, "g"), ["<text>"]);
}

#[test]
fn read_appends_and_saves_atomically() {
    let store = Store::new();
    store
        .cmd()
        .args(["create", "g", "abcabc"])
        .assert()
        .success();
    store
        .cmd()
        .args(["read", "g", "bcdbcd"])
        .assert()
        .success()
        .stdout(predicate::str::contains("read 1 input into g"));
    assert_eq!(input_labels(&store, "g").len(), 2);

    let before = fs::read(store.graph_path("g")).unwrap();
    let good = store.path().join("good.txt");
    let bad = store.path().join("bad.bin");
    fs::write(&good, "cdecde").unwrap();
    fs::write(&bad, [b'a', 0xff, 0xfe, b'b']).unwrap();
    store
        .cmd()
        .args([
            "read",
            "g",
            "-f",
            good.to_str().unwrap(),
            "-f",
            bad.to_str().unwrap(),
        ])
        .assert()
        .code(3)
        .stderr(
            predicate::str::contains("bad.bin")
                .and(predicate::str::contains("UTF-8")),
        );
    assert_eq!(
        fs::read(store.graph_path("g")).unwrap(),
        before,
        "graph file changed"
    );

    // --dry-run reports without saving.
    store
        .cmd()
        .args(["read", "g", "xyzxyz", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("dry run"));
    assert_eq!(fs::read(store.graph_path("g")).unwrap(), before);

    // --steps includes the reader steps.
    match store.ok(&["read", "g", "abcd", "--steps"]) {
        CommandResult::Read(report) =>
            assert!(report.inputs[0].steps.is_some()),
        other => panic!("{other:?}"),
    }
}

#[test]
fn read_missing_graph_hint() {
    let store = Store::new();
    store
        .cmd()
        .args(["read", "missing", "abc"])
        .assert()
        .code(1)
        .stderr(
            predicate::str::contains("not found")
                .and(predicate::str::contains("--create")),
        );
    assert!(!store.graph_path("missing").exists());
    store
        .cmd()
        .args(["read", "missing", "abc", "--create"])
        .assert()
        .success();
    assert!(store.graph_path("missing").is_file());
}

#[test]
fn split_lines() {
    let store = Store::new();
    store
        .cmd()
        .args(["create", "g", "--split", "lines"])
        .write_stdin("one line\n\nsecond line\nthird\n")
        .assert()
        .success();
    assert_eq!(
        input_labels(&store, "g"),
        ["<stdin>:1", "<stdin>:3", "<stdin>:4"]
    );
    store
        .cmd()
        .args(["read", "g", "-", "--split", "paragraphs"])
        .write_stdin("a b\nc d\n\ne f\n")
        .assert()
        .success();
    assert_eq!(input_labels(&store, "g").len(), 5);
}

#[test]
fn recursive_directory_input() {
    let store = Store::new();
    let notes = store.path().join("notes");
    fs::create_dir_all(notes.join("sub")).unwrap();
    fs::write(notes.join("b.md"), "bravo").unwrap();
    fs::write(notes.join("a.md"), "alpha").unwrap();
    fs::write(notes.join("skip.txt"), "skip").unwrap();
    fs::write(notes.join("sub/c.md"), "charlie").unwrap();
    let dir = notes.to_str().unwrap();
    store
        .cmd()
        .args(["create", "g", "-f", dir])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("--recursive"));
    store
        .cmd()
        .args(["create", "g", "-f", dir, "--recursive", "--glob", "*.md"])
        .assert()
        .success();
    let labels = input_labels(&store, "g");
    let names: Vec<_> = labels
        .iter()
        .map(|l| l.rsplit('/').next().unwrap().to_string())
        .collect();
    assert_eq!(names, ["a.md", "b.md", "c.md"]);
}

#[test]
fn vertex_by_index_and_text() {
    let store = Store::new();
    store
        .cmd()
        .args(["create", "g", "abcabca", "bcab"])
        .assert()
        .success();
    let CommandResult::Vertex(by_text) = store.ok(&["vertex", "g", "bca"])
    else {
        panic!()
    };
    assert_eq!(by_text.token.label, "bca");
    let index = format!("#{}", by_text.token.index);
    let CommandResult::Vertex(by_index) = store.ok(&["vertex", "g", &index])
    else {
        panic!()
    };
    assert_eq!(by_index, by_text);
    store
        .cmd()
        .args(["vertex", "g", "bca"])
        .assert()
        .success()
        .stdout(predicate::str::contains("parents"));
    store.cmd().args(["vertex", "g", "#9999"]).assert().code(1);
}

#[test]
fn list_delete_stats_validate() {
    let store = Store::new();
    store.cmd().args(["create", "a", "abab"]).assert().success();
    store.cmd().args(["create", "b", "cdcd"]).assert().success();
    let CommandResult::Graphs { graphs } = store.ok(&["list"]) else {
        panic!()
    };
    assert_eq!(graphs.len(), 2);
    store
        .cmd()
        .args(["stats", "a"])
        .assert()
        .success()
        .stdout(predicate::str::contains("width histogram"));
    store
        .cmd()
        .args(["validate", "a"])
        .assert()
        .success()
        .stdout("valid\n");
    store
        .cmd()
        .args(["delete", "a"])
        .assert()
        .success()
        .stdout("deleted a\n");
    assert!(!store.graph_path("a").exists());
    store.cmd().args(["info", "a"]).assert().code(1);
}

#[test]
fn graph_path_argument() {
    let store = Store::new();
    let path = store.path().join("elsewhere/my.ctx");
    let path = path.to_str().unwrap();
    store
        .cmd()
        .args(["create", path, "abab"])
        .assert()
        .success();
    store
        .cmd()
        .args(["info", path])
        .assert()
        .success()
        .stdout(predicate::str::contains("1 inputs"));
}

#[test]
fn locked_graph() {
    let store = Store::new();
    store.cmd().args(["create", "g", "abc"]).assert().success();
    let mut config = SessionConfig::new(store.path());
    config.keep_open = true;
    let holder = Session::new(config);
    let response = common::execute(
        &holder,
        Command::Read {
            graph: "g".into(),
            inputs: vec![Input::Text {
                text: "abd".into(),
                label: None,
            }],
            split: Split::None,
            create: false,
            save: SavePolicy::Defer,
            steps: false,
            dry_run: false,
        },
    );
    assert!(response.is_ok(), "{response:?}");
    store
        .cmd()
        .args(["read", "g", "xyz"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("locked"));
    let response = store.json(&["read", "g", "xyz"]);
    assert_eq!(
        response.into_result().unwrap_err().kind,
        context_api::ErrorKind::Locked
    );
    drop(holder);
    // Concurrently spawned test children may hold the inherited lock briefly.
    let released = (0..100).any(|_| {
        let ok = store
            .cmd()
            .args(["read", "g", "xyz"])
            .output()
            .unwrap()
            .status
            .success();
        if !ok {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        ok
    });
    assert!(released, "lock not released after the holder was dropped");
}

#[test]
fn legacy_workspace_rejected() {
    let store = Store::new();
    let legacy = store.path().join("old");
    fs::create_dir_all(&legacy).unwrap();
    fs::write(legacy.join("metadata.json"), "{}").unwrap();
    fs::write(legacy.join("graph.bin"), [0u8; 4]).unwrap();
    store
        .cmd()
        .args(["info", "old"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("legacy workspace"));
    store
        .cmd()
        .args(["read", "old", "abc"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("legacy"));
}

#[test]
fn json_output_is_the_response_envelope() {
    let store = Store::new();
    let response = store.json(&["create", "g", "abab"]);
    assert!(response.is_ok());
    // Errors are envelopes too; the format may also follow the command.
    let output = store
        .cmd()
        .args(["info", "nope", "--format", "json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let err = common::parse_response(&output.stdout)
        .into_result()
        .unwrap_err();
    assert_eq!(err.kind, context_api::ErrorKind::NotFound);
    // CLI-side errors as well (exit 3 for invalid UTF-8).
    let bad = store.path().join("bad.txt");
    fs::write(&bad, [0xffu8]).unwrap();
    let output = store
        .cmd()
        .args(["--format", "json", "read", "g", "-f", bad.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(3));
    let err = common::parse_response(&output.stdout)
        .into_result()
        .unwrap_err();
    assert_eq!(err.kind, context_api::ErrorKind::Io);
}

#[test]
fn corrupt_graph_exits_3() {
    let store = Store::new();
    fs::write(store.graph_path("g"), b"CTXGgarbage").unwrap();
    store
        .cmd()
        .args(["inputs", "g"])
        .assert()
        .code(3)
        .stderr(predicate::str::contains("corrupt"));
}

#[test]
fn usage_errors_exit_2() {
    let store = Store::new();
    store.cmd().assert().code(2);
    store.cmd().args(["bogus"]).assert().code(2);
    store.cmd().args(["new"]).assert().code(2);
    store.cmd().args(["search", "g"]).assert().code(2);
    store
        .cmd()
        .args(["info", "g", "--unknown"])
        .assert()
        .code(2);
    store
        .cmd()
        .args(["show", "g", "--format", "png"])
        .assert()
        .code(2);
    store
        .cmd()
        .args(["create", "g", "--split", "words", "x"])
        .assert()
        .code(2);
    store
        .cmd()
        .args(["create", "g", "--glob", "*.md", "x"])
        .assert()
        .code(2);
    store
        .cmd()
        .args(["export", "g", "--format", "native"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("-o FILE"));
    store
        .cmd()
        .args(["create", "g", "-", "-"])
        .write_stdin("x")
        .assert()
        .code(2);
}

#[test]
fn server_mode_not_available_yet() {
    let store = Store::new();
    store
        .cmd()
        .args(["--server", "http://127.0.0.1:1", "list"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("stage 4"));
    store
        .cmd()
        .env("CONTEXT_SERVER", "http://127.0.0.1:1")
        .args(["--local", "list"])
        .assert()
        .success();
}
