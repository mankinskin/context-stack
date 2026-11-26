//! `context-mcp` — MCP server for context-engine graphs.
//!
//! Exposes the `context-api` command surface to agents as typed MCP tools
//! (`context_search`, `context_read`, …) plus a raw `context_execute`
//! escape hatch. Every tool returns the `context_api::Response` JSON
//! unchanged (compact), subject to an output budget.
//!
//! ```text
//! MCP client ──stdio──► ContextServer (rmcp tools)
//!                           │ spawn_blocking
//!                           ▼
//!                  Box<dyn Backend + Send>   ← local Session (stage 5)
//!                                            ← RemoteBackend (proxy, stage 4)
//! ```
//!
//! stdout is reserved for MCP JSON-RPC; log to stderr only.
//!
//! Plan: `context-api/agents/plans/20261126_PLAN_CONTEXT_TOOLS/05_MCP.md`.

mod budget;
mod params;
mod server;

use std::path::PathBuf;

use context_api::{
    SaveMode,
    Session,
    SessionConfig,
};
use rmcp::{
    ServiceExt,
    transport::stdio,
};

pub use self::{
    params::*,
    server::ContextServer,
};

/// Default output budget of one tool result in bytes.
pub const DEFAULT_MAX_OUTPUT: usize = 32 * 1024;
/// Default hard vertex cap of `context_render`.
pub const DEFAULT_RENDER_CAP: usize = 200;

/// Configuration of the MCP server.
#[derive(Debug, Clone)]
pub struct McpConfig {
    /// Directory of `<name>.ctx` graphs.
    pub store_dir: PathBuf,
    /// `files` inputs are allowed below this directory (relative paths are
    /// resolved against it); `None` refuses file inputs.
    pub allow_fs: Option<PathBuf>,
    /// Maximum size of one tool result in bytes; larger results are
    /// truncated item-wise.
    pub max_output: usize,
    /// Register `context_delete_graph` and allow destructive overwrites.
    pub allow_delete: bool,
    /// Hard vertex cap of rendering (`None`: the API default gate only).
    pub render_cap: Option<usize>,
}

impl McpConfig {
    pub fn new(store_dir: impl Into<PathBuf>) -> Self {
        Self {
            store_dir: store_dir.into(),
            allow_fs: None,
            max_output: DEFAULT_MAX_OUTPUT,
            allow_delete: false,
            render_cap: Some(DEFAULT_RENDER_CAP),
        }
    }

    /// The configuration of the local [`Session`]: graphs stay open (and
    /// locked) between tool calls and every write is saved immediately.
    pub fn session_config(&self) -> SessionConfig {
        let mut config = SessionConfig::new(self.store_dir.clone());
        config.keep_open = true;
        config.save = SaveMode::OnWrite;
        config.allow_fs = self.allow_fs.clone();
        config.render_cap = self.render_cap;
        config
    }

    /// A local session over the configured store.
    pub fn local_session(&self) -> Session {
        Session::new(self.session_config())
    }
}

impl Default for McpConfig {
    fn default() -> Self {
        Self::new(".context-engine")
    }
}

pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// Serve the tools over stdio (local mode) until the client disconnects.
pub async fn run_stdio(config: McpConfig) -> Result<(), BoxError> {
    tracing::info!(store = %config.store_dir.display(), "context-mcp serving on stdio");
    let server = ContextServer::new(config);
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
