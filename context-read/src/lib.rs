#![deny(clippy::disallowed_methods)]
#![feature(test)]
#![feature(assert_matches)]
#![feature(try_blocks)]
//#![feature(hash_drain_filter)]
#![feature(slice_pattern)]
//#![feature(pin_macro)]
#![feature(exact_size_is_empty)]
#![feature(associated_type_defaults)]
//#![feature(return_position_impl_trait_in_trait)]

pub(crate) mod bands;
pub(crate) mod complement;
pub mod context;
pub(crate) mod expansion;
pub(crate) mod request;
pub(crate) mod segment;

#[cfg(any(test, feature = "test-api"))]
pub mod tests;
