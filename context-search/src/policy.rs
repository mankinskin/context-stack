//! Traversal policy of positioned search (legacy `policy.rs`): how parent
//! candidates are generated from parent links.
//!
//! Positioned parents are occurrences `(parent, offset)` from
//! [`PositionedGraph::parent_occurrences`], in its deterministic order
//! (parent width, index, offset). Only the ancestor policy exists
//! (Decision 7.2: no `ParentPolicy`).

use std::fmt::Debug;

use context_trace::{
    positioned::{
        trace::state::parent::ParentBatch,
        ParentState,
        PositionedGraph,
    },
    AtomPosition,
    GraphKind,
    Token,
};

pub trait SearchKind: Debug {
    type Policy: DirectedTraversalPolicy;
}

pub trait DirectedTraversalPolicy: Sized + Debug {
    /// Parent states of a state at its root end: the state raised into
    /// every occurrence of its root. `None` if the root has no parents.
    fn next_batch<G: GraphKind>(
        graph: &PositionedGraph<G>,
        parent: &ParentState,
    ) -> Option<ParentBatch> {
        let batch = Self::gen_parent_batch(graph, parent.root(), |p, o| {
            let mut parent = parent.clone();
            parent.path_raise(p, o);
            crate::search::stats::record_raise(parent.path.len());
            parent
        });
        (!batch.is_empty()).then_some(batch)
    }
    /// One state per occurrence `(parent, offset)` of `index`, built by
    /// `build_parent`, in [`PositionedGraph::parent_occurrences`] order.
    fn gen_parent_batch<G: GraphKind>(
        graph: &PositionedGraph<G>,
        index: Token,
        build_parent: impl Fn(Token, AtomPosition) -> ParentState,
    ) -> ParentBatch {
        ParentBatch {
            parents: graph
                .parent_occurrences(index)
                .map(|(parent, offset)| build_parent(parent, offset))
                .collect(),
        }
    }
}
