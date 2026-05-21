//! context-http — HTTP + GraphQL adapter for the context-engine API.
//!
//! This binary exposes the entire `context-api` command surface over HTTP.
//! The primary interface is an RPC-style `POST /api/execute` endpoint that
//! accepts a `Command` JSON body and returns a `CommandResult` JSON response,
//! exactly mirroring the MCP adapter's semantics but over HTTP.

use std::path::PathBuf;

use axum::serve;
use tracing::info;
use tracing_subscriber::{
    fmt,
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter,
};

use context_api::workspace::manager::WorkspaceManager;

use context_http::state::AppState;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing (console + optional file logging via LOG_FILE env)
    init_tracing(PathBuf::from("logs"));

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

    serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

fn init_tracing(log_dir: PathBuf) {
    let level =
        std::env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string());
    let file_logging = std::env::var("LOG_FILE").is_ok();
    let filter =
        EnvFilter::try_new(&level).unwrap_or_else(|_| EnvFilter::new("info"));
    let fmt_layer = fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true);

    if file_logging {
        let _ = std::fs::create_dir_all(&log_dir);
        let file_appender =
            tracing_appender::rolling::daily(&log_dir, "context-http.log");
        let (non_blocking, guard) =
            tracing_appender::non_blocking(file_appender);
        std::mem::forget(guard);

        let file_layer = fmt::layer()
            .with_writer(non_blocking)
            .with_ansi(false)
            .with_target(true)
            .with_thread_ids(true)
            .with_file(true)
            .with_line_number(true);

        tracing_subscriber::registry()
            .with(filter)
            .with(fmt_layer)
            .with(file_layer)
            .init();
    } else {
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt_layer)
            .init();
    }
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    info!("Received shutdown signal");
}
