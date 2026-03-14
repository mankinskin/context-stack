//! `context-api` — Unified API for hypergraph workspace management and operations.
//!
//! This crate provides the single public interface for all hypergraph operations
//! across the context-engine workspace. It wraps `context-trace`, `context-search`,
//! `context-insert`, and (in Phase 2) `context-read` behind a workspace-oriented,
//! command-based API with feature-gated adapters for CLI, MCP, HTTP, and future
//! protocols.
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────┐  ┌──────────────┐  ┌──────────────┐
//! │ context-cli  │  │ context-mcp  │  │ context-http │
//! │ (bin)        │  │ (bin)        │  │ (bin)        │
//! └──────┬───────┘  └──────┬───────┘  └──────┬───────┘
//!        └─────────────────┴────────┬────────┘
//!                                   │
//!                            ┌──────┴───────┐
//!                            │  context-api │  ← you are here
//!                            └──────┬───────┘
//!                                   │
//!         ┌─────────────────────────┼─────────────────────────┐
//!         │                         │                         │
//!  context-insert ──► context-search ──► context-trace
//! ```
//!
//! # Usage
//!
//! ## Rust consumers — use the `WorkspaceApi` trait
//!
//! ```rust,no_run
//! use context_api::prelude::*;
//! use context_api::workspace::manager::WorkspaceManager;
//!
//! let mut mgr = WorkspaceManager::current_dir().unwrap();
//! let info = mgr.create_workspace("demo").unwrap();
//! let atom = mgr.add_atom("demo", 'a').unwrap();
//! mgr.save_workspace("demo").unwrap();
//! ```
//!
//! ## Adapter layers — use the `Command` / `CommandResult` enums
//!
//! ```rust,no_run
//! use context_api::commands::{Command, execute};
//! use context_api::workspace::manager::WorkspaceManager;
//!
//! let mut mgr = WorkspaceManager::current_dir().unwrap();
//! let cmd: Command = serde_json::from_str(r#"{"command":"create_workspace","name":"demo"}"#).unwrap();
//! let result = execute(&mut mgr, cmd).unwrap();
//! let json = serde_json::to_string(&result).unwrap();
//! ```

// Public modules
pub mod commands;
pub mod error;
pub mod jq;
pub mod log_parser;
pub mod resolve;
pub mod tracing_capture;
pub mod types;
pub mod validation;
pub mod workspace;

// Tests (integration-level, in addition to per-module unit tests)
#[cfg(test)]
mod tests;

/// Convenience re-exports for common usage patterns.
///
/// ```rust
/// use context_api::prelude::*;
/// ```
pub mod prelude {
    pub use crate::{
        commands::{
            Command,
            CommandResult,
            WorkspaceApi,
            execute,
        },
        error::{
            ApiError,
            AtomError,
            ErrorResponse,
            InsertError,
            LogError,
            PatternError,
            ReadError,
            SearchError,
            WorkspaceError,
        },
        types::{
            AtomInfo,
            GraphStatistics,
            InsertResult,
            LogAnalysis,
            LogDeleteResult,
            LogEntryInfo,
            LogFileInfo,
            LogFileSearchResult,
            PartialMatchInfo,
            PartialMatchKind,
            PatternInfo,
            PatternReadResult,
            ReadNode,
            SearchResult,
            SpanSummary,
            TokenInfo,
            TokenRef,
            TraceSummary,
            ValidationReport,
            VertexInfo,
            WorkspaceInfo,
        },
    };
}
