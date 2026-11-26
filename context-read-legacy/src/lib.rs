//! Row-model reader (legacy).
//!
//! Extracted from `context_read::legacy`. Frozen: no new code here, only
//! fixes needed to keep the legacy tests green. New code lives in
//! `context-read` and must not depend on this crate.

pub(crate) mod expansion;
pub(crate) mod input;
pub mod pipeline;
pub(crate) mod request;
pub(crate) mod segment;

use context_trace_legacy::{
    HypergraphRef,
    Token,
};

// Re-export key public types from the context module.
pub use input::IntoReadInput;
pub use pipeline::{
    ReadCtx,
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
/// let root = context_read_legacy::read(&graph, "hello world");
/// ```
pub fn read(
    graph: &HypergraphRef,
    input: impl IntoReadInput,
) -> Option<Token> {
    pipeline::ReadCtx::new(graph.clone(), input).read_sequence()
}

#[cfg(any(test, feature = "test-api"))]
pub mod tests;
