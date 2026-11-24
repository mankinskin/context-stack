//! Search queue and node processing (legacy `match/mod.rs`).
//!
//! **Decision 5 a.** The queue is a min-heap on the root width only, as in
//! legacy: the smallest context that could contain the match is popped
//! first. Equal widths are popped in insertion order (seeds in
//! [`Navigate::parent_occurrences`](context_trace::positioned::Navigate::parent_occurrences)
//! order); a `test-api` hook
//! ([`with_tie_order`]) selects an explicit ascending or descending
//! `(root index, root entry offset)` order instead, for the tie-order
//! independence test.

use std::{
    cmp::{
        Ordering,
        Reverse,
    },
    collections::BinaryHeap,
};

use context_trace::{
    positioned::GraphRead,
    AtomPosition,
    CommittedIndex,
    Token,
    VertexId,
};

use crate::{
    compare::{
        parent::ParentCompareState,
        state::{
            CompareEndResult,
            CompareLeafResult,
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

pub(crate) mod iterator;
pub(crate) mod root_cursor;

/// Order of queue entries with equal root width.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TieOrder {
    /// Insertion order (default).
    #[default]
    Insertion,
    /// Ascending `(root index, root entry offset)`.
    Ascending,
    /// Descending `(root index, root entry offset)`.
    Descending,
}

#[cfg(any(test, feature = "test-api"))]
thread_local! {
    static TIE_ORDER: std::cell::Cell<TieOrder> = const { std::cell::Cell::new(TieOrder::Insertion) };
}

/// Run `f` with searches on this thread using tie order `order`.
#[cfg(any(test, feature = "test-api"))]
pub fn with_tie_order<R>(
    order: TieOrder,
    f: impl FnOnce() -> R,
) -> R {
    let previous = TIE_ORDER.with(|cell| cell.replace(order));
    let result = f();
    TIE_ORDER.with(|cell| cell.set(previous));
    result
}

fn current_tie_order() -> TieOrder {
    #[cfg(any(test, feature = "test-api"))]
    {
        TIE_ORDER.with(|cell| cell.get())
    }
    #[cfg(not(any(test, feature = "test-api")))]
    {
        TieOrder::Insertion
    }
}

#[derive(Debug)]
pub(crate) enum SearchNode<I: VertexId = CommittedIndex> {
    ParentCandidate(ParentCompareState<I>),
    ChildCandidate(CompareState<Candidate, Candidate, I>),
}

impl<I: VertexId> SearchNode<I> {
    pub(crate) fn root_parent(&self) -> Token<I> {
        match self {
            Self::ParentCandidate(state) => state.parent_state.root(),
            Self::ChildCandidate(state) => state.child.current().child_state.root(),
        }
    }
    /// Offset of the root child the start path enters through.
    fn root_entry_offset(&self) -> AtomPosition {
        match self {
            Self::ParentCandidate(state) => state.parent_state.path.root_child_offset(),
            Self::ChildCandidate(state) => {
                state
                    .child
                    .current()
                    .child_state
                    .path
                    .start
                    .root_child_offset()
            },
        }
    }
}

/// Tie key of one queue; every entry of a queue uses the same variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum TieKey<I: VertexId> {
    Insertion,
    /// Popped smallest first.
    Ascending(Reverse<(I, AtomPosition)>),
    /// Popped largest first.
    Descending((I, AtomPosition)),
}

/// Heap key: smallest root width first, then the tie order, then
/// insertion order.
type QueueKey<I> = (Reverse<usize>, TieKey<I>, Reverse<usize>);

#[derive(Debug)]
struct QueueEntry<I: VertexId> {
    key: QueueKey<I>,
    node: SearchNode<I>,
}

impl<I: VertexId> PartialEq for QueueEntry<I> {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.key == other.key
    }
}
impl<I: VertexId> Eq for QueueEntry<I> {}
impl<I: VertexId> PartialOrd for QueueEntry<I> {
    fn partial_cmp(
        &self,
        other: &Self,
    ) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl<I: VertexId> Ord for QueueEntry<I> {
    fn cmp(
        &self,
        other: &Self,
    ) -> Ordering {
        self.key.cmp(&other.key)
    }
}

#[derive(Debug)]
pub(crate) struct SearchQueue<I: VertexId = CommittedIndex> {
    nodes: BinaryHeap<QueueEntry<I>>,
    pushed: usize,
    order: TieOrder,
}

impl<I: VertexId> SearchQueue<I> {
    pub(crate) fn new() -> Self {
        Self {
            nodes: BinaryHeap::new(),
            pushed: 0,
            order: current_tie_order(),
        }
    }
    pub(crate) fn push(
        &mut self,
        node: SearchNode<I>,
    ) {
        let root = node.root_parent();
        let tie = match self.order {
            TieOrder::Insertion => TieKey::Insertion,
            TieOrder::Ascending => {
                TieKey::Ascending(Reverse((root.index, node.root_entry_offset())))
            },
            TieOrder::Descending => {
                TieKey::Descending((root.index, node.root_entry_offset()))
            },
        };
        let key = (Reverse(root.width.0), tie, Reverse(self.pushed));
        self.pushed += 1;
        self.nodes.push(QueueEntry { key, node });
    }
    pub(crate) fn pop(&mut self) -> Option<SearchNode<I>> {
        self.nodes.pop().map(|entry| entry.node)
    }
    /// Root width of the next node.
    #[cfg(any(test, feature = "test-api"))]
    pub(crate) fn peek_width(&self) -> Option<usize> {
        self.nodes.peek().map(|entry| entry.key.0 .0)
    }
    pub(crate) fn clear(&mut self) {
        self.nodes.clear();
    }
}

impl<I: VertexId> Extend<SearchNode<I>> for SearchQueue<I> {
    fn extend<T: IntoIterator<Item = SearchNode<I>>>(
        &mut self,
        iter: T,
    ) {
        for node in iter {
            self.push(node);
        }
    }
}

#[derive(Debug)]
pub(crate) enum NodeResult<I: VertexId> {
    QueueMore(Vec<SearchNode<I>>),
    FoundMatch(MatchedCompareState<I>),
    Skip,
}

/// Processes one popped node.
pub(crate) struct NodeConsumer<'g, G: GraphRead + ?Sized, K: SearchKind> {
    node: SearchNode<G::Index>,
    graph: &'g G,
    _kind: std::marker::PhantomData<K>,
}

impl<'g, G: GraphRead + ?Sized, K: SearchKind> NodeConsumer<'g, G, K> {
    pub(crate) fn new(
        node: SearchNode<G::Index>,
        graph: &'g G,
    ) -> Self {
        Self {
            node,
            graph,
            _kind: std::marker::PhantomData,
        }
    }
    fn compare_next(
        graph: &G,
        state: CompareState<Candidate, Candidate, G::Index>,
    ) -> NodeResult<G::Index> {
        match state.compare_leaf_tokens(graph) {
            CompareLeafResult::Finished(CompareEndResult::FoundMatch(matched)) => {
                NodeResult::FoundMatch(matched)
            },
            CompareLeafResult::Finished(CompareEndResult::Mismatch) => NodeResult::Skip,
            CompareLeafResult::Prefixes(next) => {
                NodeResult::QueueMore(next.into_iter().map(SearchNode::ChildCandidate).collect())
            },
        }
    }
    /// A parent candidate enters its root at the next graph token, or is
    /// raised into its parents at the root end; a child candidate is
    /// compared.
    pub(crate) fn consume(self) -> NodeResult<G::Index> {
        match self.node {
            SearchNode::ParentCandidate(parent) => match parent.advance_state(self.graph) {
                Ok(state) => Self::compare_next(self.graph, state.candidate),
                Err(parent) => NodeResult::QueueMore(
                    K::Policy::next_batch(self.graph, &parent.parent_state)
                        .into_iter()
                        .flat_map(|batch| batch.parents)
                        .map(|parent_state| {
                            SearchNode::ParentCandidate(ParentCompareState {
                                parent_state,
                                cursor: parent.cursor.clone(),
                            })
                        })
                        .collect(),
                ),
            },
            SearchNode::ChildCandidate(state) => Self::compare_next(self.graph, state),
        }
    }
}
