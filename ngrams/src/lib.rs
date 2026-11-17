//! Exhaustive n-gram vocabulary construction and selected decomposition graphs.
//!
//! The oracle counts substring occurrences, labels frequency tokens and overlap
//! wrappers, then builds a smaller graph while discovering gap tokens. The
//! original containment graph and final labels remain available in parse results.
//!
//! Decompositions are currently traversal-order dependent; serialization sorting
//! does not make the oracle's token-selection process order-independent.

#![allow(non_snake_case, unused)]
#![feature(mapped_lock_guards)]

use std::path::Path;

use itertools::Itertools;

#[cfg(not(debug_assertions))]
pub(crate) use {
    graph::*,
    shared::*,
};

pub mod cancellation;
pub mod config;
pub mod graph;
pub mod output;
#[cfg(not(debug_assertions))]
mod shared;
pub(crate) mod storage;
pub(crate) mod tests;

pub use crate::{
    cancellation::Cancellation,
    config::CacheConfig,
    graph::{
        vocabulary::ProcessStatus,
        Status,
    },
};
