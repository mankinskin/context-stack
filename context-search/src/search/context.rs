//! Search kinds and the search context (legacy `search/context.rs`).
//! Only the ancestor search exists (Decision 7.2).

use context_trace::{
    positioned::PositionedGraph,
    GraphKind,
};

use crate::policy::{
    DirectedTraversalPolicy,
    SearchKind,
};

/// Raises states at their root end into every parent occurrence.
#[derive(Debug)]
pub struct AncestorPolicy;

impl DirectedTraversalPolicy for AncestorPolicy {}

/// The graph a search reads, borrowed from one read guard for the whole
/// search.
#[derive(Debug)]
pub struct SearchCtx<'g, G: GraphKind> {
    pub(crate) graph: &'g PositionedGraph<G>,
}

impl<G: GraphKind> Clone for SearchCtx<'_, G> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<G: GraphKind> Copy for SearchCtx<'_, G> {}

impl<'g, G: GraphKind> SearchCtx<'g, G> {
    pub fn new(graph: &'g PositionedGraph<G>) -> Self {
        Self { graph }
    }
    pub fn graph(&self) -> &'g PositionedGraph<G> {
        self.graph
    }
}

#[derive(Debug, Default)]
pub struct AncestorSearchTraversal;

impl SearchKind for AncestorSearchTraversal {
    type Policy = AncestorPolicy;
}
