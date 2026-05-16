use context_api::{
    commands::{
        execute,
        execute_traced,
    },
    tracing_capture::CaptureConfig,
};
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::Parameters,
    model::*,
};

use super::{
    ContextServer,
    ExecuteInput,
    ExecuteOutput,
    HelpInput,
    WorkflowInput,
    all_categories,
    all_commands,
    available_workflows,
    build_workflow,
    extract_workspace_name,
};

pub(super) async fn execute_command_impl(
    server: &ContextServer,
    Parameters(input): Parameters<ExecuteInput>,
) -> Result<CallToolResult, McpError> {
        tracing::debug!(command = ?input.command, trace = input.trace, "Executing command");

        let result = {
            let mut manager = server.manager.lock().map_err(|e| {
                McpError::internal_error(format!("Lock poisoned: {e}"), None)
            })?;

            if input.trace {
                // Try to extract workspace name for log directory
                let ws_name = extract_workspace_name(&input.command);
                if let Some(ws_name) = ws_name {
                    match manager.log_dir(ws_name) {
                        Ok(log_dir) => {
                            let config = CaptureConfig {
                                enabled: true,
                                log_dir,
                                level: "TRACE".to_string(),
                            };
                            match execute_traced(
                                &mut manager,
                                input.command,
                                Some(&config),
                            ) {
                                Ok((cmd_result, trace_summary)) =>
                                    Ok((cmd_result, trace_summary)),
                                Err(e) => Err(e),
                            }
                        },
                        Err(_) => {
                            // Can't get log dir, fall back to non-traced
                            execute(&mut manager, input.command)
                                .map(|r| (r, None))
                        },
                    }
                } else {
                    // No workspace in this command (e.g. list_workspaces), run without trace
                    execute(&mut manager, input.command).map(|r| (r, None))
                }
            } else {
                execute(&mut manager, input.command).map(|r| (r, None))
            }
        };

        match result {
            Ok((cmd_result, trace_summary)) => {
                let output = ExecuteOutput {
                    success: true,
                    result: Some(cmd_result),
                    error: None,
                    trace: trace_summary,
                };
                let json =
                    serde_json::to_string_pretty(&output).map_err(|e| {
                        McpError::internal_error(
                            format!("Serialization failed: {e}"),
                            None,
                        )
                    })?;
                Ok(CallToolResult::success(vec![Content::text(json)]))
            },
            Err(api_error) => {
                tracing::warn!(error = %api_error, "Command failed");
                let output = ExecuteOutput {
                    success: false,
                    result: None,
                    error: Some(format!("{api_error}")),
                    trace: None,
                };
                let json =
                    serde_json::to_string_pretty(&output).map_err(|e| {
                        McpError::internal_error(
                            format!("Serialization failed: {e}"),
                            None,
                        )
                    })?;
                // Return as a successful MCP response with an error payload
                // so the agent can read and react to the error message.
                Ok(CallToolResult::success(vec![Content::text(json)]))
            },
        }
    }

pub(super) async fn help_command_impl(
    _server: &ContextServer,
    Parameters(input): Parameters<HelpInput>,
) -> Result<CallToolResult, McpError> {
    let commands = all_commands();

        // Case 1: Specific command requested
        if let Some(ref cmd_name) = input.command {
            let needle = cmd_name.to_lowercase();
            if let Some(cmd) = commands.iter().find(|c| c.name == needle) {
                let json = serde_json::to_string_pretty(cmd).map_err(|e| {
                    McpError::internal_error(
                        format!("Serialization failed: {e}"),
                        None,
                    )
                })?;
                return Ok(CallToolResult::success(vec![Content::text(json)]));
            } else {
                // Fuzzy: show commands that contain the search term
                let matches: Vec<&str> = commands
                    .iter()
                    .filter(|c| {
                        c.name.contains(&needle)
                            || c.summary.to_lowercase().contains(&needle)
                    })
                    .map(|c| c.name)
                    .collect();

                let msg = if matches.is_empty() {
                    format!(
                        "No command named '{}'. Use help with no arguments to see all commands.",
                        cmd_name,
                    )
                } else {
                    format!(
                        "No exact match for '{}'. Did you mean one of: {}?\n\nCall help with the exact command name for details.",
                        cmd_name,
                        matches.join(", "),
                    )
                };
                return Ok(CallToolResult::success(vec![Content::text(msg)]));
            }
        }

        // Case 2: Specific category requested
        if let Some(ref cat_name) = input.category {
            let needle = cat_name.to_lowercase();
            let categories = all_categories();
            let cat = categories.iter().find(|c| c.name == needle);

            if cat.is_none() {
                let valid: Vec<&str> =
                    categories.iter().map(|c| c.name).collect();
                return Ok(CallToolResult::success(vec![Content::text(
                    format!(
                        "Unknown category '{}'. Valid categories: {}",
                        cat_name,
                        valid.join(", "),
                    ),
                )]));
            }
            let cat = cat.unwrap();

            let cat_commands: Vec<_> =
                commands.iter().filter(|c| c.category == needle).collect();

            let mut text = format!(
                "## {} ({})\n\n{}\n\n",
                cat.display, cat.name, cat.description
            );
            for cmd in &cat_commands {
                text.push_str(&format!(
                    "### {}\n{}\n\nExample:\n```\n{}\n```\n\nResult: {}\n\n",
                    cmd.name, cmd.summary, cmd.example, cmd.result_type
                ));
                if !cmd.parameters.is_empty() {
                    text.push_str("Parameters:\n");
                    for p in cmd.parameters {
                        text.push_str(&format!(
                            "  - `{}` ({}{}): {}\n",
                            p.name,
                            p.param_type,
                            if p.required {
                                ", required"
                            } else {
                                ", optional"
                            },
                            p.description,
                        ));
                    }
                    text.push('\n');
                }
                if !cmd.notes.is_empty() {
                    text.push_str(&format!("Note: {}\n\n", cmd.notes));
                }
                text.push_str("---\n\n");
            }

            return Ok(CallToolResult::success(vec![Content::text(text)]));
        }

        // Case 3: Full overview
        let categories = all_categories();
        let mut text = String::from(
            "# Context Engine — Command Reference\n\n\
             Use the `execute` tool to run any command. Use `help` with a specific\n\
             command name for detailed parameters and examples.\n\n\
             **Typical workflow:** create_workspace → add_atoms → insert_sequence → search_sequence → read_as_text → save_workspace\n\n\
             **Tip:** Use the `workflow` tool for ready-to-use command sequences.\n\n",
        );

        for cat in &categories {
            let cat_commands: Vec<_> =
                commands.iter().filter(|c| c.category == cat.name).collect();

            text.push_str(&format!(
                "## {} ({})\n\n{}\n\n",
                cat.display, cat.name, cat.description
            ));
            text.push_str("| Command | Summary |\n|---------|----------|\n");
            for cmd in &cat_commands {
                text.push_str(&format!(
                    "| `{}` | {} |\n",
                    cmd.name, cmd.summary
                ));
            }
            text.push('\n');
        }

        text.push_str(
            "---\n\n\
             Call `help` with `command: \"<name>\"` for full parameter details and examples.\n\
             Call `help` with `category: \"<name>\"` for all commands in a category.\n",
        );

    Ok(CallToolResult::success(vec![Content::text(text)]))
}

pub(super) async fn workflow_command_impl(
    _server: &ContextServer,
    Parameters(input): Parameters<WorkflowInput>,
) -> Result<CallToolResult, McpError> {
        let name = input.name.to_lowercase();

        // List available workflows
        if name == "list" {
            let summaries = available_workflows();
            let mut text = String::from(
                "# Available Workflow Templates\n\n\
                 Call `workflow` with `name` set to one of these to get step-by-step commands:\n\n\
                 | Name | Description |\n\
                 |------|-------------|\n",
            );
            for s in &summaries {
                text.push_str(&format!(
                    "| `{}` | {} |\n",
                    s.name, s.description
                ));
            }
            text.push_str(&format!(
                "\nExample: call `workflow` with `name: \"basic\", workspace: \"my-graph\"` \
                 to get the basic workflow customized for your workspace name.\n"
            ));
            return Ok(CallToolResult::success(vec![Content::text(text)]));
        }

        // Build the requested workflow
        match build_workflow(&name, &input.workspace) {
            Some(wf) => {
                let json = serde_json::to_string_pretty(&wf).map_err(|e| {
                    McpError::internal_error(
                        format!("Serialization failed: {e}"),
                        None,
                    )
                })?;
                Ok(CallToolResult::success(vec![Content::text(json)]))
            },
            None => {
                let available: Vec<&str> =
                    available_workflows().iter().map(|s| s.name).collect();
                Ok(CallToolResult::success(vec![Content::text(format!(
                    "Unknown workflow '{}'. Available workflows: {}\n\nCall workflow with name='list' to see descriptions.",
                    name,
                    available.join(", "),
                ))]))
            },
        }
}