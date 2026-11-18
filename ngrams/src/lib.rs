//! Exhaustive n-gram vocabulary construction and selected decomposition graphs.
//!
//! The oracle counts substring occurrences, selects frequency tokens into a
//! frozen retained set, and publishes every retained vertex's maximal
//! positioned child occurrences (format v3, see [`output`]). The result does
//! not depend on traversal or corpus input order.
//!
//! The legacy row-based pipeline ([`graph::parse_corpus_legacy`]) additionally
//! labels overlap wrappers and gap tokens; its decompositions are
//! traversal-order dependent.

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
