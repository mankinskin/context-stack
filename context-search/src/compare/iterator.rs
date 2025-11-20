use crate::{
    compare::state::{
        CompareResult,
        CompareState,
    },
    cursor::Candidate,
};
use context_trace::*;

use std::fmt::Debug;

use crate::compare::state::CompareResult::*;

#[derive(Debug)]
pub(crate) struct CompareIterator<G: HasGraph> {
    pub(crate) children: ChildIterator<G, CompareState<Candidate, Candidate>>,
}

impl<G: HasGraph> CompareIterator<G> {
    pub(crate) fn new(
        trav: G,
        queue: impl Into<ChildQueue<CompareState<Candidate, Candidate>>>,
    ) -> Self {
        Self {
            children: ChildIterator::new(trav, queue),
        }
    }
    //pub(crate) fn find_match(self) -> Option<CompareState<Matched, Matched>> {
    //    match self.compare() {
    //        Mismatch(_) => None,
    //        FoundMatch(state) => Some(state),
    //        Prefixes(_) =>
    //            unreachable!("compare() always returns Match or Mismatch"),
    //    }
    //}
    pub(crate) fn compare(mut self) -> CompareResult {
        self.find_map(|flow| flow).unwrap()
    }
}
impl<G: HasGraph> Iterator for CompareIterator<G> {
    type Item = Option<CompareResult>;
    fn next(&mut self) -> Option<Self::Item> {
        tracing::debug!(
            queue_len = self.children.queue.len(),
            "processing next state"
        );
        self.children.next().map(|cs| {
            match cs.compare_leaf_tokens(&self.children.trav) {
                Prefixes(next) => {
                    tracing::debug!(
                        num_prefixes = next.len(),
                        "got Prefixes, extending queue"
                    );
                    self.children.queue.extend(next);
                    None
                },
                result => {
                    tracing::debug!(
                        result = %result,
                        "got result (Match/Mismatch)"
                    );
                    Some(result)
                },
            }
        })
    }
}
