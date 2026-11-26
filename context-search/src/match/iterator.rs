//! Search iterator (legacy `match/iterator.rs`): pops queue nodes until a
//! root matches.

use crate::{
    TraceCache,
    TraceCtx,
};
use context_trace::{
    positioned::GraphRead,
    Token,
    VertexId,
};

use crate::{
    compare::state::MatchedCompareState,
    policy::SearchKind,
    r#match::{
        root_cursor::CompareParentBatch,
        NodeConsumer,
        NodeResult,
        SearchNode,
        SearchQueue,
    },
};

#[derive(Debug)]
pub(crate) enum BfsStepResult<I: VertexId> {
    Expanded,
    FoundMatch(MatchedCompareState<I>),
    Skipped,
    Empty,
}

#[derive(Debug)]
pub(crate) struct SearchIterator<'g, G: GraphRead + ?Sized, K: SearchKind> {
    pub(crate) trace_ctx: TraceCtx<'g, G>,
    pub(crate) queue: SearchQueue<G::Index>,
    _kind: std::marker::PhantomData<K>,
}

impl<'g, G: GraphRead + ?Sized, K: SearchKind> SearchIterator<'g, G, K> {
    pub(crate) fn new(
        graph: &'g G,
        start_index: Token<G::Index>,
        batch: CompareParentBatch<G::Index>,
    ) -> Self {
        let mut queue = SearchQueue::new();
        queue.extend(
            batch
                .into_compare_batch()
                .into_iter()
                .map(SearchNode::ParentCandidate),
        );
        Self {
            trace_ctx: TraceCtx::new(graph, TraceCache::new(start_index)),
            queue,
            _kind: std::marker::PhantomData,
        }
    }
    pub(crate) fn graph(&self) -> &'g G {
        self.trace_ctx.graph
    }

    pub(crate) fn pop_and_process_one(&mut self) -> BfsStepResult<G::Index> {
        let Some(node) = self.queue.pop() else {
            return BfsStepResult::Empty;
        };
        match NodeConsumer::<G, K>::new(node, self.graph()).consume() {
            NodeResult::QueueMore(next) => {
                self.queue.extend(next);
                BfsStepResult::Expanded
            },
            NodeResult::FoundMatch(state) => {
                #[cfg(any(test, feature = "test-api"))]
                self.probe_equal_width(state.child.current().child_state.root().width.0);
                BfsStepResult::FoundMatch(state)
            },
            NodeResult::Skip => BfsStepResult::Skipped,
        }
    }

    /// Uniqueness counter: drain the queued candidates with the width of
    /// the root just matched (the caller clears the queue anyway) and count
    /// those that also reach a match at their first comparison.
    #[cfg(any(test, feature = "test-api"))]
    fn probe_equal_width(
        &mut self,
        width: usize,
    ) {
        let graph = self.graph();
        crate::search::stats::probing(|| {
            while self.queue.peek_width() == Some(width) {
                crate::search::stats::record_equal_width_candidate();
                let mut pending = vec![self.queue.pop().expect("peeked")];
                while let Some(node) = pending.pop() {
                    match NodeConsumer::<G, K>::new(node, graph).consume() {
                        NodeResult::FoundMatch(_) => {
                            crate::search::stats::record_equal_width_first_match()
                        },
                        // Decompositions of the same first comparison;
                        // raised parents are wider roots.
                        NodeResult::QueueMore(next) => pending.extend(
                            next.into_iter()
                                .filter(|node| matches!(node, SearchNode::ChildCandidate(_))),
                        ),
                        NodeResult::Skip => {},
                    }
                }
            }
        });
    }

    /// Pop nodes until a root matches; `None` if the queue runs empty.
    pub(crate) fn find_next_root_match(&mut self) -> Option<MatchedCompareState<G::Index>> {
        loop {
            match self.pop_and_process_one() {
                BfsStepResult::Expanded | BfsStepResult::Skipped => continue,
                BfsStepResult::FoundMatch(state) => return Some(state),
                BfsStepResult::Empty => return None,
            }
        }
    }
}
