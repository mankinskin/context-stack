//! Positioned trace cache (legacy `trace::cache`, Decision 4 a).
//!
//! Same shape as legacy: vertex → [`VertexCache`] `{ bottom_up, top_down }`
//! → query position → [`PositionCache`] `{ top, bottom }`, keyed by
//! [`DirectedKey`] with query positions. Edges are recorded at the parent and
//! labelled by the child's offset in the parent, so consumers read offsets
//! without a composition lookup. The root offset of the match is not stored
//! here but once in [`RangePath::span`](crate::positioned::path::RangePath).
//!
//! Also holds the key accessors of trace states ([`RootKey`],
//! [`TargetKey`]; legacy `cache::key::props`, the keys themselves are the
//! model-agnostic [`crate::key`] types) and the new trace edges
//! ([`EditKind`], [`NewTraceEdge`]; legacy `cache::new`), labelled by the
//! child's offset in its parent instead of a row location.

pub mod position;
pub mod vertex;

use derive_more::{
    From,
    derive::IntoIterator,
};
use derive_new::new;

use crate::{
    AtomPosition,
    DirectedKey,
    HashMap,
    Token,
    UpKey,
    graph::vertex::{
        CommittedIndex,
        VertexId,
        has_vertex_index::HasVertexIndex,
    },
    positioned::trace::{
        BottomUp,
        TopDown,
        TraceDirection,
    },
};
use position::PositionCache;
use vertex::VertexCache;

#[derive(Clone, Debug, PartialEq, Eq, IntoIterator)]
pub struct TraceCache<I: VertexId = CommittedIndex> {
    pub entries: HashMap<I, VertexCache<I>>,
}

impl<I: VertexId> Default for TraceCache<I> {
    fn default() -> Self {
        Self {
            entries: HashMap::default(),
        }
    }
}

impl<I: VertexId> TraceCache<I> {
    /// A cache holding only the (position-less) entry of the start token.
    pub fn new(start_index: Token<I>) -> Self {
        let mut entries = HashMap::default();
        entries.insert(start_index.vertex_index(), VertexCache::start(start_index));
        Self { entries }
    }
    /// The entry of `key`, created empty if missing.
    pub fn force_mut(
        &mut self,
        key: &DirectedKey<I>,
    ) -> &mut PositionCache<I> {
        self.entries
            .entry(key.index.vertex_index())
            .or_insert_with(|| VertexCache::from(key.index))
            .dir_mut(&key.pos)
            .entry(*crate::HasAtomPosition::pos(&key.pos))
            .or_default()
    }
    pub fn get_vertex(
        &self,
        index: impl HasVertexIndex<Index = I>,
    ) -> Option<&VertexCache<I>> {
        self.entries.get(&index.vertex_index())
    }
    pub fn get_vertex_mut(
        &mut self,
        index: impl HasVertexIndex<Index = I>,
    ) -> Option<&mut VertexCache<I>> {
        self.entries.get_mut(&index.vertex_index())
    }
    pub fn get(
        &self,
        key: &DirectedKey<I>,
    ) -> Option<&PositionCache<I>> {
        self.get_vertex(key.index).and_then(|ve| ve.get(&key.pos))
    }
    pub fn get_mut(
        &mut self,
        key: &DirectedKey<I>,
    ) -> Option<&mut PositionCache<I>> {
        self.get_vertex_mut(key.index).and_then(|ve| ve.get_mut(&key.pos))
    }
    pub fn exists_vertex(
        &self,
        index: impl HasVertexIndex<Index = I>,
    ) -> bool {
        self.entries.contains_key(&index.vertex_index())
    }
    pub fn exists(
        &self,
        key: &DirectedKey<I>,
    ) -> bool {
        self.get(key).is_some()
    }
    /// Every recorded edge as `(parent key, child key, child offset)`.
    pub fn edges(&self) -> impl Iterator<Item = (DirectedKey<I>, &DirectedKey<I>, AtomPosition)> {
        self.entries.values().flat_map(|ve| {
            let index = ve.index;
            let up = ve.bottom_up.iter().map(move |(pos, pc)| {
                (DirectedKey::up(index, *pos), pc)
            });
            let down = ve.top_down.iter().map(move |(pos, pc)| {
                (DirectedKey::down(index, *pos), pc)
            });
            up.chain(down).flat_map(|(parent, pc)| {
                pc.bottom
                    .iter()
                    .map(move |(child, offset)| (parent.clone(), child, *offset))
            })
        })
    }
}

impl<I: VertexId> TraceCache<I> {
    /// Add the target key of `edit` and record its edge at the parent.
    /// Returns the target key and whether it was new. Unlike legacy, the
    /// edge is recorded even if the target key existed.
    pub fn add_state<E: Into<EditKind<I>>>(
        &mut self,
        edit: E,
    ) -> (DirectedKey<I>, bool) {
        let edit = edit.into();
        let key = edit.target_key();
        let was_new = !self.exists(&key);
        let (parent, child, offset) = edit.edge();
        self.force_mut(&child);
        self.force_mut(&parent).bottom.insert(child, offset);
        (key, was_new)
    }
}

impl<I: VertexId> Extend<(I, VertexCache<I>)> for TraceCache<I> {
    fn extend<T: IntoIterator<Item = (I, VertexCache<I>)>>(
        &mut self,
        iter: T,
    ) {
        for (k, v) in iter {
            if let Some(c) = self.entries.get_mut(&k) {
                assert!(c.index == v.index);
                c.bottom_up.extend(v.bottom_up);
                c.top_down.extend(v.top_down);
            } else {
                self.entries.insert(k, v);
            }
        }
    }
}

pub trait RootKey<I: VertexId = CommittedIndex> {
    fn root_key(&self) -> UpKey<I>;
}

pub trait TargetKey<I: VertexId = CommittedIndex> {
    fn target_key(&self) -> DirectedKey<I>;
}

#[derive(Clone, Debug, PartialEq, Eq, From)]
pub enum EditKind<I: VertexId = CommittedIndex> {
    Parent(NewTraceEdge<BottomUp, I>),
    Child(NewTraceEdge<TopDown, I>),
}

impl<I: VertexId> TargetKey<I> for EditKind<I> {
    fn target_key(&self) -> DirectedKey<I> {
        match self {
            Self::Parent(edge) => edge.target.into(),
            Self::Child(edge) => edge.target.into(),
        }
    }
}

/// Edge between `prev` and `target` in trace direction `D`: bottom-up
/// `prev` is the child and `target` the parent, top-down the reverse.
/// `offset` is the child's offset in the parent.
#[derive(Clone, Debug, PartialEq, Eq, new)]
pub struct NewTraceEdge<D: TraceDirection, I: VertexId = CommittedIndex> {
    pub prev: D::Key<I>,
    pub target: D::Key<I>,
    pub offset: AtomPosition,
}

impl<I: VertexId> EditKind<I> {
    /// `(parent key, child key, child offset)`.
    pub(crate) fn edge(
        &self
    ) -> (DirectedKey<I>, DirectedKey<I>, AtomPosition) {
        match self {
            Self::Parent(e) => (e.target.into(), e.prev.into(), e.offset),
            Self::Child(e) => (e.prev.into(), e.target.into(), e.offset),
        }
    }
}
