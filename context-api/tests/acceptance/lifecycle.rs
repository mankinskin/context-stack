//! Lifecycle commands, read semantics, file inputs, locking and legacy
//! workspaces (plan 1.3, 1.4, 1.8 test 6).

use context_api::{
    types::*,
    Command,
    CommandResult,
    ErrorKind,
};

use crate::common::*;

fn labeled(
    text: &str,
    label: &str,
) -> Input {
    Input::Text {
        text: text.into(),
        label: Some(label.into()),
    }
}

fn new(overwrite: bool) -> Command {
    Command::New {
        graph: "g".into(),
        overwrite,
    }
}

fn read_command(
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

#[test]
fn new_list_info_delete() {
    let env = Env::new();
    let s = &env.session;
    assert!(list(s).is_empty());

    match exec(s, new(false)) {
        CommandResult::Graph(summary) => {
            assert_eq!(summary.name, "g");
            assert_eq!((summary.vertex_count, summary.input_count), (0, 0));
        },
        other => panic!("new: {other:?}"),
    }
    let names: Vec<String> = list(s).into_iter().map(|g| g.name).collect();
    assert_eq!(names, ["g"]);
    let summary = info(s, "g");
    assert_eq!(
        (summary.vertex_count, summary.atom_count, summary.composite_count),
        (0, 0, 0)
    );
    assert_eq!((summary.input_count, summary.total_input_chars), (0, 0));

    match exec(s, Command::Delete { graph: "g".into() }) {
        CommandResult::Deleted { graph } => assert_eq!(graph, "g"),
        other => panic!("delete: {other:?}"),
    }
    assert!(list(s).is_empty());
    assert_eq!(exec_err(s, Command::Info { graph: "g".into() }).kind, ErrorKind::NotFound);
    assert_eq!(exec_err(s, Command::Delete { graph: "g".into() }).kind, ErrorKind::NotFound);
}

#[test]
fn new_on_an_existing_graph_needs_overwrite() {
    let env = Env::new();
    let s = &env.session;
    exec(s, new(false));
    read(s, "g", vec![labeled("abab", "t")], false);
    assert_eq!(exec_err(s, new(false)).kind, ErrorKind::AlreadyExists);
    assert_eq!(info(s, "g").input_count, 1, "a refused new keeps the graph");

    exec(s, new(true));
    let summary = info(s, "g");
    assert_eq!((summary.vertex_count, summary.input_count), (0, 0));
}

#[test]
fn create_on_an_existing_graph_needs_overwrite() {
    let env = Env::new();
    let s = &env.session;
    create(s, "g", &strings(&["abc"]));
    let again = Command::Create {
        graph: "g".into(),
        inputs: text_inputs(&strings(&["xy"])),
        split: Split::None,
        overwrite: false,
    };
    assert_eq!(exec_err(s, again).kind, ErrorKind::AlreadyExists);
    exec(
        s,
        Command::Create {
            graph: "g".into(),
            inputs: text_inputs(&strings(&["xy"])),
            split: Split::None,
            overwrite: true,
        },
    );
    let summary = info(s, "g");
    assert_eq!((summary.input_count, summary.total_input_chars), (1, 2));
}

#[test]
fn read_on_a_missing_graph_needs_create() {
    let env = Env::new();
    let s = &env.session;
    let error = exec_err(s, read_command("g", vec![labeled("ab", "t")]));
    assert_eq!(error.kind, ErrorKind::NotFound);
    assert!(list(s).is_empty(), "a failed read creates nothing");

    let report = read(s, "g", vec![labeled("ab", "t")], true);
    assert_eq!(report.inputs.len(), 1);
    assert_eq!(info(s, "g").input_count, 1);
}

#[test]
fn reads_append_inputs_with_continuing_ids() {
    let env = Env::new();
    let s = &env.session;
    let first = read(s, "g", vec![labeled("abab", "one")], true);
    assert_eq!(first.vertex_count_before, 0);
    let second = read(s, "g", vec![labeled("ba", "two"), labeled("abc", "three")], false);
    assert_eq!(second.vertex_count_before, first.vertex_count_after);

    let ids: Vec<u32> = first.inputs.iter().chain(&second.inputs).map(|i| i.id).collect();
    assert!(ids.windows(2).all(|w| w[0] < w[1]), "ids {ids:?} must continue");

    let log = inputs(s, "g");
    let logged: Vec<(u32, &str, usize)> =
        log.iter().map(|r| (r.id, r.label.as_str(), r.width)).collect();
    assert_eq!(logged, [(ids[0], "one", 4), (ids[1], "two", 2), (ids[2], "three", 3)]);
    for (record, report) in log.iter().zip(first.inputs.iter().chain(&second.inputs)) {
        assert_eq!(record.token, report.token.index);
        assert_eq!(report.chars, record.width);
        assert_eq!(report.label, record.label);
    }
    let texts: Vec<String> = log.iter().map(|r| label(s, "g", r.token)).collect();
    assert_eq!(texts, ["abab", "ba", "abc"]);
    let summary = info(s, "g");
    assert_eq!((summary.input_count, summary.total_input_chars), (3, 9));
    assert_eq!(summary.vertex_count, second.vertex_count_after);
}

#[test]
fn empty_inputs_are_skipped() {
    let env = Env::new();
    let report = read(
        &env.session,
        "g",
        vec![labeled("", "empty"), labeled("ab", "full")],
        true,
    );
    assert_eq!(report.skipped, ["empty"]);
    assert_eq!(report.inputs.len(), 1);
    assert_eq!(report.inputs[0].label, "full");
    assert_eq!(info(&env.session, "g").input_count, 1);
}

#[test]
fn dry_run_leaves_the_graph_unchanged() {
    let env = Env::new();
    let s = &env.session;
    read(s, "g", vec![labeled("abab", "one")], true);
    let before = info(s, "g");
    let log = inputs(s, "g");
    let exported = export(s, "g");

    let report = match exec(
        s,
        Command::Read {
            graph: "g".into(),
            inputs: vec![labeled("abcabc", "two")],
            split: Split::None,
            create: false,
            save: SavePolicy::Auto,
            steps: false,
            dry_run: true,
        },
    ) {
        CommandResult::Read(report) => report,
        other => panic!("read: {other:?}"),
    };
    assert!(report.dry_run);
    assert_eq!(report.saved, None);
    assert_eq!(report.inputs.len(), 1);
    assert!(report.vertex_count_after > report.vertex_count_before);

    let after = info(s, "g");
    assert_eq!(
        (after.vertex_count, after.input_count, after.total_input_chars, after.generation),
        (before.vertex_count, before.input_count, before.total_input_chars, before.generation)
    );
    assert_eq!(inputs(s, "g"), log);
    assert_eq!(export(s, "g"), exported);
    let reopened = env.session(|_| {});
    assert_eq!(export(&reopened, "g"), exported, "nothing was saved");
}

#[test]
fn file_inputs_need_allow_fs() {
    let env = Env::new();
    let file = env.dir.path().join("in.txt");
    std::fs::write(&file, "abcab").unwrap();
    let input = Input::File {
        path: file.to_string_lossy().into(),
    };
    let mut command = read_command("g", vec![input]);
    if let Command::Read { create, .. } = &mut command {
        *create = true;
    }
    assert_eq!(exec_err(&env.session, command).kind, ErrorKind::Forbidden);
}

#[test]
fn file_inputs_below_allow_fs_are_read_and_others_refused() {
    let env = Env::new();
    let allowed = env.dir.path().join("files");
    std::fs::create_dir(&allowed).unwrap();
    let inside = allowed.join("in.txt");
    std::fs::write(&inside, "abcab").unwrap();
    let outside = env.dir.path().join("out.txt");
    std::fs::write(&outside, "xyz").unwrap();
    let session = env.session(|config| config.allow_fs = Some(allowed.clone()));

    let path = inside.to_string_lossy().to_string();
    let report = read(&session, "g", vec![Input::File { path: path.clone() }], true);
    assert_eq!(report.inputs.len(), 1);
    assert_eq!(report.inputs[0].label, path);
    assert_eq!(report.inputs[0].chars, 5);
    let log = inputs(&session, "g");
    assert_eq!(log[0].label, path);
    assert_eq!(label(&session, "g", log[0].token), "abcab");

    for refused in [
        outside.to_string_lossy().to_string(),
        allowed.join("../out.txt").to_string_lossy().to_string(),
    ] {
        let error = exec_err(&session, read_command("g", vec![Input::File { path: refused.clone() }]));
        assert_eq!(error.kind, ErrorKind::Forbidden, "{refused}: {error:?}");
    }
    assert_eq!(info(&session, "g").input_count, 1, "refused reads change nothing");
}

fn split_texts(
    split: Split,
    text: &str,
) -> Vec<String> {
    let env = Env::new();
    let s = &env.session;
    exec(
        s,
        Command::Read {
            graph: "g".into(),
            inputs: vec![labeled(text, "src")],
            split,
            create: true,
            save: SavePolicy::Auto,
            steps: false,
            dry_run: false,
        },
    );
    inputs(s, "g").into_iter().map(|r| label(s, "g", r.token)).collect()
}

#[test]
fn split_lines_reads_one_input_per_non_empty_line() {
    assert_eq!(split_texts(Split::Lines, "ab\n\nba\nabc\n"), ["ab", "ba", "abc"]);
}

#[test]
fn split_paragraphs_reads_one_input_per_blank_line_separated_block() {
    assert_eq!(
        split_texts(Split::Paragraphs, "ab\ncd\n\nef\n\n\ngh"),
        ["ab\ncd", "ef", "gh"]
    );
}

#[test]
fn split_none_reads_the_whole_source() {
    assert_eq!(split_texts(Split::None, "ab\n\nba"), ["ab\n\nba"]);
}

#[test]
fn an_open_graph_is_locked_for_other_sessions() {
    let env = Env::with(|config| config.keep_open = true);
    read(&env.session, "g", vec![labeled("abab", "a")], true);

    let other = env.session(|_| {});
    let error = exec_err(&other, read_command("g", vec![labeled("ba", "b")]));
    assert_eq!(error.kind, ErrorKind::Locked, "{error:?}");

    exec(
        &env.session,
        Command::Close {
            graph: "g".into(),
            save: true,
        },
    );
    read(&other, "g", vec![labeled("ba", "b")], false);
    assert_eq!(info(&other, "g").input_count, 2);
}

#[test]
fn legacy_workspaces_are_rejected_with_a_hint() {
    let env = Env::new();
    let legacy = env.dir.path().join("old");
    std::fs::create_dir(&legacy).unwrap();
    std::fs::write(legacy.join("metadata.json"), "{}").unwrap();
    std::fs::write(legacy.join("graph.bin"), [0u8; 4]).unwrap();

    for command in [
        Command::Info { graph: "old".into() },
        read_command("old", vec![labeled("ab", "t")]),
    ] {
        let error = exec_err(&env.session, command);
        assert_eq!(error.kind, ErrorKind::InvalidInput, "{error:?}");
        assert!(error.message.contains("legacy"), "{error:?}");
    }
}
