//! Comparison inside one root (legacy `compare/iterator.rs`): decompose
//! until the first match or mismatch.

use context_trace::positioned::{
    trace::child::iterator::ChildQueue,
    GraphRead,
};

use crate::{
    compare::state::{
        CompareEndResult,
        CompareLeafResult,
        CompareState,
    },
    cursor::Candidate,
};

pub(crate) struct CompareIterator<'g, G: GraphRead + ?Sized> {
    graph: &'g G,
    queue: ChildQueue<CompareState<Candidate, Candidate, G::Index>>,
}

impl<'g, G: GraphRead + ?Sized> CompareIterator<'g, G> {
    pub(crate) fn new(
        graph: &'g G,
        queue: impl Into<ChildQueue<CompareState<Candidate, Candidate, G::Index>>>,
    ) -> Self {
        Self {
            graph,
            queue: queue.into(),
        }
    }
    pub(crate) fn compare(mut self) -> CompareEndResult<G::Index> {
        self.find_map(|flow| flow)
            .expect("a comparison ends in a match or a mismatch")
    }
}

impl<G: GraphRead + ?Sized> Iterator for CompareIterator<'_, G> {
    type Item = Option<CompareEndResult<G::Index>>;
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
