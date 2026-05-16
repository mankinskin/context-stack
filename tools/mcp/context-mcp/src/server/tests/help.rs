use super::*;

#[tokio::test]
async fn test_help_overview() {
    let (_tmp, server) = tmp_server();

    let input = Parameters(HelpInput {
        command: None,
        category: None,
    });
    let result = server
        .help_command(input)
        .await
        .expect("help should not fail");
    let text = extract_text(&result);

    assert!(
        text.contains("Workspace Lifecycle"),
        "should have workspace category"
    );
    assert!(text.contains("Atoms"), "should have atoms category");
    assert!(text.contains("Search"), "should have search category");
    assert!(text.contains("Insert"), "should have insert category");
    assert!(text.contains("Read"), "should have read category");
    assert!(text.contains("Debug"), "should have debug category");

    assert!(text.contains("create_workspace"));
    assert!(text.contains("insert_sequence"));
    assert!(text.contains("search_sequence"));
}

#[tokio::test]
async fn test_help_specific_command() {
    let (_tmp, server) = tmp_server();

    let input = Parameters(HelpInput {
        command: Some("insert_sequence".to_string()),
        category: None,
    });
    let result = server
        .help_command(input)
        .await
        .expect("help should not fail");
    let text = extract_text(&result);

    let parsed: serde_json::Value = serde_json::from_str(&text)
        .expect("help output should be valid JSON");
    assert_eq!(parsed["name"], "insert_sequence");
    assert_eq!(parsed["category"], "insert");
    assert!(parsed["summary"].as_str().unwrap().len() > 0);
    assert!(
        parsed["example"]
            .as_str()
            .unwrap()
            .contains("insert_sequence")
    );
    assert!(parsed["parameters"].as_array().unwrap().len() > 0);
}

#[tokio::test]
async fn test_help_unknown_command_fuzzy() {
    let (_tmp, server) = tmp_server();

    let input = Parameters(HelpInput {
        command: Some("insert".to_string()),
        category: None,
    });
    let result = server
        .help_command(input)
        .await
        .expect("help should not fail");
    let text = extract_text(&result);

    assert!(
        text.contains("insert_sequence")
            || text.contains("insert_first_match"),
        "should suggest commands containing 'insert': {text}"
    );
}

#[tokio::test]
async fn test_help_category() {
    let (_tmp, server) = tmp_server();

    let input = Parameters(HelpInput {
        command: None,
        category: Some("search".to_string()),
    });
    let result = server
        .help_command(input)
        .await
        .expect("help should not fail");
    let text = extract_text(&result);

    assert!(
        text.contains("search_pattern"),
        "should list search_pattern"
    );
    assert!(
        text.contains("search_sequence"),
        "should list search_sequence"
    );
    assert!(
        !text.contains("### create_workspace"),
        "should not have workspace commands"
    );
}

#[tokio::test]
async fn test_help_invalid_category() {
    let (_tmp, server) = tmp_server();

    let input = Parameters(HelpInput {
        command: None,
        category: Some("nonexistent".to_string()),
    });
    let result = server
        .help_command(input)
        .await
        .expect("help should not fail");
    let text = extract_text(&result);

    assert!(
        text.contains("Unknown category"),
        "should report unknown category: {text}"
    );
    assert!(text.contains("workspace"), "should list valid categories");
}

#[tokio::test]
async fn test_help_all_commands_covered() {
    let commands = all_commands();
    let names: Vec<&str> = commands.iter().map(|c| c.name).collect();

    let expected = [
        "create_workspace",
        "open_workspace",
        "close_workspace",
        "save_workspace",
        "list_workspaces",
        "delete_workspace",
        "add_atom",
        "add_atoms",
        "get_atom",
        "list_atoms",
        "add_simple_pattern",
        "get_vertex",
        "list_vertices",
        "search_pattern",
        "search_sequence",
        "insert_first_match",
        "insert_sequence",
        "insert_sequences",
        "read_pattern",
        "read_as_text",
        "read_sequence",
        "read_file",
        "get_snapshot",
        "get_statistics",
        "validate_graph",
        "show_graph",
        "show_vertex",
        "list_logs",
        "get_log",
        "query_log",
        "analyze_log",
        "search_logs",
        "delete_log",
        "delete_logs",
    ];

    for exp in &expected {
        assert!(
            names.contains(exp),
            "Missing help entry for command: {}",
            exp,
        );
    }
}