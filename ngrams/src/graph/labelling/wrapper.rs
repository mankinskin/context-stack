//! Label enclosing strings when their largest selected children overlap.

use std::{
    collections::VecDeque,
    ops::Range,
};

use derive_more::{
    Deref,
    DerefMut,
    From,
};
use derive_new::new;
use itertools::Itertools;
use range_ext::intersect::Intersect;
use tokio_util::sync::CancellationToken;

use crate::graph::{
    labelling::LabellingCtx,
    traversal::{
        direction::{
            BottomUp,
            TopDown,
            TraversalDirection,
        },
        pass::{
            RunResult,
            TraversalPass,
        },
        queue::{
            LayeredQueue,
            Queue,
        },
        visited::VisitTracking,
    },
    utils::cover::ChildCover,
    vocabulary::{
        entry::VertexCtx,
        NGramId,
        ProcessStatus,
        Vocabulary,
    },
    HasVertexEntries,
};
use context_trace::{
    graph::vertex::{
        has_vertex_index::HasVertexIndex,
        has_vertex_key::HasVertexKey,
        key::VertexKey,
        wide::Wide,
        VertexIndex,
    },
    HashSet,
};
/// Bottom-up wrapper selection, starting from atoms and advancing by width.
#[derive(Debug, Deref, new, DerefMut)]
pub(crate) struct WrapperCtx<'b> {
    /// Shared labels, including frequency selections and newly found wrappers.
    #[deref]
    #[deref_mut]
    ctx: &'b mut LabellingCtx,
    /// Reserved visit set; this pass currently does not consult it to skip nodes.
    #[new(default)]
    visited_mut: <Self as VisitTracking>::Collection,
}
impl VisitTracking for WrapperCtx<'_> {
    type Collection = HashSet<<Self as TraversalPass>::Node>;
    /// Expose the reserved set; the traversal does not automatically populate it.
    fn visited_mut(&mut self) -> &mut <Self as VisitTracking>::Collection {
        &mut self.visited_mut
    }
}
impl TraversalPass for WrapperCtx<'_> {
    type Node = VertexKey;
    type NextNode = VertexKey;
    type Queue = LayeredQueue<Self>;
    /// Expose shared labels and cancellation state to traversal helpers.
    fn ctx(&self) -> &LabellingCtx {
        self.ctx
    }
    /// Start at atoms so smaller wrappers can influence larger candidates.
    fn start_queue(&mut self) -> RunResult<Self::Queue> {
        self.status.next_pass(
            ProcessStatus::Wrappers,
            0,
            self.vocab().containment.vertex_count(),
        );
        Ok(BottomUp::starting_nodes(self.vocab())
            .into_iter()
            .map(|ng| ng.key)
            .collect())
    }
    /// Label an unselected node if its child cover contains overlapping spans.
    ///
    /// Parents are scheduled even when the node is already labelled. There is
    /// no visit guard here, so containment paths can schedule repeated work.
    fn on_node(
        &mut self,
        node: &Self::Node,
    ) -> RunResult<Option<Vec<Self::NextNode>>> {
        *self.status.steps_mut() += 1;
        let entry = self.vocab().get_vertex(node).unwrap();
        let next = BottomUp::next_nodes(&entry)
            .iter()
            .map(HasVertexKey::vertex_key)
            .collect();

        if !self.labels().contains(node) {
            let tree = ChildCover::from_key(self.ctx, entry.vertex_key());
            if tree.any_intersect() {
                let key = entry.data.vertex_key();
                // Later, wider candidates see this wrapper as a selected child.
                self.labels_mut().insert(key);
            }
        }

        Ok(Some(next))
    }
    /// Announce wrapper selection on stderr.
    fn begin_run(&mut self) {
        eprintln!("Wrapper Pass");
    }
}
