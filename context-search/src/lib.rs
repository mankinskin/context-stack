#![deny(clippy::disallowed_methods)]
#![feature(test)]
//#![feature(hash_drain_filter)]
//#![feature(pin_macro)]
//#![feature(return_position_impl_trait_in_trait)]
#![cfg_attr(feature = "legacy", feature(type_changing_struct_update))]

extern crate test;

/// Cursor typestates and checkpoints shared with `legacy` (promoted, plan 2.0).
pub mod cursor;

/// Positioned search errors.
pub mod error;
pub use error::SearchError;

// Positioned ancestor search (plan 2.3–2.5). Module layout and names mirror
// `legacy`; only navigation (positions instead of rows) differs.
pub mod compare;
pub mod r#match;
pub mod policy;
pub mod search;
pub mod state;

pub use search::{
    Find,
    searchable::Searchable,
};
pub use state::response::Response;

#[cfg(test)]
mod tests;

/// Row-model search, fenced off from the model-agnostic root.
#[cfg(feature = "legacy")]
pub mod legacy;
