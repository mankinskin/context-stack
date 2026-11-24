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
        StagedIndex,
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
        store::VertexState,
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

/// A vertex created in the stage.
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
    /// By [`StagedIndex`], in creation order.
    pub(super) vertices: Vec<StagedVertex<G>>,
    /// Atoms new in this stage.
    pub(super) atoms: StageMap<AtomOf<G>, StagedIndex>,
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
            atoms: StageMap::default(),
            deltas: StageMap::default(),
            base_generation,
            generation: 0,
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
            let node = match vertex.node {
                VertexNode::Atom(atom) => {
                    base.atoms.insert(atom, index);
                    VertexNode::Atom(atom)
                },
                VertexNode::Composite(composition) => VertexNode::Composite(
                    composition.map_index(|child| translation.index(child)),
                ),
            };
            base.vertices.push(PositionedVertex {
                index,
                key: VertexKey::default(),
                width: vertex.width,
                state: VertexState::Committed,
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
