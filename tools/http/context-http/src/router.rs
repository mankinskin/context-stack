//! Router configuration for the context-engine HTTP server.
//!
//! Assembles all routes (RPC, REST, and optionally GraphQL) into
//! a single [`axum::Router`] with CORS middleware and optional
//! static file serving.

use std::path::PathBuf;

use viewer_api::{
    axum::{
        routing::{
            get,
            post,
        },
        Router,
    },
    default_cors,
    with_static_files,
};

use crate::{
    log_rest,
    rest,
    rpc,
    state::AppState,
};

/// Create the HTTP router with all routes.
///
/// This follows the same pattern as `log-viewer`'s `create_router()`:
/// routes are grouped under `/api/`, CORS is applied as a middleware
/// layer, and an optional static directory is served as a fallback.
///
/// # Routes
///
/// ## RPC (primary)
///
/// - `POST /api/execute` — accepts any [`Command`] JSON, returns
///   [`CommandResult`] JSON. This is the single endpoint that covers
///   100 % of the API surface.
///
/// ## REST (convenience)
///
/// - `GET  /api/health`                        — health check
/// - `GET  /api/workspaces`                    — list workspaces
/// - `GET  /api/workspaces/:name/snapshot`     — graph snapshot
/// - `GET  /api/workspaces/:name/atoms`        — list atoms
/// - `GET  /api/workspaces/:name/statistics`   — graph statistics
/// - `GET  /api/workspaces/:name_a/diff/:name_b[?mode=full|subset]` — graph diff
///
/// ## GraphQL (optional, feature-gated)
///
/// - `POST /api/graphql`  — execute a GraphQL query
/// - `GET  /api/graphql`  — GraphQL Playground (development)
pub fn create_router(
    state: AppState,
    static_dir: Option<PathBuf>,
) -> Router {
    let router = Router::new()
        // ── Primary RPC endpoint ─────────────────────────────────
        .route("/api/execute", post(rpc::execute_command))
        // ── Convenience REST endpoints ───────────────────────────
        .route("/api/health", get(rest::health))
        .route("/api/workspaces", get(rest::list_workspaces))
        .route("/api/workspaces/{name}/snapshot", get(rest::get_snapshot))
        .route("/api/workspaces/{name}/atoms", get(rest::list_atoms))
        .route("/api/workspaces/{name}/vertices", get(rest::list_vertices))
        .route(
            "/api/workspaces/{name}/statistics",
            get(rest::get_statistics),
        )
        .route(
            "/api/workspaces/{name_a}/diff/{name_b}",
            get(rest::diff_workspaces),
        )
        // ── Log REST endpoints ───────────────────────────────────
        .route(
            "/api/workspaces/{name}/logs",
            get(log_rest::list_logs).delete(log_rest::delete_logs),
        )
        .route(
            "/api/workspaces/{name}/logs/search",
            get(log_rest::search_logs),
        )
        .route(
            "/api/workspaces/{name}/logs/{filename}",
            get(log_rest::get_log).delete(log_rest::delete_log),
        )
        .route(
            "/api/workspaces/{name}/logs/{filename}/query",
            get(log_rest::query_log),
        )
        .route(
            "/api/workspaces/{name}/logs/{filename}/analysis",
            get(log_rest::analyze_log),
        )
        .layer(default_cors());

    // Conditionally add GraphQL routes when the feature is enabled.
    #[cfg(feature = "graphql")]
    let router = crate::graphql::add_graphql_routes(router, &state);

    let router = router.with_state(state);

    // Optionally serve static files (future web frontend).
    with_static_files(router, static_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn router_builds_without_panic() {
        let state = AppState::new_in_memory();
        let _router = create_router(state, None);
    }

    #[test]
    fn router_builds_with_missing_static_dir() {
        let state = AppState::new_in_memory();
        let _router =
            create_router(state, Some(PathBuf::from("/nonexistent/dir")));
    }
}
