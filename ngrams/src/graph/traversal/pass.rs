//! Shared sequential traversal lifecycle and cancellation result types.

use std::{
    collections::VecDeque,
    fmt::Debug,
    hash::Hash,
};

use itertools::Itertools;
use tokio_util::sync::CancellationToken;

use crate::graph::{
    labelling::LabellingCtx,
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

use super::{
    queue::Queue,
    visited::{
        VisitTracking,
        VisitorCollection,
    },
};
/// Identity and diagnostic requirements for nodes carried through a pass.
pub(crate) trait PassNode:
    Eq + PartialEq + Debug + Clone + Hash
{
}
impl<N: Eq + PartialEq + Debug + Clone + Hash> PassNode for N {}
/// Reasons a pass may stop without returning its normal result.
#[derive(Debug)]
pub enum CancelReason {
    /// The caller's cancellation handle was signalled.
    Cancelled,
    /// A pass reported a generic processing error.
    Error,
    /// No usable vocabulary could be constructed.
    EmptyVocabulary,
}
/// Fallible result propagated by traversal and corpus construction.
#[must_use]
pub(crate) type RunResult<T> = Result<T, CancelReason>;
/// A pass whose node processing can discover additional work.
pub(crate) trait TraversalPass: Sized {
    /// Identity presented to the condition and processing hooks.
    type Node: PassNode + Copy;
    /// Queued discovery type, convertible into a processing identity.
    type NextNode: PassNode + Into<Self::Node>;
    /// Scheduling policy for this pass.
    type Queue: Queue<Self>;
    /// Initialize pass state and seed its queue.
    fn start_queue(&mut self) -> RunResult<Self::Queue>;
    /// Process one eligible node and optionally return further work.
    fn on_node(
        &mut self,
        node: &Self::Node,
    ) -> RunResult<Option<Vec<Self::NextNode>>>;
    /// Shared vocabulary, labels, status, and cancellation context.
    fn ctx(&self) -> &LabellingCtx;
    /// Decide whether to process a popped node; may also mark it visited.
    fn node_condition(
        &mut self,
        node: Self::Node,
    ) -> bool {
        true
    }
    /// Optional hook called before queue initialization.
    fn begin_run(&mut self) {}
    /// Optional hook called only after the queue completes successfully.
    fn finish_run(&mut self) -> RunResult<()> {
        Ok(())
    }
    /// Drain the queue, checking cancellation before each node condition.
    ///
    /// A layer barrier is applied after the active queue drains. Errors return
    /// immediately without calling `finish_run` or rolling back mutations.
    fn run(&mut self) -> RunResult<()> {
        self.begin_run();
        let mut queue = self.start_queue()?;

        while !queue.is_empty() {
            while let Some(node) = queue.pop_front() {
                self.ctx().check_cancelled()?;
                let node = node.into();
                if self.node_condition(node) {
                    if let Some(next) = self.on_node(&node)? {
                        queue.extend_layer(next);
                    }
                }
            }
            queue.finish_layer()
        }
        self.finish_run()
    }
}
