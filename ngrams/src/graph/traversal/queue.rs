//! Queue policies controlling when newly discovered traversal nodes are visited.

use std::collections::VecDeque;

use itertools::Itertools;

use crate::graph::{
    vocabulary::{
        entry::VertexCtx,
        NGramId,
        Vocabulary,
    },
};
use context_trace::graph::vertex::{
    token::Token,
    wide::Wide,
    VertexIndex,
};

use crate::graph::traversal::pass::TraversalPass;
use derive_more::{
    Deref,
    DerefMut,
};

/// Scheduling operations required by the shared traversal driver.
pub(crate) trait Queue<P: TraversalPass>:
    FromIterator<P::NextNode>
{
    /// Schedule discovered nodes according to this queue's ordering policy.
    fn extend_layer(
        &mut self,
        iter: impl IntoIterator<Item = <P as TraversalPass>::NextNode>,
    );
    /// Make a deferred layer available, if this queue separates layers.
    fn finish_layer(&mut self);
    /// Remove the next currently available node.
    fn pop_front(&mut self) -> Option<P::NextNode>;
    /// Whether the active queue is empty; deferred layers are not included.
    fn is_empty(&self) -> bool;
}
/// Active FIFO storage for a pass's outgoing node type.
type NodeQueue<P> = VecDeque<<P as TraversalPass>::NextNode>;
/// Breadth-first queue that defers discoveries until the active layer drains.
#[derive(Debug, Deref, Default)]
pub(crate) struct LayeredQueue<P: TraversalPass> {
    /// Nodes available in the current layer.
    #[deref]
    queue: NodeQueue<P>,
    /// Nodes discovered while processing the current layer.
    layer: NodeQueue<P>,
}
impl<P: TraversalPass> FromIterator<P::NextNode> for LayeredQueue<P> {
    /// Treat supplied nodes as the active layer and leave the deferred layer empty.
    fn from_iter<T: IntoIterator<Item = P::NextNode>>(iter: T) -> Self {
        Self {
            queue: FromIterator::from_iter(iter),
            layer: Default::default(),
        }
    }
}
impl<P: TraversalPass> Queue<P> for LayeredQueue<P> {
    /// Append discoveries to the deferred layer, preserving their iteration order.
    fn extend_layer(
        &mut self,
        iter: impl IntoIterator<Item = <P as TraversalPass>::NextNode>,
    ) {
        self.layer.extend(iter)
    }
    /// Move all deferred nodes into the active queue.
    fn finish_layer(&mut self) {
        self.queue.extend(self.layer.drain(..))
    }
    /// Pop only from the active layer.
    fn pop_front(&mut self) -> Option<P::NextNode> {
        self.queue.pop_front()
    }
    /// Check only the active layer; the driver promotes deferred work after draining.
    fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

/// FIFO queue that appends discoveries immediately, without a layer barrier.
#[derive(Debug, Deref, Default)]
pub(crate) struct LinearQueue<P: TraversalPass> {
    /// Pending nodes in insertion order.
    queue: NodeQueue<P>,
}
impl<P: TraversalPass> FromIterator<P::NextNode> for LinearQueue<P> {
    /// Preserve the input iterator's order without sorting or deduplication.
    fn from_iter<T: IntoIterator<Item = P::NextNode>>(iter: T) -> Self {
        Self {
            queue: FromIterator::from_iter(iter),
        }
    }
}
impl<P: TraversalPass> Queue<P> for LinearQueue<P> {
    /// Append discoveries directly behind already-pending nodes.
    fn extend_layer(
        &mut self,
        iter: impl IntoIterator<Item = <P as TraversalPass>::NextNode>,
    ) {
        self.queue.extend(iter)
    }
    /// No deferred layer exists for this FIFO policy.
    fn finish_layer(&mut self) {}
    /// Remove the oldest pending node.
    fn pop_front(&mut self) -> Option<P::NextNode> {
        self.queue.pop_front()
    }
    /// Whether any FIFO work remains.
    fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}
