//! Row-model search (legacy).
//!
//! Frozen: no new code here, only fixes needed to keep the legacy tests
//! green. New code lives in the crate root modules and must not depend on
//! this module.
#![feature(test)]
#![feature(type_changing_struct_update)]
#![deny(clippy::disallowed_methods)]
extern crate test;

// Mirror root items (incl. `#[macro_export]` macros) into this namespace.


// Shadows the positioned root `compare` on purpose.
#[allow(hidden_glob_reexports)]
pub(crate) mod compare;
pub mod container;
// Shadows the promoted root `cursor` on purpose; re-exports it plus the
// row-specific cursor parts.
#[allow(hidden_glob_reexports)]
pub(crate) mod cursor;
// Shadows the positioned root `match` on purpose.
#[allow(hidden_glob_reexports)]
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
