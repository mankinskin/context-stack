use std::sync::{
    Arc,
    atomic::{
        AtomicUsize,
        Ordering,
    },
};

use dashmap::DashMap;

use crate::{
    HashMap,
    TokenWidth,
    Wide,
    graph::{
        child_strings::TokenStrings,
        getters::vertex::VertexSet,
        kind::{
            BaseGraphKind,
            GraphKind,
        },
        vertex::{
            VertexEntry,
            data::VertexData,
            key::VertexKey,
        },
    },
};
use derive_new::new;
use itertools::Itertools;
use petgraph::graph::DiGraph;
use serde::{
    Deserialize,
    Serialize,
};
use vertex::{
    VertexIndex,
    atom::Atom,
    has_vertex_index::HasVertexIndex,
    pattern::IntoPattern,
    token::Token,
};

pub mod child_strings;
pub mod getters;
pub mod insert;
pub mod kind;
pub mod validation;

pub mod vertex;

#[cfg(any(test, feature = "test-api"))]
pub mod test_graph;

/// Thread-safe reference to a Hypergraph.
///
/// Uses `Arc<Hypergraph>` with interior mutability (per-vertex RwLocks via DashMap).
/// All methods can be called with `&self` - no need for `.read()` or `.write()`.
#[derive(Debug, Clone, Default)]
pub struct HypergraphRef<G: GraphKind = BaseGraphKind>(pub Arc<Hypergraph<G>>);

impl<G: GraphKind> HypergraphRef<G> {
    pub fn new(g: Hypergraph<G>) -> Self {
        Self::from(g)
    }
}

impl<G: GraphKind> From<Hypergraph<G>> for HypergraphRef<G> {
    fn from(g: Hypergraph<G>) -> Self {
        Self(Arc::new(g))
    }
}

impl<G: GraphKind> std::ops::Deref for HypergraphRef<G> {
    type Target = Hypergraph<G>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<G: GraphKind> AsRef<Self> for Hypergraph<G> {
    fn as_ref(&self) -> &Self {
        self
    }
}

impl<G: GraphKind> AsMut<Self> for Hypergraph<G> {
    fn as_mut(&mut self) -> &mut Self {
        self
    }
}

/// A hypergraph data structure with concurrent per-vertex access.
///
/// Uses `DashMap` for the vertex storage with per-vertex `RwLock`s,
/// enabling concurrent reads during writes to other vertices.
#[derive(Debug, Serialize, Deserialize)]
pub struct Hypergraph<G: GraphKind = BaseGraphKind> {
    /// Lock-free vertex ID counter
    next_id: AtomicUsize,
    /// Concurrent vertex storage with per-vertex locking
    graph: DashMap<VertexKey, VertexEntry>,
    /// Bidirectional key<->index mappings
    key_to_index: DashMap<VertexKey, VertexIndex>,
    index_to_key: DashMap<VertexIndex, VertexKey>,
    /// Atom data (indexed by key)
    atoms: DashMap<VertexKey, Atom<G::Atom>>,
    /// Reverse lookup: atom value -> key
    atom_keys: DashMap<Atom<G::Atom>, VertexKey>,
    _ty: std::marker::PhantomData<G>,
}

impl<G: GraphKind> Clone for Hypergraph<G> {
    fn clone(&self) -> Self {
        // Clone all entries from DashMaps
        let graph = DashMap::new();
        for entry in self.graph.iter() {
            graph.insert(
                *entry.key(),
                VertexEntry::new(entry.value().clone_data()),
            );
        }
        let key_to_index = DashMap::new();
        for entry in self.key_to_index.iter() {
            key_to_index.insert(*entry.key(), *entry.value());
        }
        let index_to_key = DashMap::new();
        for entry in self.index_to_key.iter() {
            index_to_key.insert(*entry.key(), *entry.value());
        }
        let atoms = DashMap::new();
        for entry in self.atoms.iter() {
            atoms.insert(*entry.key(), entry.value().clone());
        }
        let atom_keys = DashMap::new();
        for entry in self.atom_keys.iter() {
            atom_keys.insert(entry.key().clone(), *entry.value());
        }
        Self {
            next_id: AtomicUsize::new(self.next_id.load(Ordering::SeqCst)),
            graph,
            key_to_index,
            index_to_key,
            atoms,
            atom_keys,
            _ty: self._ty,
        }
    }
}

impl<G: GraphKind> PartialEq for Hypergraph<G> {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        if self.graph.len() != other.graph.len() {
            return false;
        }
        // Compare all entries
        for entry in self.graph.iter() {
            match other.graph.get(entry.key()) {
                Some(other_entry) => {
                    if *entry.value().read() != *other_entry.read() {
                        return false;
                    }
                },
                None => return false,
            }
        }
        true
    }
}
impl<G: GraphKind> Eq for Hypergraph<G> {}

impl<G: GraphKind> Default for Hypergraph<G> {
    fn default() -> Self {
        Self {
            next_id: AtomicUsize::new(0),
            graph: DashMap::new(),
            key_to_index: DashMap::new(),
            index_to_key: DashMap::new(),
            atoms: DashMap::new(),
            atom_keys: DashMap::new(),
            _ty: Default::default(),
        }
    }
}

impl<G: GraphKind> Hypergraph<G> {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn to_ref(self) -> HypergraphRef<G> {
        self.into()
    }
    pub fn vertex_count(&self) -> usize {
        self.graph.len()
    }
    pub fn validate_expansion(
        &self,
        index: impl HasVertexIndex,
    ) {
        //let root = index.index();
        let data = self.expect_vertex_data(index.vertex_index());
        data.children.iter().fold(
            Vec::new(),
            |mut acc: Vec<vertex::VertexIndex>, (_pid, p)| {
                assert!(!p.is_empty());
                let exp = p.iter().map(|c| c.vertex_index()).collect_vec();
                if acc.is_empty() {
                    acc = exp;
                } else {
                    assert_eq!(acc, exp);
                }
                acc
            },
        );
    }
}

#[derive(Clone, Debug)]
pub struct Edge {
    pub parent: vertex::parent::Parent,
    pub token: Token,
}

#[derive(Clone, Debug, new)]
pub struct Node {
    pub name: String,
    pub data: VertexData,
}

impl<'a, G: GraphKind> Hypergraph<G>
where
    G::Atom: std::fmt::Display,
{
    pub fn to_petgraph(&self) -> DiGraph<(VertexIndex, Node), Edge> {
        let mut pg = DiGraph::new() as DiGraph<(VertexIndex, Node), Edge>;
        // id refers to index in Hypergraph
        // idx refers to index in petgraph
        let nodes: HashMap<_, (_, Node)> = self
            .vertex_iter()
            .map(|(_id, data)| {
                let vi = data.vertex_index();
                let label = self.index_string(vi);
                let node = Node::new(label, data.clone());
                let idx = pg.add_node((vi, node.clone()));
                (vi, (idx, node))
            })
            .collect();
        nodes.values().for_each(|(idx, node)| {
            let parents = node.data.parents();
            for (p_id, parent) in parents {
                let (p_idx, _p_data) = nodes
                    .get(p_id)
                    .expect("Parent not mapped to node in petgraph!");
                pg.add_edge(
                    *p_idx,
                    *idx,
                    Edge {
                        parent: parent.clone(),
                        token: node.data.to_child(),
                    },
                );
            }
        });
        pg
    }

    pub fn to_node_child_strings(&self) -> TokenStrings {
        let nodes = self.graph.iter().map(|entry| {
            let data = entry.value().clone_data();
            (
                self.vertex_data_string(data.clone()),
                data.to_pattern_strings(self),
            )
        });
        TokenStrings::from_nodes(nodes)
    }
    #[allow(dead_code)]
    pub(crate) fn pattern_child_strings(
        &self,
        pattern: impl IntoPattern,
    ) -> TokenStrings {
        let nodes = pattern.into_pattern().into_iter().map(|token| {
            (
                self.index_string(token.vertex_index()),
                self.expect_vertex_data(token.vertex_index())
                    .to_pattern_strings(self),
            )
        });
        TokenStrings::from_nodes(nodes)
    }

    pub(crate) fn pattern_string_with_separator(
        &'a self,
        pattern: impl IntoIterator<Item = impl HasVertexIndex>,
        separator: &'static str,
    ) -> String {
        pattern
            .into_iter()
            .map(|token| self.index_string(token.vertex_index()))
            .join(separator)
    }
    #[allow(dead_code)]
    pub(crate) fn separated_pattern_string(
        &'a self,
        pattern: impl IntoIterator<Item = impl HasVertexIndex>,
    ) -> String {
        self.pattern_string_with_separator(pattern, "_")
    }
    pub(crate) fn pattern_string(
        &'a self,
        pattern: impl IntoIterator<Item = impl HasVertexIndex>,
    ) -> String {
        self.pattern_string_with_separator(pattern, "")
    }
    #[allow(dead_code)]
    pub(crate) fn pattern_strings(
        &'a self,
        patterns: impl IntoIterator<
            Item = impl IntoIterator<Item = impl HasVertexIndex>,
        >,
    ) -> Vec<String> {
        patterns
            .into_iter()
            .map(|pattern| self.pattern_string_with_separator(pattern, ""))
            .collect()
    }
    pub(crate) fn get_atom_by_key(
        &self,
        key: &VertexKey,
    ) -> Option<Atom<G::Atom>> {
        self.atoms.get(key).map(|r| r.clone())
    }
    #[allow(dead_code)]
    pub(crate) fn expect_atom_by_key(
        &self,
        key: &VertexKey,
    ) -> Atom<G::Atom> {
        self.get_atom_by_key(key)
            .expect("Key does not belong to an atom!")
    }
    #[allow(dead_code)]
    pub fn vertex_key_string(
        &self,
        key: &VertexKey,
    ) -> String {
        self.vertex_data_string(self.expect_vertex_data(key))
    }
    pub fn vertex_data_string(
        &self,
        data: VertexData,
    ) -> String {
        #[cfg(any(test, feature = "test-api"))]
        {
            // Check cache first
            if let Ok(cache) = data.cached_string.read()
                && let Some(cached) = cache.as_ref()
            {
                return cached.clone();
            }
        }

        // Compute string
        let s = if let Some(atom) = self.get_atom_by_key(&data.key) {
            atom.to_string()
        } else {
            assert!(data.width() > TokenWidth(1));
            self.pattern_string(data.expect_any_child_pattern().1)
        };

        #[cfg(any(test, feature = "test-api"))]
        {
            // Populate cache
            if let Ok(mut cache) = data.cached_string.write() {
                *cache = Some(s.clone());
            }
        }

        s
    }
    pub fn index_string(
        &self,
        index: impl HasVertexIndex,
    ) -> String {
        let data = self.expect_vertex_data(index.vertex_index());
        self.vertex_data_string(data)
    }
}
