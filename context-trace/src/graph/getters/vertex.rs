use std::fmt::Display;

use dashmap::mapref::one::Ref;

use crate::graph::{
    Hypergraph,
    getters::ErrorReason,
    kind::GraphKind,
    vertex::{
        VertexEntry,
        VertexIndex,
        data::VertexData,
        has_vertex_index::HasVertexIndex,
        has_vertex_key::HasVertexKey,
        key::VertexKey,
        token::Token,
    },
};

pub trait GetVertexKey {
    fn get_vertex_key<G: GraphKind>(
        &self,
        graph: &Hypergraph<G>,
    ) -> VertexKey;
}
impl<T: GetVertexKey> GetVertexKey for &'_ T {
    fn get_vertex_key<G: GraphKind>(
        &self,
        g: &Hypergraph<G>,
    ) -> VertexKey {
        (*self).get_vertex_key(g)
    }
}
macro_rules! impl_GetVertexKey_with {
    (($g:ident, $_self:ident) => $f:expr, {$($t:ty,)*}) => {
      $(
        impl GetVertexKey for $t {
            fn get_vertex_key<G: GraphKind>(&$_self, $g: &Hypergraph<G>) -> VertexKey {
                $f
            }
        }
      )*
    };
}
macro_rules! impl_GetVertexIndex_with {
    (($g:ident, $_self:ident) => $f:expr, {$($t:ty,)*}) => {
      $(
        impl GetVertexIndex for $t {
            fn get_vertex_index<G: GraphKind>(&$_self, $g: &Hypergraph<G>) -> VertexIndex {
                $f
            }
        }
      )*
    };
}
impl_GetVertexKey_with!(
    (graph, self) =>
        graph.expect_key_for_index(self),
    {
        VertexIndex,
        Token,
    }
);
impl_GetVertexKey_with!(
    (_graph, self) =>
        self.vertex_key(),
    {
        VertexKey,
    }
);
impl_GetVertexIndex_with!(
    (_graph, self) => self.vertex_index(),
    {
        VertexIndex,
        Token,
    }
);

pub trait GetVertexIndex: GetVertexKey + Display + Clone + Copy {
    fn get_vertex_index<G: GraphKind>(
        &self,
        graph: &Hypergraph<G>,
    ) -> VertexIndex;
}
//impl<T: HasVertexIndex + GetVertexKey + Display + Clone + Copy> GetVertexIndex for T {
//    fn get_vertex_index<G: GraphKind>(&self, _: &Hypergraph<G>) -> VertexIndex {
//        self.vertex_index()
//    }
//}
impl GetVertexIndex for VertexKey {
    fn get_vertex_index<G: GraphKind>(
        &self,
        graph: &Hypergraph<G>,
    ) -> VertexIndex {
        graph.expect_index_for_key(self)
    }
}
impl<T: GetVertexIndex> GetVertexIndex for &'_ T {
    fn get_vertex_index<G: GraphKind>(
        &self,
        g: &Hypergraph<G>,
    ) -> VertexIndex {
        (*self).get_vertex_index(g)
    }
}

/// A read guard for vertex data from the concurrent graph.
///
/// This holds both the DashMap ref and the RwLock read guard.
pub struct VertexReadGuard<'a> {
    _entry_ref: Ref<'a, VertexKey, VertexEntry>,
    // We store a raw pointer because we can't express the lifetime properly
    // The RwLockReadGuard is logically borrowed from entry_ref
    data_ptr: *const VertexData,
}

impl<'a> std::ops::Deref for VertexReadGuard<'a> {
    type Target = VertexData;
    fn deref(&self) -> &Self::Target {
        // SAFETY: The pointer is valid as long as _entry_ref is held
        unsafe { &*self.data_ptr }
    }
}

/// Trait for accessing vertices in a graph.
///
/// With DashMap, we can't return direct references, so methods return
/// cloned data or use callbacks.
pub trait VertexSet<I: GetVertexIndex> {
    /// Get vertex data by key. Returns a clone of the data.
    fn get_vertex_data(
        &self,
        key: I,
    ) -> Result<VertexData, ErrorReason>;

    /// Get vertex data, panicking if not found.
    fn expect_vertex_data(
        &self,
        index: I,
    ) -> VertexData {
        self.get_vertex_data(index)
            .unwrap_or_else(|_| panic!("Vertex {} does not exist!", index))
    }

    /// Execute a function with read access to vertex data.
    fn with_vertex<R>(
        &self,
        key: I,
        f: impl FnOnce(&VertexData) -> R,
    ) -> Result<R, ErrorReason>;

    /// Execute a function with write access to vertex data.
    fn with_vertex_mut<R>(
        &self,
        key: I,
        f: impl FnOnce(&mut VertexData) -> R,
    ) -> Result<R, ErrorReason>;

    /// Check if vertex exists.
    fn contains_vertex(
        &self,
        key: I,
    ) -> bool {
        self.get_vertex_data(key).is_ok()
    }
}

impl<'t, G: GraphKind, I: GetVertexIndex> VertexSet<&'t I> for Hypergraph<G>
where
    Hypergraph<G>: VertexSet<I>,
{
    fn get_vertex_data(
        &self,
        key: &'t I,
    ) -> Result<VertexData, ErrorReason> {
        self.get_vertex_data(*key)
    }

    fn with_vertex<R>(
        &self,
        key: &'t I,
        f: impl FnOnce(&VertexData) -> R,
    ) -> Result<R, ErrorReason> {
        self.with_vertex(*key, f)
    }

    fn with_vertex_mut<R>(
        &self,
        key: &'t I,
        f: impl FnOnce(&mut VertexData) -> R,
    ) -> Result<R, ErrorReason> {
        self.with_vertex_mut(*key, f)
    }
}

impl<G: GraphKind> VertexSet<VertexKey> for Hypergraph<G> {
    fn get_vertex_data(
        &self,
        key: VertexKey,
    ) -> Result<VertexData, ErrorReason> {
        self.graph
            .get(&key)
            .map(|entry| entry.clone_data())
            .ok_or(ErrorReason::UnknownIndex)
    }

    fn with_vertex<R>(
        &self,
        key: VertexKey,
        f: impl FnOnce(&VertexData) -> R,
    ) -> Result<R, ErrorReason> {
        // Clone the Arc to release the DashMap shard lock before acquiring vertex lock.
        // This prevents deadlocks when the callback accesses other vertices.
        let entry = self
            .graph
            .get(&key)
            .map(|r| r.clone())
            .ok_or(ErrorReason::UnknownIndex)?;
        Ok(f(&entry.read()))
    }

    fn with_vertex_mut<R>(
        &self,
        key: VertexKey,
        f: impl FnOnce(&mut VertexData) -> R,
    ) -> Result<R, ErrorReason> {
        // Clone the Arc to release the DashMap shard lock before acquiring vertex lock.
        // This prevents deadlocks when the callback accesses other vertices.
        let entry = self
            .graph
            .get(&key)
            .map(|r| r.clone())
            .ok_or(ErrorReason::UnknownIndex)?;
        Ok(f(&mut entry.write()))
    }
}

impl<G: GraphKind> VertexSet<VertexIndex> for Hypergraph<G> {
    fn get_vertex_data(
        &self,
        key: VertexIndex,
    ) -> Result<VertexData, ErrorReason> {
        let vk = self.get_key_for_index(key)?;
        self.get_vertex_data(vk)
    }

    fn with_vertex<R>(
        &self,
        key: VertexIndex,
        f: impl FnOnce(&VertexData) -> R,
    ) -> Result<R, ErrorReason> {
        let vk = self.get_key_for_index(key)?;
        self.with_vertex(vk, f)
    }

    fn with_vertex_mut<R>(
        &self,
        key: VertexIndex,
        f: impl FnOnce(&mut VertexData) -> R,
    ) -> Result<R, ErrorReason> {
        let vk = self.get_key_for_index(key)?;
        self.with_vertex_mut(vk, f)
    }
}

impl<G: GraphKind> VertexSet<Token> for Hypergraph<G> {
    fn get_vertex_data(
        &self,
        key: Token,
    ) -> Result<VertexData, ErrorReason> {
        self.get_vertex_data(key.vertex_index())
    }

    fn with_vertex<R>(
        &self,
        key: Token,
        f: impl FnOnce(&VertexData) -> R,
    ) -> Result<R, ErrorReason> {
        self.with_vertex(key.vertex_index(), f)
    }

    fn with_vertex_mut<R>(
        &self,
        key: Token,
        f: impl FnOnce(&mut VertexData) -> R,
    ) -> Result<R, ErrorReason> {
        self.with_vertex_mut(key.vertex_index(), f)
    }
}
impl<G: GraphKind> Hypergraph<G> {
    /// Get the index for a key by looking up in key_to_index map.
    pub(crate) fn get_index_for_key(
        &self,
        key: &VertexKey,
    ) -> Result<VertexIndex, ErrorReason> {
        self.key_to_index
            .get(key)
            .map(|r| *r)
            .ok_or(ErrorReason::UnknownKey)
    }

    #[track_caller]
    pub fn expect_index_for_key(
        &self,
        key: &VertexKey,
    ) -> VertexIndex {
        self.get_index_for_key(key).expect("Key does not exist")
    }

    /// Get the key for an index by looking up in index_to_key map.
    pub(crate) fn get_key_for_index(
        &self,
        index: impl HasVertexIndex,
    ) -> Result<VertexKey, ErrorReason> {
        self.index_to_key
            .get(&index.vertex_index())
            .map(|r| *r)
            .ok_or(ErrorReason::UnknownKey)
    }

    #[track_caller]
    pub fn expect_key_for_index(
        &self,
        index: impl HasVertexIndex,
    ) -> VertexKey {
        self.get_key_for_index(index).expect("Key does not exist")
    }

    /// Get the next vertex index that would be allocated (without incrementing).
    pub fn next_vertex_index(&self) -> VertexIndex {
        VertexIndex::from(
            self.next_id.load(std::sync::atomic::Ordering::Relaxed),
        )
    }

    /// Try to get vertex data without blocking.
    ///
    /// This is useful for avoiding deadlocks when called from within a callback
    /// that already holds a lock on a vertex (e.g., when formatting tokens during
    /// validation inside a write lock).
    ///
    /// Returns `None` if:
    /// - The vertex doesn't exist
    /// - A write lock is currently held on the vertex
    pub fn try_get_vertex_data(
        &self,
        index: impl HasVertexIndex,
    ) -> Option<VertexData> {
        let key = self.get_key_for_index(index).ok()?;
        self.graph
            .get(&key)
            .and_then(|entry| entry.try_clone_data())
    }

    /// Iterate over all vertices (key, data pairs) - returns cloned data.
    pub(crate) fn vertex_iter(
        &self
    ) -> impl Iterator<Item = (VertexKey, VertexData)> + '_ {
        self.graph
            .iter()
            .map(|entry| (*entry.key(), entry.clone_data()))
    }

    /// Iterate over all vertex keys.
    pub fn vertex_keys(&self) -> impl Iterator<Item = VertexKey> + '_ {
        self.graph.iter().map(|entry| *entry.key())
    }
}
