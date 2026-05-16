use super::*;

#[tokio::test]
async fn test_workflow_list() {
    let (_tmp, server) = tmp_server();

    let input = Parameters(WorkflowInput {
        name: "list".to_string(),
        workspace: "demo".to_string(),
    });
    let result = server
        .workflow_command(input)
        .await
        .expect("workflow should not fail");
    let text = extract_text(&result);

    assert!(text.contains("basic"), "should list basic workflow");
    assert!(
        text.contains("bulk_insert"),
        "should list bulk_insert workflow"
    );
    assert!(
        text.contains("search_and_read"),
        "should list search_and_read"
    );
    assert!(text.contains("inspect"), "should list inspect");
    assert!(text.contains("persistence"), "should list persistence");
    assert!(
        text.contains("trace_and_debug"),
        "should list trace_and_debug"
    );
}

#[tokio::test]
async fn test_workflow_basic() {
    let (_tmp, server) = tmp_server();

    let input = Parameters(WorkflowInput {
        name: "basic".to_string(),
        workspace: "my-graph".to_string(),
    });
    let result = server
        .workflow_command(input)
        .await
        .expect("workflow should not fail");
    let text = extract_text(&result);

    let parsed: serde_json::Value = serde_json::from_str(&text)
        .expect("workflow output should be JSON");

    assert_eq!(parsed["name"], "basic");
    assert!(
        parsed["steps"].as_array().unwrap().len() >= 5,
        "basic workflow should have at least 5 steps"
    );
    assert!(
        parsed["tips"].as_array().unwrap().len() > 0,
        "should have tips"
    );

    let json_text = text;
    assert!(
        json_text.contains("my-graph"),
        "should contain customized workspace name"
    );
}

#[tokio::test]
async fn test_workflow_default_workspace() {
    let (_tmp, server) = tmp_server();

    let input = Parameters(WorkflowInput {
        name: "basic".to_string(),
        workspace: "demo".to_string(),
    });
    let result = server
        .workflow_command(input)
        .await
        .expect("workflow should not fail");
    let text = extract_text(&result);

    assert!(text.contains("demo"), "default workspace should be 'demo'");
}

#[tokio::test]
async fn test_workflow_unknown() {
    let (_tmp, server) = tmp_server();

    let input = Parameters(WorkflowInput {
        name: "nonexistent".to_string(),
        workspace: "demo".to_string(),
    });
    let result = server
        .workflow_command(input)
        .await
        .expect("workflow should not fail");
    let text = extract_text(&result);

    assert!(
        text.contains("Unknown workflow"),
        "should report unknown: {text}"
    );
    assert!(text.contains("basic"), "should suggest available workflows");
}

#[tokio::test]
async fn test_workflow_all_templates_build() {
    let summaries = available_workflows();
    for s in &summaries {
        let wf = build_workflow(s.name, "test-ws");
        assert!(
            wf.is_some(),
            "Workflow '{}' is advertised but build_workflow returns None",
            s.name,
        );
        let wf = wf.unwrap();
        assert!(!wf.steps.is_empty(), "Workflow '{}' has no steps", s.name);
        assert!(!wf.tips.is_empty(), "Workflow '{}' has no tips", s.name);

        let json = serde_json::to_string_pretty(&wf);
        assert!(
            json.is_ok(),
            "Workflow '{}' failed to serialize: {:?}",
            s.name,
            json.err()
        );
    }
}

#[tokio::test]
async fn test_workflow_steps_have_valid_commands() {
    let wf = build_workflow("basic", "demo").unwrap();
    for step in &wf.steps {
        let cmd = step.command.get("command");
        assert!(
            cmd.is_some(),
            "Step {} should have a 'command' field",
            step.step,
        );
        let cmd_name = cmd.unwrap().as_str().unwrap();
        let known: Vec<&str> =
            all_commands().iter().map(|c| c.name).collect();
        assert!(
            known.contains(&cmd_name),
            "Step {} references unknown command '{}' (known: {:?})",
            step.step,
            cmd_name,
            known,
        );
    }
}