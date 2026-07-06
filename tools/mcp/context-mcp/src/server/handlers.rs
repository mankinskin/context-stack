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

fn text_result(text: impl Into<String>) -> CallToolResult {
    CallToolResult::success(vec![Content::text(text.into())])
}

fn json_result<T: serde::Serialize>(value: &T) -> Result<CallToolResult, McpError> {
    let json = serde_json::to_string(value).map_err(|e| {
        McpError::internal_error(format!("Serialization failed: {e}"), None)
    })?;
    Ok(text_result(json))
}

fn matching_command_names<'a>(
    commands: &'a [super::CommandHelp],
    needle: &str,
) -> Vec<&'a str> {
    commands
        .iter()
        .filter(|command| {
            command.name.contains(needle)
                || command.summary.to_lowercase().contains(needle)
        })
        .map(|command| command.name)
        .collect()
}

fn command_help_result(
    commands: &[super::CommandHelp],
    cmd_name: &str,
) -> Result<CallToolResult, McpError> {
    let needle = cmd_name.to_lowercase();
    if let Some(command) = commands.iter().find(|command| command.name == needle) {
        return json_result(command);
    }

    let matches = matching_command_names(commands, &needle);
    let message = if matches.is_empty() {
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

    Ok(text_result(message))
}

fn render_category_help(
    category: &super::CategoryInfo,
    commands: &[super::CommandHelp],
) -> String {
    let category_commands: Vec<_> = commands
        .iter()
        .filter(|command| command.category == category.name)
        .collect();

    let mut text = format!(
        "## {} ({})\n\n{}\n\n",
        category.display, category.name, category.description
    );
    for command in &category_commands {
        text.push_str(&format!(
            "### {}\n{}\n\nExample:\n```\n{}\n```\n\nResult: {}\n\n",
            command.name, command.summary, command.example, command.result_type
        ));
        if !command.parameters.is_empty() {
            text.push_str("Parameters:\n");
            for parameter in command.parameters {
                text.push_str(&format!(
                    "  - `{}` ({}{}): {}\n",
                    parameter.name,
                    parameter.param_type,
                    if parameter.required {
                        ", required"
                    } else {
                        ", optional"
                    },
                    parameter.description,
                ));
            }
            text.push('\n');
        }
        if !command.notes.is_empty() {
            text.push_str(&format!("Note: {}\n\n", command.notes));
        }
        text.push_str("---\n\n");
    }

    text
}

fn category_help_result(
    commands: &[super::CommandHelp],
    cat_name: &str,
) -> CallToolResult {
    let needle = cat_name.to_lowercase();
    let categories = all_categories();
    let Some(category) = categories.iter().find(|category| category.name == needle) else {
        let valid: Vec<&str> = categories.iter().map(|category| category.name).collect();
        return text_result(format!(
            "Unknown category '{}'. Valid categories: {}",
            cat_name,
            valid.join(", "),
        ));
    };

    text_result(render_category_help(category, commands))
}

fn overview_help_result(commands: &[super::CommandHelp]) -> CallToolResult {
    let categories = all_categories();
    let mut text = String::from(
        "# Context Engine — Command Reference\n\n\
             Use the `execute` tool to run any command. Use `help` with a specific\n\
             command name for detailed parameters and examples.\n\n\
             **Typical workflow:** create_workspace → add_atoms → insert_sequence → search_sequence → read_as_text → save_workspace\n\n\
             **Tip:** Use the `workflow` tool for ready-to-use command sequences.\n\n",
    );

    for category in &categories {
        let category_commands: Vec<_> = commands
            .iter()
            .filter(|command| command.category == category.name)
            .collect();

        text.push_str(&format!(
            "## {} ({})\n\n{}\n\n",
            category.display, category.name, category.description
        ));
        text.push_str("| Command | Summary |\n|---------|----------|\n");
        for command in &category_commands {
            text.push_str(&format!("| `{}` | {} |\n", command.name, command.summary));
        }
        text.push('\n');
    }

    text.push_str(
        "---\n\n\
             Call `help` with `command: \"<name>\"` for full parameter details and examples.\n\
             Call `help` with `category: \"<name>\"` for all commands in a category.\n",
    );

    text_result(text)
}

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
                        execute(&mut manager, input.command).map(|r| (r, None))
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
            let json = serde_json::to_string(&output).map_err(|e| {
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
            let json = serde_json::to_string(&output).map_err(|e| {
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

    if let Some(ref cmd_name) = input.command {
        return command_help_result(&commands, cmd_name);
    }

    if let Some(ref cat_name) = input.category {
        return Ok(category_help_result(&commands, cat_name));
    }

    Ok(overview_help_result(&commands))
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
            text.push_str(&format!("| `{}` | {} |\n", s.name, s.description));
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
            let json = serde_json::to_string(&wf).map_err(|e| {
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
