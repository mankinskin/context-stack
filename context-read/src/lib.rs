#![deny(clippy::disallowed_methods)]

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
/// This is a thin convenience wrapper over `ReadCtx::new(...).read_sequence()`.
/// The returned token is the materialized root for the consumed input span.
/// The graph may store multiple first-class decompositions for that token; this
/// entry point does not choose or expose a canonical child-pattern ordering.
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
