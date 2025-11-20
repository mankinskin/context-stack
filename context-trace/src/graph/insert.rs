use std::{
    borrow::Borrow,
    sync::atomic::AtomicUsize,
};

use itertools::Itertools;
use lazy_static::lazy_static;

use crate::{
    HashSet,
    graph::{
        getters::{
            ErrorReason,
            vertex::VertexSet,
        },
        kind::GraphKind,
        vertex::{
            atom::{
                Atom,
                NewAtomIndex,
                NewAtomIndices,
            },
            data::{
                VertexData,
                VertexDataBuilder,
            },
            has_vertex_index::{
                HasVertexIndex,
                ToToken,
            },
            key::VertexKey,
            location::{
                child::ChildLocation,
                pattern::IntoPatternLocation,
            },
            parent::PatternIndex,
            pattern::{
                IntoPattern,
                Pattern,
                id::PatternId,
                pattern_range::{
                    PatternRangeIndex,
                    get_child_pattern_range,
                },
                pattern_width,
                replace_in_pattern,
            },
            token::Token,
        },
    },
};

lazy_static! {
    static ref VERTEX_ID_COUNTER: AtomicUsize = AtomicUsize::new(0);
}
impl<G> crate::graph::Hypergraph<G>
where
    G: GraphKind,
{
    #[allow(dead_code)]
    pub(crate) fn insert_vertex_builder(
        &mut self,
        builder: VertexDataBuilder,
    ) -> Token {
        let data = self.finish_vertex_builder(builder);
        self.insert_vertex_data(data)
    }
    #[allow(dead_code)]
    pub(crate) fn finish_vertex_builder(
        &mut self,
        mut builder: VertexDataBuilder,
    ) -> VertexData {
        builder.index(self.next_vertex_index()).build().unwrap()
    }
    /// insert raw vertex data
    pub(crate) fn insert_vertex_data(
        &mut self,
        data: VertexData,
    ) -> Token {
        let c = Token::new(data.vertex_index(), data.width);
        self.graph.insert(data.key, data);
        c
    }
    fn insert_atom_key(
        &mut self,
        atom: Atom<G::Atom>,
        key: VertexKey,
    ) {
        self.atoms.insert(key, atom);
        self.atom_keys.insert(atom, key);
    }
    /// insert raw vertex data
    pub(crate) fn insert_atom_data(
        &mut self,
        atom: Atom<G::Atom>,
        data: VertexData,
    ) -> Token {
        self.insert_atom_key(atom, data.key);
        self.insert_vertex_data(data)
    }
    #[allow(dead_code)]
    pub(crate) fn insert_atom_builder(
        &mut self,
        atom: Atom<G::Atom>,
        builder: VertexDataBuilder,
    ) -> Token {
        let data = self.finish_vertex_builder(builder);
        self.insert_atom_data(atom, data)
    }
    /// insert single atom node
    pub fn insert_atom(
        &mut self,
        atom: Atom<G::Atom>,
    ) -> Token {
        let data = VertexData::new(self.next_vertex_index(), 1);
        self.insert_atom_data(atom, data)
    }
    /// insert multiple atom nodes
    pub fn insert_atoms(
        &mut self,
        atoms: impl IntoIterator<Item = Atom<G::Atom>>,
    ) -> Vec<Token> {
        atoms
            .into_iter()
            .map(|atom| self.insert_atom(atom))
            .collect()
    }
    /// utility, builds total width, indices and tokens for pattern
    fn to_width_indices_children(
        &self,
        indices: impl IntoIterator<Item = impl HasVertexIndex>,
    ) -> (usize, Vec<crate::graph::vertex::VertexIndex>, Vec<Token>) {
        let mut width = 0;
        let (a, b) = indices
            .into_iter()
            .map(|index| {
                let index = index.vertex_index();
                let w = self.expect_vertex(index.vertex_index()).get_width();
                width += w;
                (index, Token::new(index, w))
            })
            .unzip();
        (width, a, b)
    }
    /// adds a parent to all nodes in a pattern
    #[track_caller]
    pub(crate) fn add_parents_to_pattern_nodes<
        I: HasVertexIndex,
        P: ToToken,
    >(
        &mut self,
        pattern: Vec<I>,
        parent: P,
        pattern_id: PatternId,
    ) {
        for (i, token) in pattern.into_iter().enumerate() {
            let node = self.expect_vertex_mut(token.vertex_index());
            node.add_parent(ChildLocation::new(
                parent.to_child(),
                pattern_id,
                i,
            ));
        }
    }
    pub(crate) fn validate_vertex(
        &self,
        index: impl HasVertexIndex,
    ) {
        self.expect_vertex(index.vertex_index()).validate()
    }
    /// add pattern to existing node
    pub fn add_pattern_with_update(
        &mut self,
        index: impl HasVertexIndex,
        pattern: Pattern,
    ) -> PatternId {
        // todo handle atom nodes
        let indices = pattern.into_pattern();
        let (width, indices, tokens) = self.to_width_indices_children(indices);
        let pattern_id = PatternId::default();
        let data = self.expect_vertex_mut(index.vertex_index());
        data.add_pattern_no_update(pattern_id, Pattern::from(tokens));
        self.add_parents_to_pattern_nodes(
            indices,
            Token::new(index, width),
            pattern_id,
        );
        pattern_id
    }
    /// add pattern to existing node
    //#[track_caller]
    pub fn add_patterns_with_update(
        &mut self,
        index: impl HasVertexIndex,
        patterns: impl IntoIterator<Item = Pattern>,
    ) -> Vec<PatternId> {
        let index = index.vertex_index();
        patterns
            .into_iter()
            .map(|p| self.add_pattern_with_update(index, p))
            .collect()
    }
    /// create new node from a pattern
    #[track_caller]
    pub fn insert_pattern_with_id(
        &mut self,
        pattern: impl IntoPattern,
    ) -> (Token, Option<PatternId>) {
        let indices = pattern.into_pattern();
        let (c, id) = match indices.len() {
            0 => (None, None),
            1 => (
                Some(self.to_child(indices.first().unwrap().vertex_index())),
                None,
            ),
            _ => {
                let (c, id) = self.force_insert_pattern_with_id(indices);
                (Some(c), Some(id))
            },
        };
        (c.expect("Tried to index empty pattern!"), id)
    }
    /// create new node from a pattern (even if single index)
    //#[track_caller]
    pub(crate) fn force_insert_pattern_with_id(
        &mut self,
        pattern: impl IntoPattern,
    ) -> (Token, PatternId) {
        let indices = pattern.into_pattern();
        let (width, indices, tokens) = self.to_width_indices_children(indices);
        let index = self.next_vertex_index();
        let mut new_data = VertexData::new(index, width);
        let pattern_id = PatternId::default();
        new_data.add_pattern_no_update(pattern_id, Pattern::from(tokens));
        let index = self.insert_vertex_data(new_data);
        self.add_parents_to_pattern_nodes(indices, index, pattern_id);
        (index, pattern_id)
    }
    /// create new node from a pattern
    pub fn insert_pattern(
        &mut self,
        pattern: impl IntoPattern,
    ) -> Token {
        let indices = pattern.into_pattern();
        self.insert_pattern_with_id(indices).0
    }
    /// create new node from a pattern
    pub(crate) fn force_insert_pattern(
        &mut self,
        indices: impl IntoPattern,
    ) -> Token {
        self.force_insert_pattern_with_id(indices).0
    }
    pub fn insert_patterns_with_ids(
        &mut self,
        patterns: impl IntoIterator<Item = Pattern>,
    ) -> (Token, Vec<PatternId>) {
        // todo handle atom nodes
        let patterns = patterns.into_iter().collect_vec();
        let mut ids = Vec::with_capacity(patterns.len());
        let mut patterns = patterns.into_iter();
        let first = patterns.next().expect("Tried to insert no patterns");
        let (node, first_id) = self.insert_pattern_with_id(first);
        ids.push(first_id.unwrap());
        for pat in patterns {
            ids.push(self.add_pattern_with_update(node, pat));
        }
        (node, ids)
    }
    /// create new node from multiple patterns
    //#[track_caller]
    pub fn insert_patterns(
        &mut self,
        patterns: impl IntoIterator<Item = impl IntoPattern>,
    ) -> Token {
        let patterns = patterns
            .into_iter()
            .map(IntoPattern::into_pattern)
            .collect_vec();
        patterns
            .iter()
            .find(|p| p.len() == 1)
            .map(|p| *p.first().unwrap())
            .unwrap_or_else(|| {
                // todo handle atom nodes
                let mut patterns = patterns.into_iter();
                let first =
                    patterns.next().expect("Tried to insert no patterns");
                let node = self.insert_pattern(first);
                for pat in patterns {
                    self.add_pattern_with_update(node, pat);
                }
                node
            })
    }
    #[track_caller]
    pub(crate) fn try_insert_patterns(
        &mut self,
        patterns: impl IntoIterator<Item = Pattern>,
    ) -> Option<Token> {
        let patterns = patterns
            .into_iter()
            .map(IntoPattern::into_pattern)
            .collect_vec();
        if patterns.is_empty() {
            None
        } else {
            Some(self.insert_patterns(patterns))
        }
    }
    #[track_caller]
    pub(crate) fn try_insert_range_in(
        &mut self,
        location: impl IntoPatternLocation,
        range: impl PatternRangeIndex,
    ) -> Result<Result<Token, Token>, ErrorReason> {
        let location = location.into_pattern_location();
        let vertex = self.expect_vertex(location.parent);
        vertex
            .get_child_pattern(&location.pattern_id)
            .map(|pattern| pattern.to_vec())
            .and_then(|pattern| {
                let pattern = Pattern::from(pattern);
                get_child_pattern_range(
                    &location.pattern_id,
                    &pattern,
                    range.clone(),
                )
                .and_then(|inner| {
                    if inner.is_empty() {
                        Err(ErrorReason::EmptyRange)
                    } else if inner.len() == 1 {
                        Ok(Ok(*inner.first().unwrap()))
                    } else if pattern.len() > inner.len() {
                        let c = self.insert_pattern(inner.into_pattern());
                        self.replace_in_pattern(location, range, c);
                        Ok(Ok(c))
                    } else {
                        Ok(Err(location.parent))
                    }
                })
            })
    }
    #[track_caller]
    pub(crate) fn insert_range_in(
        &mut self,
        location: impl IntoPatternLocation,
        range: impl PatternRangeIndex,
    ) -> Result<Token, ErrorReason> {
        self.try_insert_range_in(location, range)
            .and_then(|c| c.or(Err(ErrorReason::Unnecessary)))
    }
    #[track_caller]
    pub(crate) fn insert_range_in_or_default(
        &mut self,
        location: impl IntoPatternLocation,
        range: impl PatternRangeIndex,
    ) -> Result<Token, ErrorReason> {
        self.try_insert_range_in(location, range).map(|c| match c {
            Ok(c) => c,
            Err(c) => c,
        })
    }
    //#[track_caller]
    pub fn replace_in_pattern(
        &mut self,
        location: impl IntoPatternLocation,
        range: impl PatternRangeIndex,
        replace: impl IntoPattern,
    ) {
        let replace = replace.into_pattern();
        let location = location.into_pattern_location();
        let parent = location.parent;
        let parent_index = parent.vertex_index();
        let pat = location.pattern_id;
        let (replaced, width, start, new_end, rem) = {
            let vertex = self.expect_vertex_mut(parent);
            let width = vertex.width;
            let pattern = vertex.expect_child_pattern_mut(&pat);
            let _backup = pattern.clone();
            let start = range.clone().next().unwrap();
            let new_end = start + replace.len();
            let _old = pattern.clone();
            let replaced = replace_in_pattern(
                pattern.as_vec_mut(),
                range.clone(),
                replace.clone(),
            );
            let rem =
                pattern.iter().skip(new_end).cloned().collect::<Pattern>();
            vertex.validate();
            (replaced, width, start, new_end, rem)
        };
        let old_end = start + replaced.len();
        range.clone().zip(replaced).for_each(|(pos, c)| {
            let c = self.expect_vertex_mut(c);
            c.remove_parent_index(parent_index, pat, pos);
        });
        for c in rem.into_iter().unique() {
            let c = self.expect_vertex_mut(c);
            let indices =
                &mut c.expect_parent_mut(parent_index).pattern_indices;
            *indices = indices
                .drain()
                .filter(|i| {
                    i.pattern_id != pat || !range.clone().contains(&i.sub_index)
                })
                .map(|i| {
                    if i.pattern_id == pat && i.sub_index >= old_end {
                        PatternIndex::new(
                            i.pattern_id,
                            i.sub_index - old_end + new_end,
                        )
                    } else {
                        i
                    }
                })
                .collect();
            if indices.is_empty() {
                c.remove_parent(parent_index);
            }
        }
        self.add_pattern_parent(
            Token::new(parent_index, width),
            replace,
            pat,
            start,
        );
        self.validate_expansion(parent_index);
    }
    pub(crate) fn add_pattern_parent(
        &mut self,
        parent: impl ToToken,
        pattern: impl IntoPattern,
        pattern_id: PatternId,
        start: usize,
    ) {
        pattern
            .into_pattern()
            .into_iter()
            .enumerate()
            .for_each(|(pos, c)| {
                let pos = start + pos;
                let c = self.expect_vertex_mut(c.to_child());
                c.add_parent(ChildLocation::new(
                    parent.to_child(),
                    pattern_id,
                    pos,
                ));
            });
    }
    pub(crate) fn append_to_pattern(
        &mut self,
        parent: impl crate::graph::vertex::has_vertex_index::ToToken,
        pattern_id: PatternId,
        new: impl IntoIterator<
            Item = impl crate::graph::vertex::has_vertex_index::ToToken,
        >,
    ) -> Token {
        let new: Vec<_> = new.into_iter().map(|c| c.to_child()).collect();
        if new.is_empty() {
            return parent.to_child();
        }
        let width = pattern_width(&new);
        let (offset, width) = {
            // Todo: use smart pointers to reference data in the graph
            // so we can mutate multiple different nodes at the same time
            let vertex = self.expect_vertex(parent.vertex_index());
            let pattern = vertex.expect_child_pattern(&pattern_id).clone();
            for c in pattern.into_iter().collect::<HashSet<_>>() {
                let c = self.expect_vertex_mut(c.to_child());
                c.get_parent_mut(parent.vertex_index()).unwrap().width += width;
            }
            let vertex = self.expect_vertex_mut(parent.vertex_index());
            let pattern = vertex.expect_child_pattern_mut(&pattern_id);
            let offset = pattern.len();
            pattern.extend(new.iter());
            vertex.width += width;
            (offset, vertex.width)
        };
        let parent = Token::new(parent.vertex_index(), width);
        self.add_pattern_parent(parent, new, pattern_id, offset);
        parent
    }
    pub(crate) fn new_atom_indices(
        &mut self,
        sequence: impl IntoIterator<Item = G::Atom>,
    ) -> NewAtomIndices {
        sequence
            .into_iter()
            .map(Atom::Element)
            .map(|t| match self.get_atom_index(t) {
                Ok(i) => NewAtomIndex::Known(i),
                Err(_) => {
                    let i = self.insert_atom(t);
                    NewAtomIndex::New(i.index)
                },
            })
            .collect()
    }
}
