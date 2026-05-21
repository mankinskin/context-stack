mod execute;
mod help;
mod workflow;

use super::*;
use context_api::commands::{
    Command,
    CommandResult,
};
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{
        CallToolResult,
        RawContent,
    },
};

/// Helper: create a server backed by a temporary directory.
fn tmp_server() -> (tempfile::TempDir, ContextServer) {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let server = ContextServer::new(tmp.path().to_path_buf());
    (tmp, server)
}

/// Parse the text content of a `CallToolResult` into an `ExecuteOutput`.
fn parse_output(result: &CallToolResult) -> ExecuteOutput {
    let content = result.content.first().expect("result should have content");
    match &content.raw {
        RawContent::Text(text_content) =>
            serde_json::from_str(&text_content.text)
                .expect("should parse as ExecuteOutput"),
        _ => panic!("Expected text content"),
    }
}

/// Extract raw text from a `CallToolResult`.
fn extract_text(result: &CallToolResult) -> String {
    let content = result.content.first().expect("result should have content");
    match &content.raw {
        RawContent::Text(text_content) => text_content.text.clone(),
        _ => panic!("Expected text content"),
    }
}

/// Helper to execute a command via the server and return the parsed output.
async fn exec(
    server: &ContextServer,
    command: Command,
) -> ExecuteOutput {
    let input = Parameters(ExecuteInput {
        command,
        trace: false,
    });
    let result = server
        .execute_command(input)
        .await
        .expect("MCP call should not fail");
    parse_output(&result)
}
