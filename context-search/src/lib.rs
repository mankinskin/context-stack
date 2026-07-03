#![deny(clippy::disallowed_methods)]
#![feature(test)]
//#![feature(hash_drain_filter)]
//#![feature(pin_macro)]
//#![feature(return_position_impl_trait_in_trait)]
#![feature(type_changing_struct_update)]

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
