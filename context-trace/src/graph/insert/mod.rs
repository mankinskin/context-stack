//! Graph insertion operations
//!
//! Provides methods for inserting vertices, atoms, patterns, and managing
//! parent-child relationships in the hypergraph.

use std::sync::atomic::AtomicUsize;
use lazy_static::lazy_static;

mod atom;
mod parents;
mod pattern;
mod patterns;
mod range;
mod replace;
mod vertex;

lazy_static! {
    static ref VERTEX_ID_COUNTER: AtomicUsize = AtomicUsize::new(0);
}

// All implementations are in submodules and automatically available
// No need to re-export individual methods - they're part of impl Hypergraph<G>
