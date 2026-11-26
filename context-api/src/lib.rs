//! `context-api` — the command API of the context stack.
//!
//! Every tool (CLI, shell, HTTP/WebSocket server, MCP server) sends a
//! [`Request`] to a [`Session`] (in process or through a server) and gets a
//! [`Response`]. The session owns a store of graphs (`<store>/<name>.ctx`),
//! reads text into them with `context_read::read` (the only mutation),
//! searches them with `context-search`, and renders small graphs.
//!
//! ```text
//! context-cli / shell ─┐
//! context-http ────────┼──► Session::execute(Request) -> Response
//! context-mcp ─────────┘          │
//!                     context-read · context-search · context-trace
//! ```
//!
//! Plan: `agents/plans/20261126_PLAN_CONTEXT_TOOLS/`.

pub mod ascii_graph;
mod base64;
pub mod command;
pub mod error;
pub mod event;
pub mod session;
pub mod types;

#[cfg(feature = "logs")]
pub mod jq;
#[cfg(feature = "logs")]
pub mod log_parser;
#[cfg(feature = "logs")]
pub mod logs;
#[cfg(feature = "logs")]
pub mod tracing_capture;

pub use self::{
    command::{
        Command,
        CommandResult,
        Outcome,
        Request,
        Response,
    },
    error::{
        ApiError,
        ErrorKind,
        ErrorResponse,
    },
    event::{
        Event,
        EventSink,
        Topic,
    },
    session::{
        Backend,
        SaveMode,
        Session,
        SessionConfig,
    },
};

/// Canonical `export_to` path for ts-rs (`#[ts(export_to = "...")]`),
/// relative to `<crate>/bindings/`. Must match [`ts_export!`].
pub const TS_EXPORT_DIR: &str =
    "../../../packages/context-types/src/generated/";

/// Derives and exports a TypeScript type via ts-rs (feature `ts-gen`) for
/// every wrapped item, keeping the export path in one place.
#[macro_export]
macro_rules! ts_export {
    ($($item:item)*) => {
        $(
            #[cfg_attr(feature = "ts-gen", derive(ts_rs::TS))]
            #[cfg_attr(
                feature = "ts-gen",
                ts(export, export_to = "../../../packages/context-types/src/generated/")
            )]
            $item
        )*
    };
}
