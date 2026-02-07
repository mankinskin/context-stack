use context_trace::{
    graph::vertex::atom::{
        NewAtomIndex,
        NewAtomIndices,
    },
    *,
};
use derive_more::{
    Deref,
    DerefMut,
};
use itertools::Itertools;

use std::{
    fmt::Debug,
    str::Chars,
};

use crate::request::RequestInput;

pub trait ToNewAtomIndices: Debug {
    fn to_new_atom_indices<G: HasGraph<Kind = BaseGraphKind>>(
        self,
        graph: &G,
    ) -> NewAtomIndices;
}

impl ToNewAtomIndices for NewAtomIndices {
    fn to_new_atom_indices<G: HasGraph<Kind = BaseGraphKind>>(
        self,
        _graph: &G,
    ) -> NewAtomIndices {
        self
    }
}
impl ToNewAtomIndices for Chars<'_> {
    fn to_new_atom_indices<G: HasGraph<Kind = BaseGraphKind>>(
        self,
        graph: &G,
    ) -> NewAtomIndices {
        graph.graph().new_atom_indices(self)
    }
}
impl ToNewAtomIndices for RequestInput {
    fn to_new_atom_indices<G: HasGraph<Kind = BaseGraphKind>>(
        self,
        graph: &G,
    ) -> NewAtomIndices {
        match self {
            RequestInput::Text(text) => text.chars().to_new_atom_indices(graph),
            RequestInput::Pattern(pattern) =>
                pattern.to_new_atom_indices(graph),
        }
    }
}

#[derive(Debug, Deref, DerefMut, Clone)]
pub(crate) struct SegmentIter {
    iter: std::iter::Peekable<std::vec::IntoIter<NewAtomIndex>>,
}

#[derive(Debug, Clone)]
pub(crate) struct NextSegment {
    pub(crate) known: Pattern,
    pub(crate) unknown: Pattern,
}
impl Iterator for SegmentIter {
    type Item = NextSegment;
    fn next(&mut self) -> Option<Self::Item> {
        let unknown = self.next_pattern_where(|t| t.is_new());
        let known = self.next_pattern_where(|t| t.is_known());
        if unknown.is_empty() && known.is_empty() {
            None
        } else {
            Some(NextSegment { unknown, known })
        }
    }
}

impl SegmentIter {
    pub(crate) fn new(sequence: NewAtomIndices) -> Self {
        Self {
            iter: sequence.into_iter().peekable(),
        }
    }
    fn next_pattern_where(
        &mut self,
        f: impl FnMut(&NewAtomIndex) -> bool,
    ) -> Pattern {
        self.iter.peeking_take_while(f).map(Token::from).collect()
    }
}
