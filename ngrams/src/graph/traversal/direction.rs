//! Resolve containment neighbours and initial nodes for either traversal direction.

use std::collections::VecDeque;

use itertools::Itertools;

use crate::graph::vocabulary::{
    entry::VertexCtx,
    NGramId,
    Vocabulary,
};
use context_trace::graph::vertex::{
    token::Token,
    wide::Wide,
    VertexIndex,
};

/// Traverse from atoms to containing strings one character wider.
pub(crate) struct BottomUp;
/// Traverse from corpus roots to positioned, one-character-shorter substrings.
pub(crate) struct TopDown;

/// Direction-specific starting nodes, neighbours, and pair orientation.
pub(crate) trait TraversalDirection {
    /// Neighbour handle, optionally carrying a relative offset.
    type Next;
    /// Collect roots or leaves in their current hash-set iteration order.
    fn starting_nodes(vocab: &Vocabulary) -> VecDeque<NGramId>;
    /// Return adjacent containment nodes in this direction.
    fn next_nodes(entry: &VertexCtx<'_>) -> Vec<Self::Next>;
    /// Arrange a traversal edge as `(larger, smaller)`.
    fn order_top_bottom<T>(
        prev: T,
        next: T,
    ) -> (T, T);
}
impl TraversalDirection for BottomUp {
    type Next = NGramId;
    /// Begin from every atom, without sorting the leaf set.
    fn starting_nodes(vocab: &Vocabulary) -> VecDeque<NGramId> {
        FromIterator::from_iter(vocab.leaves.iter().cloned())
    }
    /// Reverse an upward traversal edge to put the parent first.
    fn order_top_bottom<T>(
        prev: T,
        next: T,
    ) -> (T, T) {
        (next, prev)
    }
    /// Keep only parents exactly one character wider than this entry.
    fn next_nodes(entry: &VertexCtx<'_>) -> Vec<Self::Next> {
        // Elementary patterns also link atoms to wider parents; skip those jumps.
        entry
            .data
            .parents()
            .iter()
            .filter(|(&id, p)| p.width() == entry.data.width() + 1)
            .map(|(id, p)| {
                NGramId::new(
                    entry.vocab.containment.expect_key_for_index(id),
                    p.width().0,
                )
            })
            .collect_vec()
    }
}
impl TraversalDirection for TopDown {
    type Next = (usize, NGramId); // (off, id)
    /// Begin from the root set, without preserving input or lexical order.
    fn starting_nodes(vocab: &Vocabulary) -> VecDeque<NGramId> {
        FromIterator::from_iter(vocab.roots.iter().cloned())
    }
    /// Preserve a downward traversal edge, which already has its parent first.
    fn order_top_bottom<T>(
        prev: T,
        next: T,
    ) -> (T, T) {
        (prev, next)
    }
    /// Return positioned children exactly one character shorter than this entry.
    fn next_nodes(entry: &VertexCtx<'_>) -> Vec<Self::Next> {
        entry
            .data
            .top_down_containment_nodes()
            .into_iter()
            .map(|(subi, c)| {
                (
                    // sub index can be used as offset because child patterns have special structure
                    subi,
                    NGramId::new(
                        entry.vocab.containment.expect_key_for_index(c),
                        c.width().0,
                    ),
                )
            })
            .collect()
    }
}
