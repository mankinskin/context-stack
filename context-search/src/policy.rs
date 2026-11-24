//! Traversal policy of positioned search (legacy `policy.rs`): how parent
//! candidates are generated from parent links.
//!
//! Positioned parents are occurrences `(parent, offset)` from
//! [`Navigate::parent_occurrences`], in its deterministic order
//! (parent width, index, offset). Only the ancestor policy exists
//! (Decision 7.2: no `ParentPolicy`).

use std::fmt::Debug;

use context_trace::{
    positioned::{
        trace::state::parent::ParentBatch,
        GraphRead,
        Navigate,
        ParentState,
    },
    AtomPosition,
    Token,
};

pub trait SearchKind: Debug {
    type Policy: DirectedTraversalPolicy;
}

pub trait DirectedTraversalPolicy: Sized + Debug {
    /// Parent states of a state at its root end: the state raised into
    /// every occurrence of its root. `None` if the root has no parents.
    fn next_batch<G: GraphRead + ?Sized>(
        graph: &G,
        parent: &ParentState<G::Index>,
    ) -> Option<ParentBatch<G::Index>> {
        let batch = Self::gen_parent_batch(graph, parent.root(), |p, o| {
            let mut parent = parent.clone();
            parent.path_raise(p, o);
            crate::search::stats::record_raise(parent.path.len());
            parent
        });
        (!batch.is_empty()).then_some(batch)
    }
    /// One state per occurrence `(parent, offset)` of `index`, built by
    /// `build_parent`, in [`Navigate::parent_occurrences`] order. Empty for
    /// an unknown `index`.
    fn gen_parent_batch<G: GraphRead + ?Sized>(
        graph: &G,
        index: Token<G::Index>,
        build_parent: impl Fn(Token<G::Index>, AtomPosition) -> ParentState<G::Index>,
    ) -> ParentBatch<G::Index> {
        ParentBatch {
            parents: graph
                .vertex(index)
                .into_iter()
                .flat_map(|vertex| vertex.parent_occurrences())
                .map(|(parent, offset)| build_parent(parent, offset))
                .collect(),
        }
    }
}
