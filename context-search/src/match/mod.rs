use std::{
    cmp::Ordering,
    collections::BinaryHeap,
};

use crate::{
    compare::{
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

/// Metadata about a child comparison (match, mismatch, or prefix expansion).
/// Carried on [`NodeResult`] variants so callers can emit visualization events.
#[derive(Debug, Clone)]
pub(crate) struct CompareInfo {
    /// The compared node index.
    pub node: usize,
    /// The compared node's width.
    pub node_width: usize,
    /// Query cursor position at comparison time.
    pub cursor_pos: usize,
    /// The outcome of the comparison.
    pub outcome: CompareOutcome,
}

#[derive(Debug, Clone)]
pub(crate) enum CompareOutcome {
    /// Leaf token matched.
    Match,
    /// Leaf token mismatched.
    Mismatch { expected: usize, actual: usize },
    /// Node decomposed into prefix children.
    Prefixes(Vec<PrefixChildInfo>),
}

#[derive(Debug, Clone)]
pub(crate) struct PrefixChildInfo {
    pub child: usize,
    pub child_width: usize,
}

#[derive(Debug)]
pub(crate) enum NodeResult {
    QueueMore(Vec<SearchNode>, CompareInfo),
    FoundMatch(MatchedCompareState, CompareInfo),
    Skip(CompareInfo),
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
        // Extract the leaf tokens being compared *before* consuming the state.
        let path_leaf =
            state.rooted_path().role_rooted_leaf_token::<End, _>(trav);
        let query_leaf =
            (*state.query.current()).role_rooted_leaf_token::<End, _>(trav);
        let cursor_pos = *state.query.current().atom_position.as_ref();
        let node = path_leaf.index.0;
        let node_width = path_leaf.width.0;

        match state.compare_leaf_tokens(trav) {
            Finished(CompareEndResult::FoundMatch(matched_state)) => {
                let info = CompareInfo {
                    node, node_width, cursor_pos,
                    outcome: CompareOutcome::Match,
                };
                Some(NodeResult::FoundMatch(matched_state, info))
            },
            Finished(CompareEndResult::Mismatch(_)) => {
                let info = CompareInfo {
                    node, node_width, cursor_pos,
                    outcome: CompareOutcome::Mismatch {
                        expected: query_leaf.index.0,
                        actual: path_leaf.index.0,
                    },
                };
                Some(Skip(info))
            },
            Prefixes(next) => {
                tracing::debug!(
                    num_prefixes = next.len(),
                    "got Prefixes, extending queue"
                );
                let prefix_children: Vec<PrefixChildInfo> = next
                    .iter()
                    .map(|prefix_state| {
                        let child_leaf = prefix_state
                            .rooted_path()
                            .role_rooted_leaf_token::<End, _>(trav);
                        PrefixChildInfo {
                            child: child_leaf.index.0,
                            child_width: child_leaf.width.0,
                        }
                    })
                    .collect();
                let info = CompareInfo {
                    node, node_width, cursor_pos,
                    outcome: CompareOutcome::Prefixes(prefix_children),
                };
                Some(QueueMore(
                    next.into_iter().map(ChildCandidate).collect(),
                    info,
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
                    // Parent expansion has no leaf comparison — use a dummy CompareInfo.
                    CompareInfo {
                        node: parent.parent_state.path.root_parent().index.0,
                        node_width: parent.parent_state.path.root_parent().width.0,
                        cursor_pos: *parent.cursor.candidate().atom_position.as_ref(),
                        outcome: CompareOutcome::Prefixes(vec![]),
                    },
                )),
            },
            ChildCandidate(state) => Self::compare_next(self.1, state),
        }
    }
}
