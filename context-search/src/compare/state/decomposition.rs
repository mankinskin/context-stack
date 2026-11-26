//! Leaf comparison with one prefix chain (legacy
//! `compare/state/decomposition.rs`, Decision 1 b).
//!
//! A decomposition replaces the wider leaf by its prefix child
//! ([`Navigate::prefix_child`](context_trace::positioned::Navigate::prefix_child));
//! equal widths replace both leaves in one step (`BothMajor`). Each
//! decomposition yields exactly one state.

use std::cmp::Ordering;

use crate::trace::child::ChildQueue;
use context_trace::{
    positioned::GraphRead,
    VertexId,
};

use super::core::{
    CompareEndResult,
    CompareLeafResult,
    CompareState,
    PathPairMode,
};
use crate::{
    cursor::{
        Candidate,
        MarkMatchState,
    },
    search::stats,
};

impl<X: VertexId> CompareState<Candidate, Candidate, X> {
    /// Compare the graph leaf `g` with the query leaf `q`: equal → match;
    /// two different atoms → mismatch; otherwise decompose the wider side
    /// (both on equal width).
    pub(crate) fn compare_leaf_tokens<G: GraphRead<Index = X> + ?Sized>(
        mut self,
        graph: &G,
    ) -> CompareLeafResult<X> {
        let g = self.child.current().child_state.path.end.leaf();
        let q = self.query.current().path.leaf();
        stats::record_compare(self.query.current().path.leaf_offset());
        if g.index == q.index {
            return CompareLeafResult::Finished(CompareEndResult::FoundMatch(self.mark_match()));
        }
        let mode = match g.width.cmp(&q.width) {
            Ordering::Equal if g.width.0 == 1 => {
                return CompareLeafResult::Finished(CompareEndResult::Mismatch);
            },
            Ordering::Equal => PathPairMode::BothMajor,
            Ordering::Greater => PathPairMode::GraphMajor,
            Ordering::Less => PathPairMode::QueryMajor,
        };
        self.mode = mode;
        self.expand_to_prefix_comparisons(graph);
        CompareLeafResult::Prefixes(ChildQueue::from_iter([self]))
    }

    /// Replace the leaf of the side(s) selected by `mode` by its prefix
    /// child.
    pub(crate) fn expand_to_prefix_comparisons<G: GraphRead<Index = X> + ?Sized>(
        &mut self,
        graph: &G,
    ) {
        if matches!(
            self.mode,
            PathPairMode::GraphMajor | PathPairMode::BothMajor
        ) {
            let split = self
                .child
                .current_mut()
                .child_state
                .path
                .end
                .push_border_child(graph);
            debug_assert!(split, "wider graph leaf is composite");
            stats::record_graph_prefix_descent(
                self.child.current().child_state.path.end.len(),
            );
        }
        if matches!(
            self.mode,
            PathPairMode::QueryMajor | PathPairMode::BothMajor
        ) {
            let split = self.query.current_mut().push_border_child(graph);
            debug_assert!(split, "wider query leaf is composite");
            stats::record_query_prefix_descent();
        }
        self.target = self.current_target();
    }
}
