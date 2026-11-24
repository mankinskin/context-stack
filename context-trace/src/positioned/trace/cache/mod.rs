//! Positioned trace cache (legacy `trace::cache`, Decision 4 a).
//!
//! Same shape as legacy: vertex → [`VertexCache`] `{ bottom_up, top_down }`
//! → query position → [`PositionCache`] `{ top, bottom }`, keyed by
//! [`DirectedKey`] with query positions. Edges are recorded at the parent and
//! labelled by the child's offset in the parent, so consumers read offsets
//! without a composition lookup. The root offset of the match is not stored
//! here but once in [`RangePath::span`](crate::positioned::path::RangePath).

pub mod key;
pub mod new;
pub mod position;
pub mod vertex;

use derive_more::derive::IntoIterator;

use crate::{
    AtomPosition,
    DirectedKey,
    HashMap,
    Token,
    graph::vertex::{
        CommittedIndex,
        VertexId,
        has_vertex_index::HasVertexIndex,
    },
};
use key::TargetKey;
use new::EditKind;
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

impl TraceCache {
    /// Add the target key of `edit` and record its edge at the parent.
    /// Returns the target key and whether it was new. Unlike legacy, the
    /// edge is recorded even if the target key existed.
    pub fn add_state<E: Into<EditKind>>(
        &mut self,
        edit: E,
    ) -> (DirectedKey, bool) {
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
