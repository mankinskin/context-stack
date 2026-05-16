use std::collections::HashSet;

use crate::types::TokenRef;

use super::tmp_manager;
use super::super::{
    execute,
    Command,
    CommandResult,
};

#[test]
fn execute_error_propagation() {
    let (_tmp, mut mgr) = tmp_manager();

    let result = execute(
        &mut mgr,
        Command::AddAtom {
            workspace: "nope".into(),
            ch: 'x',
        },
    );
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.kind(), "atom");
}

#[test]
fn execute_list_workspaces_empty() {
    let (_tmp, mut mgr) = tmp_manager();
    let result = execute(&mut mgr, Command::ListWorkspaces).unwrap();
    match result {
        CommandResult::WorkspaceInfoList { workspaces } => {
            assert!(workspaces.is_empty());
        },
        other => panic!("expected WorkspaceInfoList, got: {other:?}"),
    }
}

#[test]
fn execute_get_atom() {
    let (_tmp, mut mgr) = tmp_manager();
    execute(&mut mgr, Command::CreateWorkspace { name: "ws".into() })
        .unwrap();

    let result = execute(
        &mut mgr,
        Command::GetAtom {
            workspace: "ws".into(),
            ch: 'z',
        },
    )
    .unwrap();
    match &result {
        CommandResult::OptionalAtomInfo { atom } => {
            assert!(atom.is_none());
        },
        other => panic!("expected OptionalAtomInfo, got: {other:?}"),
    }

    execute(
        &mut mgr,
        Command::AddAtom {
            workspace: "ws".into(),
            ch: 'z',
        },
    )
    .unwrap();

    let result = execute(
        &mut mgr,
        Command::GetAtom {
            workspace: "ws".into(),
            ch: 'z',
        },
    )
    .unwrap();
    match &result {
        CommandResult::OptionalAtomInfo { atom } => {
            assert!(atom.is_some());
            assert_eq!(atom.as_ref().unwrap().ch, 'z');
        },
        other => panic!("expected OptionalAtomInfo, got: {other:?}"),
    }
}

#[test]
fn execute_get_vertex() {
    let (_tmp, mut mgr) = tmp_manager();
    execute(&mut mgr, Command::CreateWorkspace { name: "ws".into() })
        .unwrap();
    execute(
        &mut mgr,
        Command::AddAtom {
            workspace: "ws".into(),
            ch: 'a',
        },
    )
    .unwrap();

    let result = execute(
        &mut mgr,
        Command::GetVertex {
            workspace: "ws".into(),
            index: 0,
        },
    )
    .unwrap();
    match result {
        CommandResult::OptionalVertexInfo { vertex } => {
            assert!(vertex.is_some());
        },
        other => panic!("expected OptionalVertexInfo, got: {other:?}"),
    }
}

#[test]
fn execute_delete_workspace() {
    let (_tmp, mut mgr) = tmp_manager();
    execute(&mut mgr, Command::CreateWorkspace { name: "ws".into() })
        .unwrap();
    let result =
        execute(&mut mgr, Command::DeleteWorkspace { name: "ws".into() })
            .unwrap();
    assert!(matches!(result, CommandResult::Ok));
}

#[test]
fn execute_snapshot() {
    let (_tmp, mut mgr) = tmp_manager();
    execute(&mut mgr, Command::CreateWorkspace { name: "ws".into() })
        .unwrap();
    execute(
        &mut mgr,
        Command::AddAtom {
            workspace: "ws".into(),
            ch: 'a',
        },
    )
    .unwrap();

    let result = execute(
        &mut mgr,
        Command::GetSnapshot {
            workspace: "ws".into(),
        },
    )
    .unwrap();
    match result {
        CommandResult::Snapshot(snapshot) => {
            assert_eq!(snapshot.nodes.len(), 1);
        },
        other => panic!("expected Snapshot, got: {other:?}"),
    }
}

#[test]
fn execute_insert_first_match_via_command() {
    let (_tmp, mut mgr) = tmp_manager();
    execute(&mut mgr, Command::CreateWorkspace { name: "ws".into() })
        .unwrap();

    let chars: Vec<char> = vec!['a', 'b'];
    execute(
        &mut mgr,
        Command::AddAtoms {
            workspace: "ws".into(),
            chars,
        },
    )
    .unwrap();

    let a_result = execute(
        &mut mgr,
        Command::GetAtom {
            workspace: "ws".into(),
            ch: 'a',
        },
    )
    .unwrap();
    let a_index = match &a_result {
        CommandResult::OptionalAtomInfo { atom: Some(a) } => a.index,
        other => panic!("expected atom a, got: {other:?}"),
    };

    let b_result = execute(
        &mut mgr,
        Command::GetAtom {
            workspace: "ws".into(),
            ch: 'b',
        },
    )
    .unwrap();
    let b_index = match &b_result {
        CommandResult::OptionalAtomInfo { atom: Some(b) } => b.index,
        other => panic!("expected atom b, got: {other:?}"),
    };

    let result = execute(
        &mut mgr,
        Command::InsertFirstMatch {
            workspace: "ws".into(),
            query: vec![TokenRef::Index(a_index), TokenRef::Index(b_index)],
        },
    )
    .unwrap();
    match &result {
        CommandResult::InsertResult(ir) => {
            assert!(!ir.already_existed);
            assert_eq!(ir.token.label, "ab");
        },
        other => panic!("expected InsertResult, got: {other:?}"),
    }
}

#[test]
fn execute_insert_sequences_via_command() {
    let (_tmp, mut mgr) = tmp_manager();
    execute(&mut mgr, Command::CreateWorkspace { name: "ws".into() })
        .unwrap();

    let texts: HashSet<String> =
        ["abc", "def"].iter().map(|s| s.to_string()).collect();

    let result = execute(
        &mut mgr,
        Command::InsertSequences {
            workspace: "ws".into(),
            texts,
        },
    )
    .unwrap();
    match &result {
        CommandResult::InsertResultList { results } => {
            assert_eq!(results.len(), 2);
        },
        other => panic!("expected InsertResultList, got: {other:?}"),
    }
}