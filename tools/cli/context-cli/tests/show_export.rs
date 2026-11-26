//! `show` formats and gate, `export` / `import`.

mod common;

use std::{
    fs,
    process::Command as Process,
};

use common::Store;
use context_trace::positioned::{
    diff,
    interchange::fixtures::fixture_snapshots,
};
use predicates::prelude::*;

fn demo() -> Store {
    let store = Store::new();
    store
        .cmd()
        .args(["create", "demo", "abcabca", "bcab"])
        .assert()
        .success();
    store
}

fn stdout(
    store: &Store,
    args: &[&str],
) -> String {
    let output = store.cmd().args(args).output().unwrap();
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn show_formats() {
    let store = demo();
    let ascii = stdout(&store, &["show", "demo"]);
    assert!(!ascii.trim().is_empty());
    assert!(
        !stdout(&store, &["show", "demo", "--format", "ascii-dag"])
            .trim()
            .is_empty()
    );

    let dot = stdout(&store, &["show", "demo", "--format", "dot"]);
    assert!(dot.trim_start().starts_with("digraph"), "{dot}");
    assert_eq!(dot.matches('{').count(), dot.matches('}').count());
    assert!(dot.contains("->"));
    let dot_file = store.path().join("demo.dot");
    store
        .cmd()
        .args(["show", "demo", "-o", dot_file.to_str().unwrap()])
        .assert()
        .success();
    assert_eq!(
        fs::read_to_string(&dot_file).unwrap().trim_end(),
        dot.trim_end()
    );
    if Process::new("dot")
        .arg("-V")
        .output()
        .is_ok_and(|o| o.status.success())
    {
        let status = Process::new("dot")
            .args(["-Tsvg", "-o", "/dev/null"])
            .arg(&dot_file)
            .status()
            .unwrap();
        assert!(status.success(), "dot rejected the output");
    }

    let mermaid = stdout(&store, &["show", "demo", "--format", "mermaid"]);
    assert!(mermaid.starts_with("graph TD"), "{mermaid}");
    let md = store.path().join("demo.md");
    store
        .cmd()
        .args(["show", "demo", "-o", md.to_str().unwrap()])
        .assert()
        .success();
    let md = fs::read_to_string(md).unwrap();
    assert!(md.starts_with("```mermaid\ngraph TD"), "{md}");
    assert!(md.trim_end().ends_with("```"));

    let json = store.path().join("demo.json");
    store
        .cmd()
        .args(["show", "demo", "-o", json.to_str().unwrap()])
        .assert()
        .success();
    let graph: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(json).unwrap()).unwrap();
    assert!(graph["vertices"].is_array() && graph["edges"].is_array());
}

#[test]
fn show_gate() {
    let store = Store::new();
    let words: Vec<String> =
        (0..40).map(|i| format!("w{i}x{}y", i * 7 % 13)).collect();
    store
        .cmd()
        .arg("create")
        .arg("big")
        .args(&words)
        .assert()
        .success();
    store
        .cmd()
        .args(["show", "big", "--format", "dot"])
        .assert()
        .code(1)
        .stderr(
            predicate::str::contains("too large").and(
                predicate::str::contains("hint:")
                    .and(predicate::str::contains("--root")),
            ),
        );
    store
        .cmd()
        .args(["show", "big", "--root", &words[3], "--depth", "2"])
        .assert()
        .success();
    store
        .cmd()
        .args(["show", "big", "--format", "dot", "--force"])
        .assert()
        .success();
    store
        .cmd()
        .args(["show", "big", "--max-vertices", "100000"])
        .assert()
        .success();
}

#[test]
fn export_import_v3() {
    let store = demo();
    let v3 = store.path().join("demo.v3.json");
    store
        .cmd()
        .args([
            "export",
            "demo",
            "--format",
            "v3",
            "-o",
            v3.to_str().unwrap(),
        ])
        .assert()
        .success();
    let document: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&v3).unwrap()).unwrap();
    assert!(document.is_object());
    assert_eq!(
        stdout(&store, &["export", "demo"]).trim_end(),
        fs::read_to_string(&v3).unwrap().trim_end()
    );
    store
        .cmd()
        .args(["import", "copy", v3.to_str().unwrap()])
        .assert()
        .success();
    let d = diff(&store.graph_data("demo"), &store.graph_data("copy"));
    assert!(d.is_empty(), "{d}");
    store
        .cmd()
        .args(["import", "copy", v3.to_str().unwrap()])
        .assert()
        .code(1);
    store
        .cmd()
        .args(["import", "copy", v3.to_str().unwrap(), "--force"])
        .assert()
        .success();

    // Native round trip.
    let native = store.path().join("demo.bin");
    store
        .cmd()
        .args([
            "export",
            "demo",
            "--format",
            "native",
            "-o",
            native.to_str().unwrap(),
        ])
        .assert()
        .success();
    assert!(fs::read(&native).unwrap().starts_with(b"CTXG"));
    store
        .cmd()
        .args(["import", "native", native.to_str().unwrap()])
        .assert()
        .success();
    let d = diff(&store.graph_data("demo"), &store.graph_data("native"));
    assert!(d.is_empty(), "{d}");
    let envelope = store.json(&["export", "demo", "--format", "native"]);
    assert!(envelope.is_ok());

    // A reader fixture snapshot imports as the fixture graph.
    let (name, snapshot) =
        fixture_snapshots().into_iter().next().expect("fixtures");
    let file = store.path().join("fixture.json");
    fs::write(&file, serde_json::to_string(&snapshot).unwrap()).unwrap();
    store
        .cmd()
        .args(["import", "fixture", file.to_str().unwrap()])
        .assert()
        .success();
    let d = diff(&snapshot.graph, &store.graph_data("fixture"));
    assert!(d.is_empty(), "fixture {name}: {d}");
}
