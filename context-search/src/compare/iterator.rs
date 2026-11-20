//! Comparison inside one root (legacy `compare/iterator.rs`): decompose
//! until the first match or mismatch.

use context_trace::{
    positioned::{
        trace::child::iterator::ChildQueue,
        PositionedGraph,
    },
    GraphKind,
};

use crate::{
    compare::state::{
        CompareEndResult,
        CompareLeafResult,
        CompareState,
    },
    cursor::Candidate,
};

#[derive(Debug)]
pub(crate) struct CompareIterator<'g, G: GraphKind> {
    graph: &'g PositionedGraph<G>,
    queue: ChildQueue<CompareState<Candidate, Candidate>>,
}

impl<'g, G: GraphKind> CompareIterator<'g, G> {
    pub(crate) fn new(
        graph: &'g PositionedGraph<G>,
        queue: impl Into<ChildQueue<CompareState<Candidate, Candidate>>>,
    ) -> Self {
        Self {
            graph,
            queue: queue.into(),
        }
    }
    pub(crate) fn compare(mut self) -> CompareEndResult {
        self.find_map(|flow| flow)
            .expect("a comparison ends in a match or a mismatch")
    }
}

impl<G: GraphKind> Iterator for CompareIterator<'_, G> {
    type Item = Option<CompareEndResult>;
    fn next(&mut self) -> Option<Self::Item> {
        let state = self.queue.pop_front()?;
        Some(match state.compare_leaf_tokens(self.graph) {
            CompareLeafResult::Prefixes(next) => {
                self.queue.extend(next);
                None
            },
            CompareLeafResult::Finished(result) => Some(result),
        })
    }
}
