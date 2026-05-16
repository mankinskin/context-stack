use super::*;

#[tokio::test]
async fn test_create_workspace() {
    let (_tmp, server) = tmp_server();

    let output = exec(
        &server,
        Command::CreateWorkspace {
            name: "test".into(),
        },
    )
    .await;

    assert!(output.success, "create_workspace should succeed");
    assert!(output.result.is_some());
    assert!(output.error.is_none());
}

#[tokio::test]
async fn test_error_workspace_not_found() {
    let (_tmp, server) = tmp_server();

    let output = exec(
        &server,
        Command::OpenWorkspace {
            name: "nope".into(),
        },
    )
    .await;

    assert!(!output.success, "open nonexistent should fail");
    assert!(output.result.is_none());
    assert!(output.error.is_some());
    assert!(
        output.error.as_ref().unwrap().contains("not found"),
        "error should mention 'not found': {:?}",
        output.error
    );
}

#[tokio::test]
async fn test_add_atoms_and_list() {
    let (_tmp, server) = tmp_server();

    exec(&server, Command::CreateWorkspace { name: "ws".into() }).await;

    let output = exec(
        &server,
        Command::AddAtoms {
            workspace: "ws".into(),
            chars: vec!['a', 'b', 'c'],
        },
    )
    .await;
    assert!(output.success);

    let output = exec(
        &server,
        Command::ListAtoms {
            workspace: "ws".into(),
        },
    )
    .await;
    assert!(output.success);

    match &output.result {
        Some(CommandResult::AtomInfoList { atoms }) => {
            assert_eq!(atoms.len(), 3, "should have 3 atoms");
        },
        other => panic!("Expected AtomInfoList, got {other:?}"),
    }
}

#[tokio::test]
async fn test_insert_and_search_workflow() {
    let (_tmp, server) = tmp_server();

    exec(&server, Command::CreateWorkspace { name: "ws".into() }).await;
    exec(
        &server,
        Command::AddAtoms {
            workspace: "ws".into(),
            chars: vec!['a', 'b', 'c', 'd', 'e'],
        },
    )
    .await;

    let insert_out = exec(
        &server,
        Command::InsertSequence {
            workspace: "ws".into(),
            text: "abcde".into(),
        },
    )
    .await;
    assert!(insert_out.success);

    let search_out = exec(
        &server,
        Command::SearchSequence {
            workspace: "ws".into(),
            text: "abc".into(),
        },
    )
    .await;
    assert!(search_out.success);

    let search_full = exec(
        &server,
        Command::SearchSequence {
            workspace: "ws".into(),
            text: "abcde".into(),
        },
    )
    .await;
    assert!(search_full.success);
}

#[tokio::test]
async fn test_statistics() {
    let (_tmp, server) = tmp_server();

    exec(&server, Command::CreateWorkspace { name: "ws".into() }).await;
    exec(
        &server,
        Command::AddAtoms {
            workspace: "ws".into(),
            chars: vec!['x', 'y'],
        },
    )
    .await;

    let output = exec(
        &server,
        Command::GetStatistics {
            workspace: "ws".into(),
        },
    )
    .await;
    assert!(output.success);

    match &output.result {
        Some(CommandResult::Statistics(stats)) => {
            assert_eq!(stats.atom_count, 2);
            assert!(stats.vertex_count >= 2);
        },
        other => panic!("Expected Statistics, got {other:?}"),
    }
}

#[tokio::test]
async fn test_save_close_reopen_persistence() {
    let (_tmp, server) = tmp_server();

    exec(
        &server,
        Command::CreateWorkspace {
            name: "persist".into(),
        },
    )
    .await;
    exec(
        &server,
        Command::AddAtoms {
            workspace: "persist".into(),
            chars: vec!['p', 'q', 'r'],
        },
    )
    .await;
    exec(
        &server,
        Command::InsertSequence {
            workspace: "persist".into(),
            text: "pqr".into(),
        },
    )
    .await;

    let save_out = exec(
        &server,
        Command::SaveWorkspace {
            name: "persist".into(),
        },
    )
    .await;
    assert!(save_out.success, "save should succeed");

    let close_out = exec(
        &server,
        Command::CloseWorkspace {
            name: "persist".into(),
        },
    )
    .await;
    assert!(close_out.success);

    let open_out = exec(
        &server,
        Command::OpenWorkspace {
            name: "persist".into(),
        },
    )
    .await;
    assert!(open_out.success);

    let stats_out = exec(
        &server,
        Command::GetStatistics {
            workspace: "persist".into(),
        },
    )
    .await;
    assert!(stats_out.success);
    match &stats_out.result {
        Some(CommandResult::Statistics(stats)) => {
            assert!(
                stats.atom_count >= 3,
                "should still have at least 3 atoms"
            );
        },
        other => panic!("Expected Statistics, got {other:?}"),
    }
}

#[tokio::test]
async fn test_list_workspaces() {
    let (_tmp, server) = tmp_server();

    let out = exec(&server, Command::ListWorkspaces).await;
    assert!(out.success);
    match &out.result {
        Some(CommandResult::WorkspaceInfoList { workspaces }) => {
            assert!(workspaces.is_empty());
        },
        other => panic!("Expected WorkspaceInfoList, got {other:?}"),
    }

    exec(&server, Command::CreateWorkspace { name: "a".into() }).await;

    let out = exec(&server, Command::ListWorkspaces).await;
    assert!(out.success);
    match &out.result {
        Some(CommandResult::WorkspaceInfoList { workspaces }) => {
            assert_eq!(workspaces.len(), 1);
        },
        other => panic!("Expected WorkspaceInfoList, got {other:?}"),
    }
}

#[tokio::test]
async fn test_execute_input_serde_flatten() {
    let json = r#"{"command": "create_workspace", "name": "demo"}"#;
    let input: ExecuteInput =
        serde_json::from_str(json).expect("should deserialize");
    match input.command {
        Command::CreateWorkspace { name } => {
            assert_eq!(name, "demo");
        },
        other => panic!("Expected CreateWorkspace, got {other:?}"),
    }
}

#[tokio::test]
async fn test_execute_input_serde_complex() {
    let json = r#"{"command": "insert_sequence", "workspace": "ws", "text": "hello world"}"#;
    let input: ExecuteInput =
        serde_json::from_str(json).expect("should deserialize");
    match input.command {
        Command::InsertSequence { workspace, text } => {
            assert_eq!(workspace, "ws");
            assert_eq!(text, "hello world");
        },
        other => panic!("Expected InsertSequence, got {other:?}"),
    }
}

#[tokio::test]
async fn test_validate_graph() {
    let (_tmp, server) = tmp_server();

    exec(&server, Command::CreateWorkspace { name: "ws".into() }).await;

    let output = exec(
        &server,
        Command::ValidateGraph {
            workspace: "ws".into(),
        },
    )
    .await;
    assert!(output.success);

    match &output.result {
        Some(CommandResult::ValidationReport(report)) => {
            assert!(report.valid, "empty graph should be valid");
        },
        other => panic!("Expected ValidationReport, got {other:?}"),
    }
}