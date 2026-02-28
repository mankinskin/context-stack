use std::{
    cmp::Ordering,
    collections::BinaryHeap,
};

use crate::{
    compare::{
        iterator::CompareEvent,
        parent::ParentCompareState,
        state::{
            CompareEndResult,
            CompareLeafResult::*,
            CompareState,
            MatchedCompareState,
        },
    },
    cursor::Candidate,
    policy::{
        DirectedTraversalPolicy,
        SearchKind,
    },
};
use context_trace::{
    path::accessors::has_path::HasRootedPath,
    *,
};

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
    QueueMore(Vec<SearchNode>, Vec<CompareEvent>),
    FoundMatch(Box<MatchedCompareState>, Vec<CompareEvent>),
    Skip(Vec<CompareEvent>),
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
        other_width.cmp(&self_width)
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
        // Extract tokens being compared *before* consuming the state.
        let path_leaf =
            state.rooted_path().role_rooted_leaf_token::<End, _>(trav);
        let query_leaf =
            (*state.query.current()).role_rooted_leaf_token::<End, _>(trav);
        let cursor_pos = *state.query.current().atom_position.as_ref();

        match state.compare_leaf_tokens(trav) {
            Finished(CompareEndResult::FoundMatch(matched_state)) => {
                let event = CompareEvent::ChildMatch {
                    node: path_leaf.index.0,
                    cursor_pos,
                };
                Some(NodeResult::FoundMatch(Box::new(matched_state), vec![event]))
            },
            Finished(CompareEndResult::Mismatch(_)) => {
                let event = CompareEvent::ChildMismatch {
                    node: path_leaf.index.0,
                    cursor_pos,
                    expected: query_leaf.index.0,
                    actual: path_leaf.index.0,
                };
                Some(Skip(vec![event]))
            },
            Prefixes(next) => {
                tracing::debug!(
                    num_prefixes = next.len(),
                    "got Prefixes, extending queue"
                );
                let events: Vec<CompareEvent> = next
                    .iter()
                    .map(|prefix_state| {
                        let child_leaf = prefix_state
                            .rooted_path()
                            .role_rooted_leaf_token::<End, _>(trav);
                        CompareEvent::VisitChild {
                            parent: path_leaf.index.0,
                            child: child_leaf.index.0,
                            child_width: child_leaf.width.0,
                        }
                    })
                    .collect();
                Some(QueueMore(
                    next.into_iter().map(ChildCandidate).collect(),
                    events,
                ))
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
                    Vec::new(),
                )),
            },
            ChildCandidate(state) => Self::compare_next(self.1, state),
        }
    }
}
