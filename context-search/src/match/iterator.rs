//! Search iterator (legacy `match/iterator.rs`): pops queue nodes until a
//! root matches.

use context_trace::{
    positioned::{
        PositionedGraph,
        TraceCache,
        TraceCtx,
    },
    GraphKind,
    Token,
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
pub(crate) enum BfsStepResult {
    Expanded,
    FoundMatch(MatchedCompareState),
    Skipped,
    Empty,
}

#[derive(Debug)]
pub(crate) struct SearchIterator<'g, G: GraphKind, K: SearchKind> {
    pub(crate) trace_ctx: TraceCtx<'g, G>,
    pub(crate) queue: SearchQueue,
    _kind: std::marker::PhantomData<K>,
}

impl<'g, G: GraphKind, K: SearchKind> SearchIterator<'g, G, K> {
    pub(crate) fn new(
        graph: &'g PositionedGraph<G>,
        start_index: Token,
        batch: CompareParentBatch,
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
    pub(crate) fn graph(&self) -> &'g PositionedGraph<G> {
        self.trace_ctx.graph
    }

    pub(crate) fn pop_and_process_one(&mut self) -> BfsStepResult {
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
    pub(crate) fn find_next_root_match(&mut self) -> Option<MatchedCompareState> {
        loop {
            match self.pop_and_process_one() {
                BfsStepResult::Expanded | BfsStepResult::Skipped => continue,
                BfsStepResult::FoundMatch(state) => return Some(state),
                BfsStepResult::Empty => return None,
            }
        }
    }
}
