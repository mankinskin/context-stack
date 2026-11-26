//! The stage of an overlay read: [`StagingGraph`] with its staged vertices
//! and the copy-on-write deltas of committed ones.

use std::{
    collections::{
        BTreeMap,
        BTreeSet,
        HashMap,
    },
    hash::{
        BuildHasherDefault,
        Hasher,
    },
};

use either::Either;

use crate::{
    AtomOf,
    AtomPosition,
    BaseGraphKind,
    GraphKind,
    TokenWidth,
    VertexKey,
    graph::vertex::{
        CommittedIndex,
        StageIndex,
        VertexId as _,
        VertexIndex,
    },
    positioned::{
        committed::PositionedGraph,
        model::{
            Composition,
            PositionedVertex,
            VertexNode,
        },
        stage::Translation,
    },
};

/// Parent vertex → offsets at which a vertex occurs in it.
pub(super) type Links<I> = BTreeMap<I, BTreeSet<AtomPosition>>;

/// Multiplicative hash (as FxHash) for the stage's maps, which are looked
/// up on every access to a committed vertex once the stage changed one;
/// keys are vertex slots and atoms, not adversarial.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct StageHasher(u64);

impl StageHasher {
    fn add(
        &mut self,
        word: u64,
    ) {
        self.0 = (self.0.rotate_left(5) ^ word)
            .wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
}

impl Hasher for StageHasher {
    fn write(
        &mut self,
        bytes: &[u8],
    ) {
        for &byte in bytes {
            self.add(u64::from(byte));
        }
    }
    fn write_u32(
        &mut self,
        word: u32,
    ) {
        self.add(u64::from(word));
    }
    fn write_u64(
        &mut self,
        word: u64,
    ) {
        self.add(word);
    }
    fn write_usize(
        &mut self,
        word: usize,
    ) {
        self.add(word as u64);
    }
    fn finish(&self) -> u64 {
        self.0
    }
}

pub(super) type StageMap<K, V> = HashMap<K, V, BuildHasherDefault<StageHasher>>;

pub(super) fn has_link<I: Ord>(
    links: &Links<I>,
    parent: &I,
    offset: AtomPosition,
) -> bool {
    links
        .get(parent)
        .is_some_and(|offsets| offsets.contains(&offset))
}

pub(super) fn insert_link<I: Ord>(
    links: &mut Links<I>,
    parent: I,
    offset: AtomPosition,
) {
    links.entry(parent).or_default().insert(offset);
}

/// Whether the link existed.
pub(super) fn remove_link<I: Ord>(
    links: &mut Links<I>,
    parent: &I,
    offset: AtomPosition,
) -> bool {
    let Some(offsets) = links.get_mut(parent) else {
        return false;
    };
    let removed = offsets.remove(&offset);
    if offsets.is_empty() {
        links.remove(parent);
    }
    removed
}

/// A composite created in the stage (atoms are committed at once).
#[derive(Clone, Debug)]
pub struct StagedVertex<G: GraphKind = BaseGraphKind> {
    pub(super) width: TokenWidth,
    pub(super) node: VertexNode<AtomOf<G>, StageIndex>,
    pub(super) parents: Links<StageIndex>,
    #[cfg(debug_assertions)]
    pub(super) label: String,
}

/// Changes of one committed vertex. Content is immutable, so only the
/// composition shape and the parent links change. `added` holds no base
/// link and `removed` only base links.
#[derive(Clone, Debug, Default)]
pub(super) struct BaseDelta {
    pub(super) composition: Option<Composition<StageIndex>>,
    pub(super) added: Links<StageIndex>,
    pub(super) removed: Links<CommittedIndex>,
}

/// The staged part of an overlay (staging plan §4.1).
#[derive(Clone, Debug)]
pub struct StagingGraph<G: GraphKind = BaseGraphKind> {
    /// By [`StagedIndex`](crate::graph::vertex::StagedIndex), in slot order
    /// (not creation order: removals and stabilizations move vertices); the
    /// first [`Self::stable`] are stable.
    pub(super) vertices: Vec<StagedVertex<G>>,
    /// Length of the stable slot prefix (cheap stabilization plan
    /// 20261126): stable vertices are never removed.
    pub(super) stable: usize,
    /// Copy-on-write compositions and link deltas of committed vertices.
    pub(super) deltas: StageMap<CommittedIndex, BaseDelta>,
    /// Generation of the base when the stage was opened.
    pub(super) base_generation: u64,
    /// Bumped by the first mutation of every edit of the stage.
    pub(super) generation: u64,
}

impl<G: GraphKind> StagingGraph<G> {
    pub(super) fn new(base_generation: u64) -> Self {
        Self {
            vertices: Vec::new(),
            stable: 0,
            deltas: StageMap::default(),
            base_generation,
            generation: 0,
        }
    }
    /// The stage's parent links of `child`: a staged child's own links, a
    /// committed child's added links (links to staged parents live only
    /// there); `None` for a committed child without delta.
    pub(super) fn links_mut(
        &mut self,
        child: StageIndex,
    ) -> Option<&mut Links<StageIndex>> {
        match child.split() {
            Either::Left(committed) =>
                self.deltas.get_mut(&committed).map(|delta| &mut delta.added),
            Either::Right(staged) =>
                Some(&mut self.vertices[staged.slot()].parents),
        }
    }
    /// The stage's composition of `parent`: a staged composite's own, a
    /// committed vertex's copy-on-write composition (a committed vertex
    /// references a staged child only through it); `None` otherwise.
    pub(super) fn composition_mut(
        &mut self,
        parent: StageIndex,
    ) -> Option<&mut Composition<StageIndex>> {
        match parent.split() {
            Either::Left(committed) => self
                .deltas
                .get_mut(&committed)
                .and_then(|delta| delta.composition.as_mut()),
            Either::Right(staged) =>
                match &mut self.vertices[staged.slot()].node {
                    VertexNode::Composite(composition) => Some(composition),
                    VertexNode::Atom(_) => None,
                },
        }
    }
    /// Rename every reference to the staged `vertices` after they changed
    /// slots: `vertices` are their new indices, `rename` maps every old index
    /// to its new one (a permutation, identity elsewhere). The references
    /// are the parent links of their children (staged: own links;
    /// committed: the delta's `added`) and the compositions of their parents
    /// (staged: own node; committed: the delta's composition). The sites are
    /// collected first and each is renamed once, so vertices referencing
    /// each other are renamed correctly.
    pub(super) fn rename(
        &mut self,
        vertices: &[StageIndex],
        rename: impl Fn(StageIndex) -> StageIndex,
    ) {
        let mut children = BTreeSet::new();
        let mut parents = BTreeSet::new();
        for vertex in vertices {
            let staged = &self.vertices[vertex
                .staged()
                .expect("only staged vertices move")
                .slot()];
            if let VertexNode::Composite(composition) = &staged.node {
                children.extend(composition.children());
            }
            parents.extend(staged.parents.keys().copied());
        }
        for child in children {
            let links = self
                .links_mut(rename(child))
                .expect("a child links to its staged parent");
            *links = std::mem::take(links)
                .into_iter()
                .map(|(parent, offsets)| (rename(parent), offsets))
                .collect();
        }
        for parent in parents {
            let composition = self
                .composition_mut(rename(parent))
                .expect("a parent of a staged vertex has a staged composition");
            *composition = composition.map_index(&rename);
        }
    }
    /// Push every staged vertex and apply every delta to `base`
    /// (staging plan §4.3). The base must be the one the stage was opened
    /// on, unchanged.
    pub(super) fn commit_into(
        self,
        base: &mut PositionedGraph<G>,
    ) -> Translation {
        let first = base.vertices.len();
        let translation = Translation::table(
            (first..first + self.vertices.len())
                .map(VertexIndex)
                .collect(),
        );
        let links = |links: Links<StageIndex>| -> Links<CommittedIndex> {
            links
                .into_iter()
                .map(|(parent, offsets)| (translation.index(parent), offsets))
                .collect()
        };
        base.vertices.reserve(self.vertices.len());
        for (vertex, &index) in self.vertices.into_iter().zip(translation.slots()) {
            let VertexNode::Composite(composition) = vertex.node else {
                unreachable!("atoms are committed through OverlayHandle::atom")
            };
            let node = VertexNode::Composite(
                composition.map_index(|child| translation.index(child)),
            );
            base.vertices.push(PositionedVertex {
                index,
                key: VertexKey::default(),
                width: vertex.width,
                node,
                parents: links(vertex.parents),
                #[cfg(debug_assertions)]
                label: vertex.label,
            });
        }
        for (index, delta) in self.deltas {
            let vertex = &mut base.vertices[index.0];
            if let Some(composition) = delta.composition {
                vertex.node = VertexNode::Composite(
                    composition.map_index(|child| translation.index(child)),
                );
            }
            for (parent, offsets) in delta.removed {
                for offset in offsets {
                    remove_link(&mut vertex.parents, &parent, offset);
                }
            }
            for (parent, offsets) in links(delta.added) {
                vertex.parents.entry(parent).or_default().extend(offsets);
            }
        }
        base.generation += self.generation;
        translation
    }
}
