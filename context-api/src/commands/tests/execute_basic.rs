use super::tmp_manager;
use super::super::{
    execute,
    Command,
    CommandResult,
};

#[test]
fn execute_create_workspace() {
    let (_tmp, mut mgr) = tmp_manager();

    let result = execute(
        &mut mgr,
        Command::CreateWorkspace {
            name: "test".into(),
        },
    )
    .unwrap();

    match result {
        CommandResult::WorkspaceInfo(info) => {
            assert_eq!(info.name, "test");
            assert_eq!(info.vertex_count, 0);
        },
        other => panic!("expected WorkspaceInfo, got: {other:?}"),
    }
}

#[test]
fn execute_add_atom() {
    let (_tmp, mut mgr) = tmp_manager();
    execute(&mut mgr, Command::CreateWorkspace { name: "ws".into() })
        .unwrap();

    let result = execute(
        &mut mgr,
        Command::AddAtom {
            workspace: "ws".into(),
            ch: 'a',
        },
    )
    .unwrap();

    match result {
        CommandResult::AtomInfo(info) => assert_eq!(info.ch, 'a'),
        other => panic!("expected AtomInfo, got: {other:?}"),
    }
}

#[test]
fn execute_full_workflow() {
    let (_tmp, mut mgr) = tmp_manager();

    execute(&mut mgr, Command::CreateWorkspace { name: "ws".into() })
        .unwrap();

    let chars: Vec<char> = vec!['a', 'b', 'c'];
    execute(
        &mut mgr,
        Command::AddAtoms {
            workspace: "ws".into(),
            chars,
        },
    )
    .unwrap();

    execute(
        &mut mgr,
        Command::AddSimplePattern {
            workspace: "ws".into(),
            atoms: vec!['a', 'b'],
        },
    )
    .unwrap();

    let search = execute(
        &mut mgr,
        Command::SearchSequence {
            workspace: "ws".into(),
            text: "ab".into(),
        },
    )
    .unwrap();
    match &search {
        CommandResult::SearchResult(sr) => {
            assert!(sr.complete, "should find 'ab'");
        },
        other => panic!("expected SearchResult, got: {other:?}"),
    }

    let vertices = execute(
        &mut mgr,
        Command::ListVertices {
            workspace: "ws".into(),
        },
    )
    .unwrap();
    match &vertices {
        CommandResult::TokenInfoList { tokens } => {
            assert_eq!(tokens.len(), 4);
        },
        other => panic!("expected TokenInfoList, got: {other:?}"),
    }

    let snap = execute(
        &mut mgr,
        Command::GetSnapshot {
            workspace: "ws".into(),
        },
    )
    .unwrap();
    assert!(matches!(snap, CommandResult::Snapshot(_)));

    let stats = execute(
        &mut mgr,
        Command::GetStatistics {
            workspace: "ws".into(),
        },
    )
    .unwrap();
    match &stats {
        CommandResult::Statistics(s) => {
            assert_eq!(s.vertex_count, 4);
            assert_eq!(s.atom_count, 3);
            assert_eq!(s.pattern_count, 1);
        },
        other => panic!("expected Statistics, got: {other:?}"),
    }

    let validation = execute(
        &mut mgr,
        Command::ValidateGraph {
            workspace: "ws".into(),
        },
    )
    .unwrap();
    match &validation {
        CommandResult::ValidationReport(vr) => {
            assert!(vr.valid);
        },
        other => panic!("expected ValidationReport, got: {other:?}"),
    }

    execute(&mut mgr, Command::SaveWorkspace { name: "ws".into() })
        .unwrap();
    execute(&mut mgr, Command::CloseWorkspace { name: "ws".into() })
        .unwrap();
}

#[test]
fn execute_insert_and_read_workflow() {
    let (_tmp, mut mgr) = tmp_manager();

    execute(&mut mgr, Command::CreateWorkspace { name: "ws".into() })
        .unwrap();

    let insert = execute(
        &mut mgr,
        Command::InsertSequence {
            workspace: "ws".into(),
            text: "world".into(),
        },
    )
    .unwrap();
    let insert_index = match &insert {
        CommandResult::InsertResult(ir) => {
            assert!(!ir.already_existed);
            assert_eq!(ir.token.width, 5);
            ir.token.index
        },
        other => panic!("expected InsertResult, got: {other:?}"),
    };

    let text = execute(
        &mut mgr,
        Command::ReadAsText {
            workspace: "ws".into(),
            index: insert_index,
        },
    )
    .unwrap();
    match &text {
        CommandResult::Text { text } => {
            assert_eq!(text, "world");
        },
        other => panic!("expected Text, got: {other:?}"),
    }

    let read = execute(
        &mut mgr,
        Command::ReadPattern {
            workspace: "ws".into(),
            index: insert_index,
        },
    )
    .unwrap();
    match &read {
        CommandResult::ReadResult(rr) => {
            assert_eq!(rr.text, "world");
            assert_eq!(rr.root.width, 5);
        },
        other => panic!("expected ReadResult, got: {other:?}"),
    }

    let search = execute(
        &mut mgr,
        Command::SearchSequence {
            workspace: "ws".into(),
            text: "world".into(),
        },
    )
    .unwrap();
    match &search {
        CommandResult::SearchResult(sr) => {
            assert!(sr.complete, "should find inserted 'world'");
            assert_eq!(sr.token.as_ref().unwrap().index, insert_index);
        },
        other => panic!("expected SearchResult, got: {other:?}"),
    }
}