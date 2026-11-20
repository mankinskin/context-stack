#![deny(clippy::disallowed_methods)]
#![feature(test)]
//#![feature(hash_drain_filter)]
//#![feature(pin_macro)]
//#![feature(return_position_impl_trait_in_trait)]
#![cfg_attr(feature = "legacy", feature(type_changing_struct_update))]

extern crate test;

/// Positioned-model boundary search (prefixes/postfixes with occurrence
/// paths), independent of `legacy`.
pub mod boundary;

/// Cursor typestates and checkpoints shared with `legacy` (promoted, plan 2.0).
pub mod cursor;

/// Positioned search errors.
pub mod error;
pub use error::SearchError;

/// Row-model search, fenced off from the model-agnostic root.
#[cfg(feature = "legacy")]
pub mod legacy;
