//! Positioned labelled-child covers and occurrence-based ancestor covers.

pub(crate) mod child;
pub(crate) mod frequency;
pub(crate) mod parent;

use child::ChildCoverPass;
use context_trace::legacy::{
    graph::{
        vertex::{
            data::{
                VertexData,
                VertexDataBuilder,
            },
            has_vertex_index::{
                HasVertexIndex,
                ToToken,
            },
            key::VertexKey,
            wide::Wide,
            VertexIndex,
        },
        Hypergraph,
    },
    HashMap,
    HashSet,
    VertexSet,
};
use derivative::Derivative;
use itertools::Itertools;
use pretty_assertions::assert_matches;
use range_ext::intersect::Intersect;
use std::{
    cmp::{
        Ordering,
        Reverse,
    },
    collections::VecDeque,
    fmt::{
        Display,
        Formatter,
    },
    num::NonZeroUsize,
    ops::Range,
};

use derive_more::{
    Deref,
    DerefMut,
    IntoIterator,
};
use derive_new::new;

use crate::graph::{
    labelling::LabellingCtx,
    partitions::{
        NodePartitionCtx,
        PartitionsCtx,
    },
    traversal::{
        direction::{
            TopDown,
            TraversalDirection,
        },
        pass::TraversalPass,
    },
    vocabulary::{
        entry::{
            HasVertexEntries,
            VertexCtx,
            VocabEntry,
        },
        NGramId,
        ProcessStatus,
        Vocabulary,
    },
};

/// Selected child spans relative to a containing string.
///
/// Partially overlapping spans are allowed. Each start offset holds at most
/// one handle; this is not an enumeration of all possible segmentations.
#[derive(Debug, Deref, DerefMut, Default, IntoIterator, new)]
pub(crate) struct ChildCover {
    /// Character offset to selected child; inserting at an offset replaces it.
    #[into_iterator(owned, ref)]
    #[deref]
    #[deref_mut]
    entries: HashMap<usize, NGramId>,
}
impl ChildCover {
    /// Search downward for the largest currently-labelled proper children.
    ///
    /// The traversal result is currently discarded, so cancellation can leave
    /// a partial cover rather than being propagated to this caller.
    pub(crate) fn from_key(
        ctx: &LabellingCtx,
        key: VertexKey,
    ) -> Self {
        let mut ctx = ChildCoverPass::new(ctx, key);
        ctx.run();
        ctx.cover
    }
    /// Convert selected children to half-open character intervals.
    pub(crate) fn as_ranges(&self) -> HashSet<Range<usize>> {
        self.entries
            .iter()
            .map(|(off, id)| *off..(off + id.width().0))
            .collect()
    }
    /// Whether two distinct selected intervals overlap.
    pub(crate) fn any_intersect(&self) -> bool {
        let ranges = self.as_ranges();
        ranges
            .iter()
            .cartesian_product(&ranges)
            .any(|(l, r)| l != r && l.does_intersect(r))
    }
    /// Whether one selected interval wholly contains the proposed child span.
    pub(crate) fn any_covers(
        &self,
        off: usize,
        node: impl Wide,
    ) -> bool {
        self.iter().any(|(&p, &c)| {
            let node_end = off + node.width();
            let probe_end = p + c.width();
            p <= off && node_end <= probe_end
        })
    }
}
