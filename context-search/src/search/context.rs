//! Search kinds and the search context (legacy `search/context.rs`).
//! Only the ancestor search exists (Decision 7.2).

use context_trace::positioned::GraphRead;

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
pub struct SearchCtx<'g, G: GraphRead + ?Sized> {
    pub(crate) graph: &'g G,
}

impl<G: GraphRead + ?Sized> Clone for SearchCtx<'_, G> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<G: GraphRead + ?Sized> Copy for SearchCtx<'_, G> {}

impl<'g, G: GraphRead + ?Sized> SearchCtx<'g, G> {
    pub fn new(graph: &'g G) -> Self {
        Self { graph }
    }
    pub fn graph(&self) -> &'g G {
        self.graph
    }
}

#[derive(Debug, Default)]
pub struct AncestorSearchTraversal;

impl SearchKind for AncestorSearchTraversal {
    type Policy = AncestorPolicy;
}
