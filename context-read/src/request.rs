//! Read request handling for context-read operations.
//!
//! A `ReadRequest` represents a request to read a sequence of tokens into the hypergraph.
//! Unlike the trait-based `HasReadCtx` interface, this provides a more explicit,
//! data-oriented way to specify read operations.

use context_trace::*;
use derive_builder::Builder;

use crate::{
    context::ReadCtx,
    sequence::ToNewAtomIndices,
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
pub struct ReadRequest {
    /// The input sequence to read, as a pattern of tokens/indices.
    /// For new content, this will be NewAtomIndices; for known content,
    /// this can be an existing Pattern.
    #[builder(setter(custom))]
    input: RequestInput,
}

/// The input to a read request, representing what should be read.
#[derive(Debug, Clone)]
pub enum RequestInput {
    /// A string to be tokenized and read
    Text(String),
    /// An existing pattern of tokens to read
    Pattern(Pattern),
}

impl ReadRequest {
    /// Create a new read request from text input.
    pub fn from_text(text: impl Into<String>) -> Self {
        Self {
            input: RequestInput::Text(text.into()),
        }
    }

    /// Create a new read request from an existing pattern.
    pub fn from_pattern(pattern: impl IntoPattern) -> Self {
        Self {
            input: RequestInput::Pattern(pattern.into_pattern()),
        }
    }

    /// Execute this read request on the given graph.
    ///
    /// Returns the root token of the inserted/found sequence, or None if the
    /// input was empty.
    pub fn execute(
        self,
        graph: &mut HypergraphRef,
    ) -> Option<Token> {
        match self.input {
            RequestInput::Text(text) => {
                let mut ctx = ReadCtx::new(graph.clone(), text.chars());
                ctx.read_sequence()
            },
            RequestInput::Pattern(pattern) => {
                if pattern.is_empty() {
                    return None;
                }
                let mut ctx =
                    ReadCtx::new(graph.clone(), PatternInput(pattern));
                ctx.read_sequence()
            },
        }
    }

    /// Get the input type.
    pub fn input(&self) -> &RequestInput {
        &self.input
    }
}

impl ReadRequestBuilder {
    /// Set the input from text.
    pub fn text(
        &mut self,
        text: impl Into<String>,
    ) -> &mut Self {
        self.input = Some(RequestInput::Text(text.into()));
        self
    }

    /// Set the input from a pattern.
    pub fn pattern(
        &mut self,
        pattern: impl IntoPattern,
    ) -> &mut Self {
        self.input = Some(RequestInput::Pattern(pattern.into_pattern()));
        self
    }
}

/// Wrapper for Pattern to implement ToNewAtomIndices.
/// This allows patterns of known tokens to be processed through the same
/// read pipeline as new text input.
#[derive(Debug, Clone)]
struct PatternInput(Pattern);

impl ToNewAtomIndices for PatternInput {
    fn to_new_atom_indices<G: HasGraph<Kind = BaseGraphKind>>(
        self,
        graph: &G,
    ) -> graph::vertex::atom::NewAtomIndices {
        use graph::vertex::atom::NewAtomIndex;
        // Convert known tokens to NewAtomIndices as "known" indices
        self.0
            .into_iter()
            .map(|token| NewAtomIndex::Known(token.vertex_index()))
            .collect()
    }
}

/// Result type for read operations.
#[derive(Debug, Clone)]
pub struct ReadResult {
    /// The root token of the read sequence (if any)
    pub root: Option<Token>,
    /// The number of tokens/atoms read
    pub token_count: usize,
}

impl ReadResult {
    /// Create a new read result.
    pub fn new(
        root: Option<Token>,
        token_count: usize,
    ) -> Self {
        Self { root, token_count }
    }

    /// Returns true if the read operation produced a result.
    pub fn has_root(&self) -> bool {
        self.root.is_some()
    }

    /// Get the root token, panicking if none exists.
    pub fn expect_root(self) -> Token {
        self.root.expect("ReadResult has no root")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_request_from_text() {
        let mut graph = HypergraphRef::<BaseGraphKind>::default();
        let request = ReadRequest::from_text("abc");
        let result = request.execute(&mut graph);

        assert!(result.is_some());
        let root = result.unwrap();
        assert_eq!(root.width(), TokenWidth(3));
    }

    #[test]
    fn read_request_empty_text() {
        let mut graph = HypergraphRef::<BaseGraphKind>::default();
        let request = ReadRequest::from_text("");
        let result = request.execute(&mut graph);

        assert!(result.is_none());
    }

    #[test]
    fn read_request_builder() {
        let mut graph = HypergraphRef::<BaseGraphKind>::default();

        let request =
            ReadRequestBuilder::default().text("hello").build().unwrap();

        let result = request.execute(&mut graph);
        assert!(result.is_some());
    }
}
