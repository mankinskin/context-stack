//! [`Publication`]: the record of one edit.

use context_trace::{
    graph::{
        vertex::{
            CommittedIndex,
            VertexId,
        },
    },
    positioned::Occurrence,
    Token,
};

/// Record of one edit: created tokens and changed existing vertices.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Publication<I: VertexId = CommittedIndex> {
    /// Created tokens in creation order.
    pub created: Vec<Token<I>>,
    /// One entry per changed vertex that existed before the edit.
    pub edits: Vec<VertexEdit<I>>,
}

impl<I: VertexId> Default for Publication<I> {
    fn default() -> Self {
        Self {
            created: Vec::new(),
            edits: Vec::new(),
        }
    }
}

/// Occurrences added to and removed from the composition of `vertex`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VertexEdit<I: VertexId = CommittedIndex> {
    pub vertex: I,
    pub added: Vec<Occurrence<I>>,
    pub removed: Vec<Occurrence<I>>,
}
