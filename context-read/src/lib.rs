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

pub(crate) mod complement;
pub(crate) mod expansion;
pub(crate) mod input;
pub mod pipeline;
pub(crate) mod request;
pub(crate) mod segment;

use context_trace::{
    HypergraphRef,
    Token,
};

// Re-export key public types from the context module.
pub use input::IntoReadInput;
pub use pipeline::{
    ReadSequenceIter,
    SegmentResult,
};

/// One-shot entry point: read `input` into `graph` and return the root token.
///
/// Returns `None` when the input is empty.
///
/// # Example
/// ```rust,ignore
/// let root = context_read::read(&graph, "hello world");
/// ```
pub fn read(
    graph: &HypergraphRef,
    input: impl IntoReadInput,
) -> Option<Token> {
    pipeline::ReadCtx::new(graph.clone(), input).read_sequence()
}

#[cfg(any(test, feature = "test-api"))]
pub mod tests;
