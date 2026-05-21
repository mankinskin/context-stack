use std::collections::HashSet;

use crate::types::TokenRef;

use super::super::{
    Command,
    export_import,
};

#[test]
fn command_serde_create_workspace() {
    let cmd = Command::CreateWorkspace {
        name: "demo".into(),
    };
    let json = serde_json::to_string(&cmd).unwrap();
    assert!(json.contains("create_workspace"));
    assert!(json.contains("demo"));

    let deser: Command = serde_json::from_str(&json).unwrap();
    match deser {
        Command::CreateWorkspace { name } => assert_eq!(name, "demo"),
        other => panic!("expected CreateWorkspace, got: {other:?}"),
    }
}

#[test]
fn command_serde_add_atom() {
    let cmd = Command::AddAtom {
        workspace: "ws".into(),
        ch: 'x',
    };
    let json = serde_json::to_string(&cmd).unwrap();
    assert!(json.contains("add_atom"));

    let deser: Command = serde_json::from_str(&json).unwrap();
    match deser {
        Command::AddAtom { workspace, ch } => {
            assert_eq!(workspace, "ws");
            assert_eq!(ch, 'x');
        },
        other => panic!("expected AddAtom, got: {other:?}"),
    }
}

#[test]
fn command_serde_add_atoms() {
    let chars: Vec<char> = vec!['a', 'b', 'c'];
    let cmd = Command::AddAtoms {
        workspace: "ws".into(),
        chars: chars.clone(),
    };
    let json = serde_json::to_string(&cmd).unwrap();
    assert!(json.contains("add_atoms"));

    let deser: Command = serde_json::from_str(&json).unwrap();
    match deser {
        Command::AddAtoms {
            workspace,
            chars: deser_chars,
        } => {
            assert_eq!(workspace, "ws");
            assert_eq!(deser_chars, chars);
        },
        other => panic!("expected AddAtoms, got: {other:?}"),
    }
}

#[test]
fn command_serde_add_simple_pattern() {
    let cmd = Command::AddSimplePattern {
        workspace: "ws".into(),
        atoms: vec!['a', 'b', 'c'],
    };
    let json = serde_json::to_string(&cmd).unwrap();
    assert!(json.contains("add_simple_pattern"));

    let deser: Command = serde_json::from_str(&json).unwrap();
    match deser {
        Command::AddSimplePattern { workspace, atoms } => {
            assert_eq!(workspace, "ws");
            assert_eq!(atoms, vec!['a', 'b', 'c']);
        },
        other => panic!("expected AddSimplePattern, got: {other:?}"),
    }
}

#[test]
fn command_serde_list_workspaces() {
    let cmd = Command::ListWorkspaces;
    let json = serde_json::to_string(&cmd).unwrap();
    let deser: Command = serde_json::from_str(&json).unwrap();
    assert!(matches!(deser, Command::ListWorkspaces));
}

#[test]
fn command_serde_search_sequence() {
    let cmd = Command::SearchSequence {
        workspace: "ws".into(),
        text: "hello".into(),
    };
    let json = serde_json::to_string(&cmd).unwrap();
    assert!(json.contains("search_sequence"));
    assert!(json.contains("hello"));

    let deser: Command = serde_json::from_str(&json).unwrap();
    match deser {
        Command::SearchSequence { workspace, text } => {
            assert_eq!(workspace, "ws");
            assert_eq!(text, "hello");
        },
        other => panic!("expected SearchSequence, got: {other:?}"),
    }
}

#[test]
fn command_serde_search_pattern() {
    let cmd = Command::SearchPattern {
        workspace: "ws".into(),
        query: vec![TokenRef::Index(0), TokenRef::Label("ab".into())],
    };
    let json = serde_json::to_string(&cmd).unwrap();
    assert!(json.contains("search_pattern"));

    let deser: Command = serde_json::from_str(&json).unwrap();
    match deser {
        Command::SearchPattern { workspace, query } => {
            assert_eq!(workspace, "ws");
            assert_eq!(query.len(), 2);
            assert_eq!(query[0], TokenRef::Index(0));
            assert_eq!(query[1], TokenRef::Label("ab".into()));
        },
        other => panic!("expected SearchPattern, got: {other:?}"),
    }
}

#[test]
fn command_serde_insert_sequence() {
    let cmd = Command::InsertSequence {
        workspace: "ws".into(),
        text: "world".into(),
    };
    let json = serde_json::to_string(&cmd).unwrap();
    assert!(json.contains("insert_sequence"));

    let deser: Command = serde_json::from_str(&json).unwrap();
    match deser {
        Command::InsertSequence { workspace, text } => {
            assert_eq!(workspace, "ws");
            assert_eq!(text, "world");
        },
        other => panic!("expected InsertSequence, got: {other:?}"),
    }
}

#[test]
fn command_serde_insert_sequences() {
    let texts: HashSet<String> =
        ["abc", "def"].iter().map(|s| s.to_string()).collect();
    let cmd = Command::InsertSequences {
        workspace: "ws".into(),
        texts: texts.clone(),
    };
    let json = serde_json::to_string(&cmd).unwrap();
    assert!(json.contains("insert_sequences"));

    let deser: Command = serde_json::from_str(&json).unwrap();
    match deser {
        Command::InsertSequences {
            workspace,
            texts: deser_texts,
        } => {
            assert_eq!(workspace, "ws");
            assert_eq!(deser_texts, texts);
        },
        other => panic!("expected InsertSequences, got: {other:?}"),
    }
}

#[test]
fn command_serde_read_pattern() {
    let cmd = Command::ReadPattern {
        workspace: "ws".into(),
        index: 42,
    };
    let json = serde_json::to_string(&cmd).unwrap();
    assert!(json.contains("read_pattern"));

    let deser: Command = serde_json::from_str(&json).unwrap();
    match deser {
        Command::ReadPattern { workspace, index } => {
            assert_eq!(workspace, "ws");
            assert_eq!(index, 42);
        },
        other => panic!("expected ReadPattern, got: {other:?}"),
    }
}

#[test]
fn command_serde_read_as_text() {
    let cmd = Command::ReadAsText {
        workspace: "ws".into(),
        index: 7,
    };
    let json = serde_json::to_string(&cmd).unwrap();
    assert!(json.contains("read_as_text"));

    let deser: Command = serde_json::from_str(&json).unwrap();
    match deser {
        Command::ReadAsText { workspace, index } => {
            assert_eq!(workspace, "ws");
            assert_eq!(index, 7);
        },
        other => panic!("expected ReadAsText, got: {other:?}"),
    }
}

#[test]
fn command_serde_validate_graph() {
    let cmd = Command::ValidateGraph {
        workspace: "ws".into(),
    };
    let json = serde_json::to_string(&cmd).unwrap();
    assert!(json.contains("validate_graph"));

    let deser: Command = serde_json::from_str(&json).unwrap();
    match deser {
        Command::ValidateGraph { workspace } => {
            assert_eq!(workspace, "ws");
        },
        other => panic!("expected ValidateGraph, got: {other:?}"),
    }
}

#[test]
fn command_serde_all_variants_have_tag() {
    let commands = vec![
        Command::CreateWorkspace { name: "a".into() },
        Command::OpenWorkspace { name: "a".into() },
        Command::CloseWorkspace { name: "a".into() },
        Command::SaveWorkspace { name: "a".into() },
        Command::ListWorkspaces,
        Command::DeleteWorkspace { name: "a".into() },
        Command::AddAtom {
            workspace: "a".into(),
            ch: 'x',
        },
        Command::AddAtoms {
            workspace: "a".into(),
            chars: Vec::new(),
        },
        Command::GetAtom {
            workspace: "a".into(),
            ch: 'x',
        },
        Command::ListAtoms {
            workspace: "a".into(),
        },
        Command::AddSimplePattern {
            workspace: "a".into(),
            atoms: vec!['a', 'b'],
        },
        Command::GetVertex {
            workspace: "a".into(),
            index: 0,
        },
        Command::ListVertices {
            workspace: "a".into(),
        },
        Command::SearchPattern {
            workspace: "a".into(),
            query: vec![TokenRef::Index(0), TokenRef::Index(1)],
        },
        Command::SearchSequence {
            workspace: "a".into(),
            text: "ab".into(),
        },
        Command::InsertFirstMatch {
            workspace: "a".into(),
            query: vec![TokenRef::Index(0), TokenRef::Index(1)],
        },
        Command::InsertSequence {
            workspace: "a".into(),
            text: "ab".into(),
        },
        Command::InsertSequences {
            workspace: "a".into(),
            texts: HashSet::new(),
        },
        Command::ReadPattern {
            workspace: "a".into(),
            index: 0,
        },
        Command::ReadAsText {
            workspace: "a".into(),
            index: 0,
        },
        Command::ReadSequence {
            workspace: "a".into(),
            text: "hello".into(),
        },
        Command::ReadFile {
            workspace: "a".into(),
            path: "/tmp/test.txt".into(),
        },
        Command::GetSnapshot {
            workspace: "a".into(),
        },
        Command::GetStatistics {
            workspace: "a".into(),
        },
        Command::ValidateGraph {
            workspace: "a".into(),
        },
        Command::ExportWorkspace {
            workspace: "a".into(),
            format: export_import::ExportFormat::Json,
            path: None,
        },
        Command::ExportWorkspace {
            workspace: "a".into(),
            format: export_import::ExportFormat::Bincode,
            path: Some("/tmp/test.bin".into()),
        },
        Command::ImportWorkspace {
            name: "a".into(),
            path: "/tmp/test.json".into(),
            overwrite: false,
        },
        Command::ImportWorkspace {
            name: "a".into(),
            path: "/tmp/test.bin".into(),
            overwrite: true,
        },
    ];

    for cmd in commands {
        let json = serde_json::to_string(&cmd)
            .unwrap_or_else(|e| panic!("failed to serialize {cmd:?}: {e}"));
        assert!(
            json.contains("\"command\""),
            "JSON should have a 'command' tag: {json}"
        );
        let _: Command = serde_json::from_str(&json)
            .unwrap_or_else(|e| panic!("failed to deserialize {json}: {e}"));
    }
}
