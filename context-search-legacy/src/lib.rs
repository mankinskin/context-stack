//! Row-model search (legacy).
//!
//! Extracted from `context_search::legacy`. Frozen: no new code here, only fixes needed
//! to keep the legacy tests green. New code lives in `context-search` and must
//! not depend on this crate.
#![feature(test)]
#![feature(type_changing_struct_update)]
#![deny(clippy::disallowed_methods)]
extern crate test;

pub(crate) mod compare;
pub mod container;
pub(crate) mod cursor;
pub(crate) mod r#match;
pub mod policy;
pub mod search;
pub mod state;

/// Compact formatting for logging
pub(crate) mod logging;

#[cfg(any(test, feature = "test-api"))]
pub mod tests;

pub use crate::{
    container::bft::BftQueue,
    policy::SearchKind,
    search::{
        context::AncestorPolicy,
        searchable::{
            ErrorState,
            Searchable,
        },
        Find,
    },
    state::response::Response,
};
