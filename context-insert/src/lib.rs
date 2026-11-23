//! Positioned insertion (insert plan rev 3): split graph, join and
//! `insert_match`. The row model lives in [`legacy`].

pub mod insert;
pub mod join;
pub mod split;

pub use insert::{
    InsertError,
    InsertOutcome,
    insert,
    insert_match,
    insert_next_match,
};

#[cfg(test)]
mod tests;

/// Row-model insertion, fenced off from the model-agnostic root.
#[cfg(feature = "legacy")]
pub mod legacy;
