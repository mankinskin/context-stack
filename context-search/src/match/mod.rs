use std::{
    cmp::Ordering,
    collections::BinaryHeap,
};

use crate::{
    compare::{
        iterator::CompareIterator,
        parent::ParentCompareState,
        state::{
            CompareResult,
            CompareState,
            MatchedCompareState,
        },
    },
    cursor::Candidate,
    r#match::root_cursor::RootCursor,
    traversal::{
        policy::DirectedTraversalPolicy,
        SearchKind,
    },
};
use context_trace::*;

use derive_new::new;
pub(crate) mod iterator;
pub(crate) mod root_cursor;

#[derive(Debug, new, Default)]
pub(crate) struct SearchQueue {
    #[new(default)]
    pub(crate) nodes: BinaryHeap<SearchNode>,
}

// Display implementation moved to logging/mod.rs via impl_display_via_compact macro

#[derive(Debug)]
pub(crate) enum NodeResult {
    QueueMore(Vec<SearchNode>),
    FoundMatch(Box<MatchedCompareState>),
    Skip,
}
use NodeResult::*;

#[derive(Debug)]
pub(crate) enum SearchNode {
    ParentCandidate(ParentCompareState),
    PrefixQueue(ChildQueue<CompareState<Candidate, Candidate>>),
}
use SearchNode::*;

// Implement ordering for SearchNode to use in priority queue
// Smaller parent tokens are prioritized (processed first)
impl PartialEq for SearchNode {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for SearchNode {}

impl PartialOrd for SearchNode {
    fn partial_cmp(
        &self,
        other: &Self,
    ) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SearchNode {
    fn cmp(
        &self,
        other: &Self,
    ) -> Ordering {
        let self_priority = match self {
            SearchNode::ParentCandidate(state) => {
                // Smaller tokens have higher priority (lower value)
                let token = state.parent_state.path.root_parent();
                token.width.0
            },
            SearchNode::PrefixQueue(_) => {
                // PrefixQueues should be processed after all parent candidates
                usize::MAX
            },
        };

        let other_priority = match other {
            SearchNode::ParentCandidate(state) => {
                let token = state.parent_state.path.root_parent();
                token.width.0
            },
            SearchNode::PrefixQueue(_) => usize::MAX,
        };

        // Reverse ordering: smaller priority values come first (min-heap behavior)
        // BinaryHeap is a max-heap by default, so we reverse the comparison
        // to get min-heap behavior (smallest widths popped first)
        let result = other_priority.cmp(&self_priority);

        // Debug output to verify ordering
        use tracing::trace;
        trace!(
            self_width = self_priority,
            other_width = other_priority,
            ordering = ?result,
            "SearchNode comparison for heap ordering"
        );

        result
    }
}

#[derive(Debug, new)]
struct NodeConsumer<'a, K: SearchKind>(SearchNode, &'a K::Trav);

impl<K: SearchKind> NodeConsumer<'_, K>
where
    K::Trav: Clone,
{
    fn compare_next(
        trav: &K::Trav,
        queue: ChildQueue<CompareState<Candidate, Candidate>>,
    ) -> Option<NodeResult> {
        let mut compare_iter = CompareIterator::<K>::new(trav.clone(), queue);
        match compare_iter.next() {
            Some(Some(CompareResult::FoundMatch(matched_state))) => {
                // Return the matched state directly without conversion
                // RootCursor will handle the conversion to Candidate with checkpoint update
                Some(NodeResult::FoundMatch(Box::new(matched_state)))
            },
            Some(Some(CompareResult::Mismatch(_))) => Some(Skip),
            Some(Some(CompareResult::Prefixes(_))) => {
                unreachable!("compare_iter.next() should never return Prefixes - they are consumed by the iterator")
            },
            Some(None) =>
                Some(QueueMore(vec![PrefixQueue(compare_iter.children.queue)])),
            None => None,
        }
    }
    fn consume(self) -> Option<NodeResult> {
        match self.0 {
            ParentCandidate(parent) => match parent.advance_state(&self.1) {
                Ok(state) => Self::compare_next(
                    self.1,
                    ChildQueue::from_iter([state.token]),
                ),
                Err(parent) => Some(QueueMore(
                    K::Policy::next_batch(self.1, &parent)
                        .into_iter()
                        .flat_map(|batch| batch.parents)
                        .map(|parent_state| ParentCompareState {
                            parent_state,
                            cursor: parent.cursor.clone(),
                        })
                        .map(ParentCandidate)
                        .collect(),
                )),
            },
            PrefixQueue(queue) => Self::compare_next(self.1, queue),
        }
    }
}
