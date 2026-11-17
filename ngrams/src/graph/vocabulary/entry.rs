//! Join containment-graph vertices with corpus occurrence metadata.

use derive_more::Deref;
use serde::{
    Deserialize,
    Serialize,
};

use context_trace::{
    graph::vertex::{
        data::VertexData,
        has_vertex_index::HasVertexIndex,
        has_vertex_key::HasVertexKey,
        key::VertexKey,
        parent::Parent,
        token::{
            Token,
            TokenWidth,
        },
        wide::Wide,
        VertexEntry,
        VertexIndex,
    },
    HashMap,
    HashSet,
    VertexSet,
};

use crate::graph::{
    containment::TextLocation,
    vocabulary::{
        NGramId,
        Vocabulary,
    },
};
use std::{
    borrow::Borrow,
    fmt::Debug,
};

/// Corpus metadata for one distinct substring, independent of graph structure.
#[derive(Debug, Deref, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct VocabEntry {
    //pub(crate) id: NGramId,
    /// Distinct text/offset pairs, including overlapping occurrences.
    pub(crate) occurrences: HashSet<TextLocation>,
    // positions of largest smaller ngrams
    //pub(crate) children: NodeChildren,
    /// Substring content, also exposed through dereferencing.
    #[deref]
    pub(crate) ngram: String,
}

impl VocabEntry {
    /// Number of recorded positions, not the number of containing texts.
    pub(crate) fn count(&self) -> usize {
        self.occurrences.len()
    }
    //pub(crate) fn needs_node(&self) -> bool {
    //    self.len() == 1
    //        || self.children.has_overlaps()
    //}
}
/// A graph-data snapshot paired with borrowed vocabulary metadata.
#[derive(Debug, Deref, Clone)]
pub(crate) struct VertexCtx<'a> {
    /// Owned vertex data returned by the containment graph.
    pub(crate) data: VertexData,
    /// Occurrence metadata, also exposed through dereferencing.
    #[deref]
    pub(crate) entry: &'a VocabEntry,
    /// Vocabulary used to resolve neighbouring vertices.
    pub(crate) vocab: &'a Vocabulary,
}
impl Wide for VertexCtx<'_> {
    /// Read width from graph data rather than the string's byte length.
    fn width(&self) -> TokenWidth {
        self.data.width()
    }
}
impl HasVertexKey for VertexCtx<'_> {
    /// Use the containment vertex's identity for label-set membership.
    fn vertex_key(&self) -> VertexKey {
        self.data.vertex_key()
    }
}
/// Mutable occurrence metadata paired with an owned graph-data snapshot.
///
/// Editing `data` does not itself write changes back to the containment graph.
#[derive(Debug, Deref)]
pub(crate) struct VertexCtxMut<'a> {
    /// Owned containment vertex data.
    pub(crate) data: VertexData,
    /// Exclusively borrowed corpus metadata.
    #[deref]
    pub(crate) entry: &'a mut VocabEntry,
}
/// Resolve vocabulary entries by content, vertex key, or graph index.
///
/// Implementations assume every resolved graph vertex has matching metadata.
pub(crate) trait HasVertexEntries<K: ?Sized + Debug> {
    /// Return an owned graph entry, panicking if the key cannot be resolved.
    fn entry(
        &mut self,
        key: K,
    ) -> VertexEntry;
    /// Look up graph data and immutable occurrence metadata.
    fn get_vertex(
        &'_ self,
        key: &K,
    ) -> Option<VertexCtx<'_>>;
    /// Look up graph data and mutable occurrence metadata.
    fn get_vertex_mut(
        &'_ mut self,
        key: &K,
    ) -> Option<VertexCtxMut<'_>>;
    /// Resolve an immutable context, panicking when the entry is absent.
    fn expect_vertex(
        &'_ self,
        key: &K,
    ) -> VertexCtx<'_> {
        self.get_vertex(key)
            .unwrap_or_else(|| panic!("No VertexKey: {:?}", key))
    }
    /// Resolve a mutable context, panicking when the entry is absent.
    fn expect_vertex_mut(
        &'_ mut self,
        key: &K,
    ) -> VertexCtxMut<'_> {
        self.get_vertex_mut(key)
            .unwrap_or_else(|| panic!("No VertexKey: {:?}", key))
    }
}
/// Marker for index-like vocabulary handles; currently has no implementations here.
pub(crate) trait VocabIndex: HasVertexIndex {}
//impl VocabIndex for VertexIndex {}
//impl VocabIndex for NGramId {}
/// Implement metadata lookup after converting an index-like handle to a key.
macro_rules! impl_index_vocab {
    ($t:ty, ($_self:ident, $ind:ident) => $func:expr) => {
        impl HasVertexEntries<$t> for Vocabulary
        {
            /// Clone graph data for a resolved handle, panicking when absent.
            fn entry(
                &'_ mut $_self,
                $ind: $t,
            ) -> VertexEntry
            {
                let key = $func;
                let data = $_self.containment.expect_vertex_data(key);
                VertexEntry::new(data)
            }
            /// Pair a graph snapshot with immutable occurrence metadata.
            fn get_vertex(
                &'_ $_self,
                $ind: &$t,
            ) -> Option<VertexCtx<'_>>
            {
                let key = $func;
                $_self.containment.get_vertex_data(key).ok().map(|data| {
                    VertexCtx {
                        data,
                        entry: $_self.entries.get(&key).unwrap(),
                        vocab: $_self,
                    }
                })
            }
            /// Pair a graph snapshot with exclusively borrowed occurrence metadata.
            fn get_vertex_mut(
                &'_ mut $_self,
                $ind: &$t,
            ) -> Option<VertexCtxMut<'_>>
            {
                let key = $func;
                $_self.containment.get_vertex_data(key).ok().map(|data| {
                    VertexCtxMut {
                        data,
                        entry: $_self.entries.get_mut(&key).unwrap(),
                    }
                })
            }
        }
    };
}
impl_index_vocab!(VertexKey, (self, key) => key.vertex_key());
impl_index_vocab!(NGramId, (self, key) => key.vertex_key());
impl_index_vocab!(Token, (self, index) => self.containment.expect_key_for_index(index.vertex_index()));
impl_index_vocab!(VertexIndex, (self, index) => self.containment.expect_key_for_index(index));

/// Resolve string content through the vocabulary's content-to-ID map.
macro_rules! impl_index_vocab_str {
    ($t:ty) => {
        impl HasVertexEntries<$t> for Vocabulary {
            /// Resolve content and return its graph entry, panicking when absent.
            fn entry(
                &'_ mut self,
                key: $t,
            ) -> VertexEntry {
                self.entry(*self.ids.get(key.borrow() as &'_ str).unwrap())
            }
            /// Return no context if this content has no vocabulary handle.
            fn get_vertex(
                &'_ self,
                key: &$t,
            ) -> Option<VertexCtx<'_>> {
                self.get_vertex(self.ids.get(key.borrow() as &'_ str)?)
            }
            /// Resolve content before borrowing the corresponding metadata mutably.
            fn get_vertex_mut(
                &'_ mut self,
                key: &$t,
            ) -> Option<VertexCtxMut<'_>> {
                let id = *self.ids.get(key.borrow() as &'_ str)?;
                self.get_vertex_mut(&id)
            }
        }
    };
}
impl_index_vocab_str!(&'_ str);
impl_index_vocab_str!(String);
