//! Command module — the `Command` enum, `CommandResult` enum, dispatch
//! function, and `WorkspaceApi` trait.
//!
//! This module provides two complementary interfaces:
//!
//! 1. **`WorkspaceApi` trait** — a strongly-typed Rust trait that
//!    `WorkspaceManager` implements. Ideal for Rust consumers that want
//!    compile-time checked method calls.
//!
//! 2. **`Command` / `CommandResult` enums** — a serializable
//!    request/response pair suitable for adapter layers (CLI, MCP, HTTP).
//!    The `execute` function dispatches a `Command` to the appropriate
//!    `WorkspaceApi` method and wraps the result in a `CommandResult`.

pub mod atoms;
pub mod compare;
pub mod debug;
pub mod export_import;
pub mod insert;
pub mod logs;
pub mod patterns;
pub mod read;
pub mod search;

mod api;
mod api_impl;
mod command;
mod dispatch;
mod result;
#[cfg(test)]
mod tests;

pub use api::WorkspaceApi;
pub use command::Command;
pub use dispatch::{
    execute,
    execute_traced,
};
pub use result::CommandResult;