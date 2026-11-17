use ngrams::output::GraphSnapshot;
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    time::{SystemTime, UNIX_EPOCH},
};

struct Directory(PathBuf);

impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "ngrams-cli-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove CLI test directory");
    }
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ngrams"))
        .args(args)
        .output()
        .unwrap()
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn snapshot(bytes: &[u8], corpus: &[&str]) -> GraphSnapshot {
    let value: Value = serde_json::from_slice(bytes).expect("output must be JSON only");
    let object = value.as_object().unwrap();
    assert_eq!(
        object.keys().map(String::as_str).collect::<Vec<_>>(),
        ["corpus", "format_version", "vertices"]
    );
    let snapshot: GraphSnapshot = serde_json::from_value(value.clone()).unwrap();
    snapshot.validate().unwrap();
    assert_eq!(snapshot.corpus, corpus);
    let mut unknown = value;
    unknown["unexpected"] = json!(true);
    assert!(serde_json::from_value::<GraphSnapshot>(unknown).is_err());
    snapshot
}

#[test]
fn inline_inputs_emit_one_complete_graph_to_stdout() {
    let output = run(&["abc", "xyz"]);
    assert_success(&output);
    let snapshot = snapshot(&output.stdout, &["abc", "xyz"]);
    assert_eq!(snapshot.graph.vertices.len(), 8);
    assert!(snapshot.graph.vertices["abc"].contains(&vec!["a".into(), "b".into(), "c".into()]));
    assert!(snapshot.graph.vertices["xyz"].contains(&vec!["x".into(), "y".into(), "z".into()]));
}

#[test]
fn output_preserves_alternative_decompositions() {
    let output = run(&["ab", "bc", "abc"]);
    assert_success(&output);
    let snapshot = snapshot(&output.stdout, &["ab", "bc", "abc"]);
    assert_eq!(
        snapshot.graph.vertices["abc"],
        [vec!["a".into(), "bc".into()], vec!["ab".into(), "c".into()]].into()
    );
}

#[test]
fn file_accepts_single_and_multiple_scenarios_and_creates_output_directory() {
    let directory = Directory::new();
    for (index, scenarios) in [
        json!({"name": "single", "inputs": ["abc"]}),
        json!([
            {"name": "first", "inputs": ["ab"]},
            {"name": "second", "inputs": ["xy", "xyz"]}
        ]),
    ]
    .into_iter()
    .enumerate()
    {
        let file = directory.path().join(format!("scenarios-{index}.json"));
        let out = directory.path().join(format!("nested-{index}/graphs"));
        fs::write(&file, serde_json::to_vec(&scenarios).unwrap()).unwrap();
        let output = run(&[
            "--file",
            file.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
        ]);
        assert_success(&output);
        assert!(output.stdout.is_empty(), "file mode must not print graphs");
        let scenarios = match scenarios {
            Value::Array(values) => values,
            other => vec![other],
        };
        assert_eq!(fs::read_dir(&out).unwrap().count(), scenarios.len());
        for scenario in scenarios {
            let name = scenario["name"].as_str().unwrap();
            let inputs: Vec<_> = scenario["inputs"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_str().unwrap())
                .collect();
            snapshot(
                &fs::read(out.join(format!("{name}.json"))).unwrap(),
                &inputs,
            );
        }
    }
}

#[test]
fn invalid_cli_combinations_fail_before_generation() {
    for args in [
        vec![],
        vec!["--file", "missing.json"],
        vec!["--out", "graphs"],
        vec!["abc", "--file", "missing.json", "--out", "graphs"],
        vec!["--unknown"],
        vec![""],
    ] {
        let output = run(&args);
        assert!(!output.status.success(), "accepted invalid args: {args:?}");
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
    let help = run(&["--help"]);
    assert_success(&help);
    assert!(String::from_utf8_lossy(&help.stdout).contains("--file"));
}

#[test]
fn invalid_scenario_files_fail_without_writing_graphs() {
    let directory = Directory::new();
    let file = directory.path().join("scenarios.json");
    let out = directory.path().join("graphs");
    for contents in [
        "not json".into(),
        json!([]).to_string(),
        json!({"name": "../escape", "inputs": ["abc"]}).to_string(),
        json!({"name": "empty", "inputs": []}).to_string(),
        json!({"name": "empty_text", "inputs": [""]}).to_string(),
        json!({"name": "wrong_shape", "inputs": "abc"}).to_string(),
        json!([
            {"name": "duplicate", "inputs": ["ab"]},
            {"name": "duplicate", "inputs": ["bc"]}
        ])
        .to_string(),
    ] {
        fs::write(&file, contents).unwrap();
        let output = run(&[
            "--file",
            file.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
        ]);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
        assert!(
            !out.exists(),
            "invalid batch must be validated before writing"
        );
    }
    fs::write(&file, r#"{"name":"valid","inputs":["ab"]}"#).unwrap();
    fs::write(&out, "not a directory").unwrap();
    let output = run(&[
        "--file",
        file.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    assert!(!output.status.success());
    assert!(!output.stderr.is_empty());
}
