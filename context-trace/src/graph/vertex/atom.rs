use std::{
    borrow::Borrow,
    fmt::{
        self,
        Debug,
        Display,
    },
    hash::Hash,
};

use petgraph::graph::EdgeIndex;
use serde::{
    Deserialize,
    Serialize,
};

use crate::graph::vertex::{
    token::TokenWidth,
    wide::Wide,
};

pub fn atomizing_iter<T: Atomize, C: AsAtom<T>>(
    seq: impl Iterator<Item = C>
) -> impl Iterator<Item = Atom<T>> {
    seq.map(|c| c.as_atom())
}

/// Trait for atom that can be mapped in a sequence
pub trait Atomize:
    AtomData
    + Wide
    + Hash
    + Eq
    + Copy
    + Debug
    + Send
    + Sync
    + 'static
    + Unpin
    + Serialize
{
    fn atomize<T: AsAtom<Self>, I: Iterator<Item = T>>(
        seq: I
    ) -> Vec<Atom<Self>> {
        let mut v = vec![];
        v.extend(atomizing_iter(seq));
        //v.push(Atom::End);
        v
    }
    fn into_atom(self) -> Atom<Self> {
        Atom::Element(self)
    }
}

impl<
    T: AtomData
        + Wide
        + Hash
        + Eq
        + Copy
        + Debug
        + Send
        + Sync
        + 'static
        + Unpin
        + Serialize,
> Atomize for T
{
}

pub trait AtomData: Debug + PartialEq + Clone + Wide {}

impl<T: Debug + PartialEq + Clone + Wide> AtomData for T {}

#[allow(dead_code)]
#[derive(Hash, Debug, Clone, PartialEq, Eq, Copy)]
pub(crate) struct NoAtom;

impl Wide for NoAtom {
    fn width(&self) -> TokenWidth {
        TokenWidth(0)
    }
}

#[derive(Clone, Debug, Copy, PartialEq, Eq)]
pub(crate) enum NewAtomIndex {
    New(crate::graph::vertex::VertexIndex),
    Known(crate::graph::vertex::VertexIndex),
}

#[allow(dead_code)]
impl NewAtomIndex {
    pub(crate) fn is_known(&self) -> bool {
        matches!(self, Self::Known(_))
    }
    pub(crate) fn is_new(&self) -> bool {
        matches!(self, Self::New(_))
    }
}

impl Wide for NewAtomIndex {
    fn width(&self) -> TokenWidth {
        TokenWidth(1)
    }
}

impl crate::graph::vertex::has_vertex_index::HasVertexIndex for NewAtomIndex {
    fn vertex_index(&self) -> crate::graph::vertex::VertexIndex {
        match self {
            Self::New(i) => *i,
            Self::Known(i) => *i,
        }
    }
}

impl Borrow<crate::graph::vertex::VertexIndex> for &'_ NewAtomIndex {
    fn borrow(&self) -> &crate::graph::vertex::VertexIndex {
        match self {
            NewAtomIndex::New(i) => i,
            NewAtomIndex::Known(i) => i,
        }
    }
}

impl Borrow<crate::graph::vertex::VertexIndex> for &'_ mut NewAtomIndex {
    fn borrow(&self) -> &crate::graph::vertex::VertexIndex {
        match self {
            NewAtomIndex::New(i) => i,
            NewAtomIndex::Known(i) => i,
        }
    }
}

pub(crate) type NewAtomIndices = Vec<NewAtomIndex>;

pub trait AsAtom<T: Atomize> {
    fn as_atom(&self) -> Atom<T>;
}

impl<T: Atomize> AsAtom<T> for &'_ Atom<T> {
    fn as_atom(&self) -> Atom<T> {
        (*self).as_atom()
    }
}
impl<T: Atomize> AsAtom<T> for Atom<T> {
    fn as_atom(&self) -> Atom<T> {
        *self
    }
}

impl<T: Atomize> AsAtom<T> for T {
    fn as_atom(&self) -> Atom<T> {
        Atom::Element(*self)
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub(crate) struct CtxInfo<T: Atomize> {
    pub(crate) atom: Atom<T>,
    pub(crate) incoming_groups: Vec<Vec<Atom<T>>>,
    pub(crate) outgoing_groups: Vec<Vec<Atom<T>>>,
}

#[allow(dead_code)]
pub(crate) trait CtxLink: Sized + Clone {
    fn index(&self) -> &EdgeIndex;
    fn into_index(self) -> EdgeIndex {
        *self.index()
    }
}

impl CtxLink for EdgeIndex {
    fn index(&self) -> &EdgeIndex {
        self
    }
}

#[allow(dead_code)]
pub(crate) trait CtxMapping<E: CtxLink> {
    /// Get distance groups for incoming edges
    fn incoming(&self) -> &Vec<E>;
    fn outgoing(&self) -> &Vec<E>;

    ///// Get distance groups for incoming edges
    //fn incoming_distance_groups(
    //    &self,
    //    graph: &SequenceGraph<T>,
    //) -> Vec<Vec<Self::Ctx>> {
    //    graph.distance_group_source_weights(self.incoming().iter().map(|e| e.into_index()))
    //}
    ///// Get distance groups for outgoing edges
    //fn outgoing_distance_groups(
    //    &self,
    //    graph: &SequenceGraph<T>,
    //) -> Vec<Vec<Self::Ctx>> {
    //    graph.distance_group_target_weights(self.outgoing().iter().map(|e| e.into_index()))
    //}
}

#[allow(dead_code)]
pub(crate) trait AtomCtx<T: Atomize, E: CtxLink>: Sized {
    type Mapping: CtxMapping<E>;
    fn atom(&self) -> &Atom<T>;
    fn into_atom(self) -> Atom<T>;
    fn map_to_atoms(groups: Vec<Vec<Self>>) -> Vec<Vec<Atom<T>>> {
        groups
            .into_iter()
            .map(|g| g.into_iter().map(|m| m.into_atom()).collect())
            .collect()
    }
    fn mapping(&self) -> &Self::Mapping;
    fn mapping_mut(&mut self) -> &mut Self::Mapping;
    //fn get_info(&self, graph: &SequenceGraph<T>) -> CtxInfo<T> {
    //    let mut incoming_groups = self.mapping().incoming_distance_groups(graph);
    //    incoming_groups.reverse();
    //    let outgoing_groups = self.mapping().outgoing_distance_groups(graph);
    //    CtxInfo {
    //        atom: self.atom().clone(),
    //        incoming_groups: Self::map_to_atoms(incoming_groups),
    //        outgoing_groups: Self::map_to_atoms(outgoing_groups),
    //    }
    //}
}

#[allow(dead_code)]
pub(crate) fn groups_to_string<
    T: Atomize,
    E: CtxLink,
    C: AtomCtx<T, E> + Display,
>(
    groups: Vec<Vec<C>>
) -> String {
    let mut lines = Vec::new();
    let max = groups.iter().map(Vec::len).max().unwrap_or(0);
    for i in 0..max {
        let mut line = Vec::new();
        for group in &groups {
            line.push(group.get(i).map(ToString::to_string));
        }
        lines.push(line);
    }
    lines.iter().fold(String::new(), |a, line| {
        format!(
            "{}{}\n",
            a,
            line.iter().fold(String::new(), |a, elem| {
                format!("{}{} ", a, elem.clone().unwrap_or_default())
            })
        )
    })
}

/// Type for storing elements of a sequence
#[derive(Copy, Debug, PartialEq, Clone, Eq, Hash, Serialize, Deserialize)]
pub enum Atom<T: Atomize = char> {
    Element(T),
    Start,
    End,
}

impl<T: Atomize + Display> Display for Atom<T> {
    fn fmt(
        &self,
        f: &mut fmt::Formatter,
    ) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Atom::Element(t) => t.to_string(),
                Atom::Start => "START".to_string(),
                Atom::End => "END".to_string(),
            }
        )
    }
}

impl<T: Atomize + Display> crate::logging::compact_format::CompactFormat for Atom<T> {
    fn fmt_compact(
        &self,
        f: &mut fmt::Formatter,
    ) -> fmt::Result {
        match self {
            Atom::Element(t) => write!(f, "{}", t),
            Atom::Start => write!(f, "START"),
            Atom::End => write!(f, "END"),
        }
    }

    fn fmt_indented(
        &self,
        f: &mut fmt::Formatter,
        _indent: usize,
    ) -> fmt::Result {
        self.fmt_compact(f)
    }
}


impl<T: Atomize> Wide for Atom<T> {
    fn width(&self) -> TokenWidth {
        match self {
            Atom::Element(t) => t.width(),
            Atom::Start => TokenWidth(0),
            Atom::End => TokenWidth(0),
        }
    }
}

impl<T: Atomize> From<T> for Atom<T> {
    fn from(e: T) -> Self {
        Atom::Element(e)
    }
}

impl<T: Atomize> PartialEq<T> for Atom<T> {
    fn eq(
        &self,
        rhs: &T,
    ) -> bool {
        match self {
            Atom::Element(e) => *e == *rhs,
            _ => false,
        }
    }
}
