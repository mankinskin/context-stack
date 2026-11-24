//! Staging implementation (b), the logical stage tag (staging plan §5): the
//! read writes into the committed graph itself, and every vertex carries its
//! [`VertexState`].
//!
//! - New composites are pushed as `Staged`; new atoms are committed at once
//!   (they are never extended, staging plan §2.2).
//! - Indices are [`StageIndex`]es whose tag bit mirrors the vertex state: a
//!   staged vertex in slot `i` is `StagedIndex(i)`, a committed one
//!   `CommittedIndex(i)`. The store keeps plain slots; the bit is derived on
//!   reading. Only the stage writes while it is open, so every composite at
//!   or after the stage's first slot is staged and every vertex there of
//!   width 1 is an atom (committed): the bit of a child or parent follows
//!   from its slot and width, without a lookup.
//! - [`StageHandle::commit`] flips the tags; its [`Translation`] only
//!   clears the bit.
//!
//! Other readers of the committed graph see staged vertices (one read at a
//! time); while a stage is open, only its views may edit the graph.
//!
//! Layout: stage id and tag helpers here, compositions
//! (`composition.rs`), vertex objects (`vertex.rs`), the store view
//! (`view.rs`) and the read's handle (`handle.rs`).
//!
//! [`StageIndex`]: crate::graph::vertex::StageIndex
//! [`StageHandle::commit`]: super::StageHandle::commit
//! [`Translation`]: super::Translation

mod composition;
mod handle;
mod vertex;
mod view;

#[cfg(debug_assertions)]
pub use self::view::TaggedLabels;
pub use self::{
    composition::TaggedComposition,
    handle::TaggedHandle,
    vertex::{
        TaggedVertex,
        TaggedVertexMut,
    },
    view::TaggedView,
};

use crate::{
    GraphKind,
    Token,
    graph::vertex::StageIndex,
    positioned::{
        committed::PositionedGraph,
        model::{
            Occurrence,
            PositionedVertex,
        },
        store::VertexState,
    },
};

/// The open tagged stage of a [`PositionedGraph`]: its first slot and the
/// graph's generation when it was opened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StageId {
    first: usize,
    generation: u64,
}

/// First slot of the open stage (no vertex is staged without one).
#[inline(always)]
fn first_slot<G: GraphKind>(graph: &PositionedGraph<G>) -> usize {
    graph.stage.map_or(usize::MAX, |stage| stage.first)
}

/// The stage index of the child of a stored occurrence.
#[inline(always)]
fn tag_child(
    first: usize,
    occurrence: &Occurrence,
) -> Occurrence<StageIndex> {
    let slot = occurrence.child.index.0;
    let staged = slot >= first && occurrence.child.width.0 > 1;
    Occurrence::new(
        occurrence.offset,
        Token::new(StageIndex::tagged(slot, staged), occurrence.child.width),
    )
}

/// The vertex in the slot of `index`. Its tag bit is only checked in debug
/// builds: the check delays every lookup on the vertex's state (about 15 %
/// of a periodic read), and the vertex object derives its index from the
/// state, not from `index`.
#[inline(always)]
fn lookup<G: GraphKind>(
    graph: &PositionedGraph<G>,
    index: StageIndex,
) -> Option<&PositionedVertex<G>> {
    let vertex = graph.vertices.get(index.slot());
    debug_assert!(
        vertex.is_none_or(|vertex| {
            (vertex.state == VertexState::Staged) == index.is_staged()
        }),
        "the tag bit of {index} disagrees with the vertex state"
    );
    vertex
}
