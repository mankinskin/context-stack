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
    ChildCandidate(CompareState<Candidate, Candidate>),
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

impl GraphRoot for SearchNode {
    fn root_parent(&self) -> Token {
        match self {
            SearchNode::ParentCandidate(state) =>
                state.parent_state.path.root_parent(),
            SearchNode::ChildCandidate(state) =>
                state.child.current().child_state.root_parent(),
        }
    }
}
impl Ord for SearchNode {
    fn cmp(
        &self,
        other: &Self,
    ) -> Ordering {
        let self_width = self.root_parent().width.0;
        let other_width = other.root_parent().width.0;

        // Reverse ordering: smaller width values come first (min-heap behavior)
        // BinaryHeap is a max-heap by default, so we reverse the comparison
        // to get min-heap behavior (smallest widths popped first)
        let result = other_width.cmp(&self_width);

        // Debug output to verify ordering
        use tracing::trace;
        trace!(
            self_width,
            other_width,
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
        state: CompareState<Candidate, Candidate>,
    ) -> Option<NodeResult> {
        match state.compare_leaf_tokens(trav) {
            CompareResult::FoundMatch(matched_state) => {
                // Return the matched state directly without conversion
                // RootCursor will handle the conversion to Candidate with checkpoint update
                Some(NodeResult::FoundMatch(Box::new(matched_state)))
            },
            CompareResult::Mismatch(_) => Some(Skip),
            CompareResult::Prefixes(next) => {
                tracing::debug!(
                    num_prefixes = next.len(),
                    "got Prefixes, extending queue"
                );
                Some(QueueMore(next.into_iter().map(ChildCandidate).collect()))
            },
        }
    }
    fn consume(self) -> Option<NodeResult> {
        match self.0 {
            ParentCandidate(parent) => match parent.advance_state(&self.1) {
                Ok(state) => Self::compare_next(self.1, state.candidate),
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
            ChildCandidate(state) => Self::compare_next(self.1, state),
        }
    }
}
