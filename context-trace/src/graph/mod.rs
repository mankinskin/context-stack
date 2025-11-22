use std::sync::{
    Arc,
    RwLock,
};

use crate::{
    HashMap,
    TokenWidth,
    graph::{
        child_strings::TokenStrings,
        getters::vertex::VertexSet,
        kind::{
            BaseGraphKind,
            GraphKind,
        },
        vertex::{
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
    has_vertex_index::{
        HasVertexIndex,
        ToToken,
    },
    pattern::IntoPattern,
    token::Token,
    wide::Wide,
};

pub mod child_strings;
pub mod getters;
pub mod insert;
pub mod kind;
pub mod validation;

pub mod vertex;

#[cfg(any(test, feature = "test-api"))]
pub mod test_graph;

#[derive(Debug, Clone, Default)]
pub struct HypergraphRef<G: GraphKind = BaseGraphKind>(
    pub Arc<RwLock<Hypergraph<G>>>,
);

impl<G: GraphKind> HypergraphRef<G> {
    pub fn new(g: Hypergraph<G>) -> Self {
        Self::from(g)
    }
}

impl<G: GraphKind> From<Hypergraph<G>> for HypergraphRef<G> {
    fn from(g: Hypergraph<G>) -> Self {
        Self(Arc::new(RwLock::new(g)))
    }
}

impl<G: GraphKind> std::ops::Deref for HypergraphRef<G> {
    type Target = Arc<RwLock<Hypergraph<G>>>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<G: GraphKind> std::ops::DerefMut for HypergraphRef<G> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
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

#[derive(Debug, Serialize, Deserialize)]
pub struct Hypergraph<G: GraphKind = BaseGraphKind> {
    graph: indexmap::IndexMap<VertexKey, VertexData>,
    atoms: indexmap::IndexMap<VertexKey, Atom<G::Atom>>,
    atom_keys: indexmap::IndexMap<Atom<G::Atom>, VertexKey>,
    _ty: std::marker::PhantomData<G>,
}
impl<G: GraphKind> Clone for Hypergraph<G> {
    fn clone(&self) -> Self {
        Self {
            graph: self.graph.clone(),
            atoms: self.atoms.clone(),
            atom_keys: self.atom_keys.clone(),
            //pattern_id_count: self.pattern_id_count.load(Ordering::SeqCst).clone().into(),
            //vertex_id_count: self.vertex_id_count.load(Ordering::SeqCst).clone().into(),
            _ty: self._ty,
        }
    }
}
impl<G: GraphKind> PartialEq for Hypergraph<G> {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.graph.eq(&other.graph)
            && self.atoms.eq(&other.atoms)
            && self.atom_keys.eq(&other.atom_keys)
            //&& self
            //    .pattern_id_count
            //    .load(Ordering::SeqCst)
            //    .eq(&other.pattern_id_count.load(Ordering::SeqCst))
            //&& self
            //    .vertex_id_count
            //    .load(Ordering::SeqCst)
            //    .eq(&other.vertex_id_count.load(Ordering::SeqCst))
            && self._ty.eq(&other._ty)
    }
}
impl<G: GraphKind> Eq for Hypergraph<G> {}

impl<G: GraphKind> Default for Hypergraph<G> {
    fn default() -> Self {
        Self {
            graph: indexmap::IndexMap::default(),
            atoms: indexmap::IndexMap::default(),
            atom_keys: indexmap::IndexMap::default(),
            //pattern_id_count: AtomicUsize::new(0),
            //vertex_id_count: AtomicUsize::new(0),
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
    //pub(crate) fn next_vertex_id(&mut self) -> vertex::VertexIndex {
    //    self.vertex_id_count.fetch_add(1, atomic::Ordering::SeqCst)
    //}
    //pub(crate) fn next_pattern_id(&mut self) -> PatternId {
    //    self.pattern_id_count.fetch_add(1, atomic::Ordering::SeqCst)
    //}
    //pub(crate) fn index_sequence<N: Into<G>, I: IntoIterator<Item = N>>(&mut self, seq: I) -> VertexIndex {
    //    let seq = seq.into_iter();
    //    let atoms = T::atomize(seq);
    //    let pattern = self.to_atom_children(atoms);
    //    self.index_pattern(&pattern[..])
    //}
    //pub(crate) fn insert_atom_indices(
    //    &self,
    //    index: impl ToToken,
    //) -> Vec<VertexIndex> {
    //    if index.width() == 1 {
    //        vec![index.vertex_index()]
    //    } else {
    //        let data = self.expect_vertex(index);
    //        assert!(!data.tokens.is_empty());
    //        data.tokens
    //            .values()
    //            .search(None, |acc, p| {
    //                let exp = self.pattern_atom_indices(p.borrow());
    //                acc.map(|acc| {
    //                    assert_eq!(acc, exp);
    //                    acc
    //                })
    //                .or(Some(exp.clone()))
    //            })
    //            .unwrap()
    //    }
    //}
    //pub(crate) fn pattern_atom_indices(
    //    &self,
    //    pattern: impl IntoPattern,
    //) -> Vec<VertexIndex> {
    //    pattern
    //        .into_iter()
    //        .flat_map(|c| self.insert_atom_indices(c))
    //        .collect_vec()
    //}
    pub fn validate_expansion(
        &self,
        index: impl HasVertexIndex,
    ) {
        //let root = index.index();
        let data = self.expect_vertex(index.vertex_index());
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
        let nodes = self.graph.iter().map(|(_, data)| {
            (self.vertex_data_string(data), data.to_pattern_strings(self))
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
                self.expect_vertex(token.vertex_index())
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
    ) -> Option<&Atom<G::Atom>> {
        self.atoms.get(key)
    }
    #[allow(dead_code)]
    pub(crate) fn expect_atom_by_key(
        &self,
        key: &VertexKey,
    ) -> &Atom<G::Atom> {
        self.get_atom_by_key(key)
            .expect("Key does not belong to an atom!")
    }
    #[allow(dead_code)]
    pub(crate) fn vertex_key_string(
        &self,
        key: &VertexKey,
    ) -> String {
        self.vertex_data_string(self.expect_vertex(key))
    }
    pub(crate) fn vertex_data_string(
        &self,
        data: &VertexData,
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
    pub(crate) fn index_string(
        &self,
        index: impl HasVertexIndex,
    ) -> String {
        let data = self.expect_vertex(index.vertex_index());
        self.vertex_data_string(data)
    }
}
