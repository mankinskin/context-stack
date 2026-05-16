//! MCP Server for context-engine hypergraph workspaces.
//!
//! Exposes the `context-api` command surface through three MCP tools:
//!
//! - **`execute`** — Run any context-engine command (the primary workhorse).
//! - **`help`** — Discover available commands, grouped by category, with
//!   examples and parameter descriptions. Agents should call this first.
//! - **`workflow`** — Get ready-to-use command sequences for common tasks
//!   (basic setup, bulk insert, search & read, etc.).
//!
//! The server runs over stdio transport using `rmcp`, following the same
//! pattern as `tools/log-viewer` and `tools/doc-viewer`.

mod handlers;
mod help;
mod metadata;
#[cfg(test)]
mod tests;
mod types;
mod workflow;

use std::{
    path::PathBuf,
    sync::{
        Arc,
        Mutex,
    },
};

use context_api::workspace::manager::WorkspaceManager;
use rmcp::{
    ErrorData as McpError,
    ServiceExt,
    handler::server::{
        tool::ToolRouter,
        wrapper::Parameters,
    },
    model::CallToolResult,
    tool,
    tool_router,
    transport::stdio,
};

pub(crate) use help::{
    all_categories,
    all_commands,
};
pub(crate) use types::{
    CategoryInfo,
    CommandHelp,
    ExecuteInput,
    ExecuteOutput,
    HelpInput,
    ParamHelp,
    WorkflowInput,
    WorkflowStep,
    WorkflowSummary,
    WorkflowTemplate,
    extract_workspace_name,
};
pub(crate) use workflow::{
    available_workflows,
    build_workflow,
};

/// MCP Server for context-engine hypergraph operations.
///
/// Wraps a `WorkspaceManager` in `Arc<Mutex<>>` for interior mutability
/// (MCP tool handlers receive `&self` but `execute()` needs `&mut`).
/// The Mutex is appropriate because stdio MCP processes one tool call at a
/// time.
#[derive(Clone)]
pub struct ContextServer {
    /// Shared workspace manager.
    manager: Arc<Mutex<WorkspaceManager>>,
    tool_router: ToolRouter<Self>,
}

impl ContextServer {
    /// Create a new server rooted at the given base directory.
    ///
    /// Workspaces will be stored under `<base_dir>/.context-engine/`.
    pub fn new(base_dir: PathBuf) -> Self {
        let manager = WorkspaceManager::new(base_dir);
        Self {
            manager: Arc::new(Mutex::new(manager)),
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_router]
impl ContextServer {
    #[tool(
        name = "execute",
        description = "Execute a context-engine hypergraph command. Send any Command JSON object and receive the corresponding CommandResult. Call the 'help' tool first to discover available commands, or the 'workflow' tool for ready-to-use command sequences."
    )]
    async fn execute_command(
        &self,
        input: Parameters<ExecuteInput>,
    ) -> Result<CallToolResult, McpError> {
        handlers::execute_command_impl(self, input).await
    }

    #[tool(
        name = "help",
        description = "Discover available context-engine commands. Call with no arguments for an overview of all commands grouped by category. Set 'command' to get detailed help for one command (parameters, example, result type, notes). Set 'category' to list only commands in that group (workspace, atoms, patterns, search, insert, read, debug, logs)."
    )]
    async fn help_command(
        &self,
        input: Parameters<HelpInput>,
    ) -> Result<CallToolResult, McpError> {
        handlers::help_command_impl(self, input).await
    }

    #[tool(
        name = "workflow",
        description = "Get ready-to-use command sequences for common tasks. Call with name='list' (or no arguments) to see available workflows. Set name to 'basic', 'bulk_insert', 'search_and_read', 'inspect', 'persistence', or 'trace_and_debug' to get a step-by-step template with example commands you can pass directly to the 'execute' tool. Set 'workspace' to customize the workspace name in the template (default: 'demo')."
    )]
    async fn workflow_command(
        &self,
        input: Parameters<WorkflowInput>,
    ) -> Result<CallToolResult, McpError> {
        handlers::workflow_command_impl(self, input).await
    }
}

/// Run the MCP server on stdio transport.
///
/// This blocks until the MCP client disconnects (stdin EOF).
pub async fn run_mcp_server(
    base_dir: PathBuf
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let server = ContextServer::new(base_dir);

    tracing::info!("Starting context-mcp server on stdio");

    let service = server.serve(stdio()).await.inspect_err(|e| {
        eprintln!("Server error: {:?}", e);
    })?;

    service.waiting().await?;

    Ok(())
}
