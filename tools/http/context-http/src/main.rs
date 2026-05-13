//! context-http — HTTP + GraphQL adapter for the context-engine API.
//!
//! This binary exposes the entire `context-api` command surface over HTTP.
//! The primary interface is an RPC-style `POST /api/execute` endpoint that
//! accepts a `Command` JSON body and returns a `CommandResult` JSON response,
//! exactly mirroring the MCP adapter's semantics but over HTTP.

use std::path::PathBuf;

use tracing::info;
use viewer_api::{
    init_tracing_full,
    TracingConfig,
};

use context_api::workspace::manager::WorkspaceManager;

use context_http::state::AppState;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing (console + optional file logging via LOG_FILE env)
    let tracing_config =
        TracingConfig::from_env("context-http", PathBuf::from("logs"))
            .with_level("info");
    init_tracing_full(&tracing_config);

    info!("context-http starting...");

    // Determine workspace base directory.
    // Default: current working directory (WorkspaceManager creates
    // `.context-engine/` lazily underneath).
    let base_dir = std::env::var("CONTEXT_ENGINE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            std::env::current_dir().expect("failed to get current directory")
        });

    info!(base_dir = %base_dir.display(), "Workspace storage directory");

    // Create the workspace manager (infallible — dirs are created lazily).
    let manager = WorkspaceManager::new(base_dir);

    // Build application state.
    let state = AppState::new(manager);

    // Resolve host and port from environment or defaults.
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3100);

    let host =
        std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());

    let addr = format!("{host}:{port}");

    // Resolve optional static directory from environment.
    let static_dir = std::env::var("STATIC_DIR").ok().map(PathBuf::from);

    info!("Building HTTP router...");
    let app = context_http::router::create_router(state, static_dir);

    info!("Starting HTTP server on http://{addr}");

    let listener = tokio::net::TcpListener::bind(&addr).await?;

    info!("HTTP server listening on http://{addr}");

    #[cfg(feature = "graphql")]
    info!("GraphQL Playground available at http://{addr}/api/graphql");

    viewer_api::axum::serve(listener, app).await?;

    Ok(())
}
