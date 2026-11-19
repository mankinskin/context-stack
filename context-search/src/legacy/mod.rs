//! Row-model search (legacy).
//!
//! Frozen: no new code here, only fixes needed to keep the legacy tests
//! green. New code lives in the crate root modules and must not depend on
//! this module.

// Mirror root items (incl. `#[macro_export]` macros) into this namespace.
pub use crate::*;

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

pub use crate::legacy::{
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
