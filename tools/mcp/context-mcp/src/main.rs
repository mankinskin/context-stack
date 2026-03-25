//! `context-mcp` — MCP server for context-engine hypergraph workspaces.
//!
//! Exposes the entire `context-api` command surface as a single MCP `execute`
//! tool over stdio transport. An MCP client (e.g. an AI agent) sends a
//! `Command` JSON object and receives a `CommandResult` JSON object.
//!
//! All diagnostic output goes to stderr — stdout is reserved exclusively
//! for the MCP JSON-RPC protocol.

mod server;

use std::path::PathBuf;

#[tokio::main]
async fn main() {
    // Initialize tracing to stderr (stdout is reserved for MCP stdio transport).
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("context_mcp=info".parse().unwrap()),
        )
        .with_writer(std::io::stderr)
        .init();

    // Use current directory as workspace base.
    let base_dir = std::env::current_dir().unwrap_or_else(|_| {
        eprintln!("Warning: could not determine current directory, using '.'");
        PathBuf::from(".")
    });

    eprintln!("context-mcp starting (base_dir: {})", base_dir.display());

    if let Err(e) = server::run_mcp_server(base_dir).await {
        eprintln!("Fatal error: {e}");
        std::process::exit(1);
    }
}
