//! Positioned insertion (insert plan rev 3): split graph, join and
//! `insert_match`. The row model lives in [`legacy`].

/// `GraphEdit`: every mutation of the positioned graph.
pub mod edit;
/// The insert lane of the reader fixtures (`test-api`).
#[cfg(any(test, feature = "test-api"))]
pub mod fixtures;
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

