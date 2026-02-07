//! Read request handling for context-read operations.
//!
//! A `ReadRequest` represents a request to read a sequence of tokens into the hypergraph.
//! Unlike the trait-based `HasReadCtx` interface, this provides a more explicit,
//! data-oriented way to specify read operations.

use context_trace::{
    graph::vertex::atom::{
        NewAtomIndex,
        NewAtomIndices,
    },
    *,
};
use derive_builder::Builder;

use crate::{
    context::ReadCtx,
    segment::ToNewAtomIndices,
};

/// Represents a request to read a sequence into the hypergraph.
///
/// `ReadRequest` provides a structured way to specify what should be read
/// and how. It can be created from various input types (strings, patterns, etc.)
/// and processed by the reading context.
///
/// # Example
/// ```rust,ignore
/// let request = ReadRequest::new("hello world".chars());
/// let result = request.execute(&mut graph);
/// ```
#[derive(Debug, Clone, Builder)]
#[builder(setter(into))]
pub(crate) struct ReadRequest {
    /// The input sequence to read, as a pattern of tokens/indices.
    /// For new content, this will be NewAtomIndices; for known content,
    /// this can be an existing Pattern.
    #[builder(setter(custom))]
    input: RequestInput,
}

/// The input to a read request, representing what should be read.
#[derive(Debug, Clone)]
pub(crate) enum RequestInput {
    /// A string to be tokenized and read
    Text(String),
    /// An existing pattern of tokens to read
    Pattern(Pattern),
}
impl RequestInput {
    /// Check if the input is empty (no tokens to read).
    pub(crate) fn is_empty(&self) -> bool {
        match self {
            RequestInput::Text(text) => text.is_empty(),
            RequestInput::Pattern(pattern) => pattern.is_empty(),
        }
    }
}

impl ReadRequest {
    /// Create a new read request from text input.
    pub(crate) fn from_text(text: impl Into<String>) -> Self {
        Self {
            input: RequestInput::Text(text.into()),
        }
    }

    /// Create a new read request from an existing pattern.
    pub(crate) fn from_pattern(pattern: impl IntoPattern) -> Self {
        Self {
            input: RequestInput::Pattern(pattern.into_pattern()),
        }
    }

    /// Execute this read request on the given graph.
    ///
    /// Returns the root token of the inserted/found sequence, or None if the
    /// input was empty.
    pub(crate) fn execute(
        self,
        graph: &mut HypergraphRef,
    ) -> Option<Token> {
        if self.input.is_empty() {
            return None;
        }
        let mut ctx = ReadCtx::new(graph.clone(), self.input);
        ctx.read_sequence()
    }

    /// Get the input type.
    pub(crate) fn input(&self) -> &RequestInput {
        &self.input
    }
}

impl ReadRequestBuilder {
    /// Set the input from text.
    pub(crate) fn text(
        &mut self,
        text: impl Into<String>,
    ) -> &mut Self {
        self.input = Some(RequestInput::Text(text.into()));
        self
    }

    /// Set the input from a pattern.
    pub(crate) fn pattern(
        &mut self,
        pattern: impl IntoPattern,
    ) -> &mut Self {
        self.input = Some(RequestInput::Pattern(pattern.into_pattern()));
        self
    }
}

impl ToNewAtomIndices for Pattern {
    fn to_new_atom_indices<G: HasGraph<Kind = BaseGraphKind>>(
        self,
        _graph: &G,
    ) -> NewAtomIndices {
        // Convert known tokens to NewAtomIndices as "known" indices
        self.into_iter()
            .map(|token| NewAtomIndex::Known(token.vertex_index()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use context_trace::init_test_tracing;

    #[test]
    fn read_request_from_text() {
        let mut graph = HypergraphRef::<BaseGraphKind>::default();
        let _tracing = init_test_tracing!(&graph);
        let request = ReadRequest::from_text("abc");
        let result = request.execute(&mut graph);

        assert!(result.is_some());
        let root = result.unwrap();
        assert_eq!(root.width(), TokenWidth(3));
    }

    #[test]
    fn read_request_empty_text() {
        let mut graph = HypergraphRef::<BaseGraphKind>::default();
        let _tracing = init_test_tracing!(&graph);
        let request = ReadRequest::from_text("");
        let result = request.execute(&mut graph);

        assert!(result.is_none());
    }

    #[test]
    fn read_request_builder() {
        let mut graph = HypergraphRef::<BaseGraphKind>::default();
        let _tracing = init_test_tracing!(&graph);

        let request =
            ReadRequestBuilder::default().text("hello").build().unwrap();

        let result = request.execute(&mut graph);
        assert!(result.is_some());
    }
}
