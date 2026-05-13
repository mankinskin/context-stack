//! context-http — HTTP + GraphQL adapter for the context-engine API.
//!
//! This library crate exposes the internal modules so that integration tests
//! (which live in `tests/`) can construct the router and application state
//! without duplicating the wiring logic from `main.rs`.
//!
//! The binary entry point is in `main.rs` and simply calls into these modules.

pub mod error;
pub mod log_rest;
pub mod rest;
pub mod router;
pub mod rpc;
pub mod state;

#[cfg(feature = "graphql")]
pub mod graphql;
