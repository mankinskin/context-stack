use crate::{
    compare::state::{
        CompareResult,
        CompareState,
    },
    cursor::Candidate,
    SearchKind,
};
use context_trace::*;

use std::fmt::Debug;

use crate::compare::state::CompareResult::*;

#[derive(Debug)]
pub(crate) struct CompareIterator<K: SearchKind> {
    pub(crate) children: ChildIterator<
        K,
        CompareState<Candidate, Candidate, PositionAnnotated<ChildLocation>>,
    >,
}

impl<K: SearchKind> CompareIterator<K> {
    pub(crate) fn new(
        trav: K::Trav,
        queue: impl Into<
            ChildQueue<
                CompareState<
                    Candidate,
                    Candidate,
                    PositionAnnotated<ChildLocation>,
                >,
            >,
        >,
    ) -> Self {
        Self {
            children: ChildIterator::<
                K,
                CompareState<
                    Candidate,
                    Candidate,
                    PositionAnnotated<ChildLocation>,
                >,
            >::new(trav, queue),
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
impl<T: SearchKind> Iterator for CompareIterator<T> {
    type Item = Option<CompareResult>;
    fn next(&mut self) -> Option<Self::Item> {
        tracing::trace!(
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
                    tracing::trace!(
                        result = ?result,
                        "got result (Match/Mismatch)"
                    );
                    Some(result)
                },
            }
        })
    }
}
