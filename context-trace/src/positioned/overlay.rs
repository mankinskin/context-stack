//! Staging implementation (a), the physical overlay (staging plan §4): a
//! [`StagingGraph`] next to the committed graph. The base is read-only while
//! the stage is open; [`StageHandle::commit`] writes everything at once.
//!
//! - Staged vertices live in the stage, addressed by [`StagedIndex`]; atoms
//!   new in the read are staged as well (atom lookup checks the stage first).
//! - Changes of committed vertices (insertion into committed hosts) are a
//!   copy-on-write composition and a parent-link delta per vertex.
//! - [`OverlayView`] (base read guard and stage guard) is the store the
//!   read works on; index type [`StageIndex`], vertex object
//!   [`OverlayVertex`]. Compositions are read as [`Lifted`] slices in the
//!   stage's index space; unchanged committed ones in place, without
//!   copying.

use std::{
    cell::{
        RefCell,
        RefMut,
    },
    collections::{
        BTreeMap,
        BTreeSet,
        HashMap,
    },
    fmt,
    hash::{
        BuildHasherDefault,
        Hasher,
    },
    sync::RwLockReadGuard,
};

use either::Either;

use super::{
    access::{
        CompositionRead,
        Generation,
        GraphRead,
        NodeRef,
        VertexRead,
        VertexState,
        raw::{
            self,
            Seal,
        },
    },
    composition::{
        Composition,
        Occurrence,
        slice,
    },
    graph::{
        PositionedGraph,
        PositionedGraphRef,
        PositionedVertex,
        VertexNode,
    },
    has_graph::{
        HasGraph,
        HasGraphMut,
    },
    stage::{
        CommitError,
        Stage,
        StageHandle,
        Translation,
    },
};
use crate::{
    AtomOf,
    AtomPosition,
    BaseGraphKind,
    GraphKind,
    Token,
    TokenWidth,
    VertexKey,
    graph::vertex::{
        CommittedIndex,
        StageIndex,
        StagedIndex,
        VertexId,
        VertexIndex,
        has_vertex_index::HasVertexIndex,
        wide::Wide,
    },
};

/// Parent vertex → offsets at which a vertex occurs in it.
type Links<I> = BTreeMap<I, BTreeSet<AtomPosition>>;

/// Multiplicative hash (as FxHash) for the stage's maps, which are looked
/// up on every access to a committed vertex once the stage changed one;
/// keys are vertex slots and atoms, not adversarial.
#[derive(Clone, Copy, Debug, Default)]
struct StageHasher(u64);

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

type StageMap<K, V> = HashMap<K, V, BuildHasherDefault<StageHasher>>;

fn has_link<I: Ord>(
    links: &Links<I>,
    parent: &I,
    offset: AtomPosition,
) -> bool {
    links
        .get(parent)
        .is_some_and(|offsets| offsets.contains(&offset))
}

fn insert_link<I: Ord>(
    links: &mut Links<I>,
    parent: I,
    offset: AtomPosition,
) {
    links.entry(parent).or_default().insert(offset);
}

/// Whether the link existed.
fn remove_link<I: Ord>(
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
    width: TokenWidth,
    node: VertexNode<AtomOf<G>, StageIndex>,
    parents: Links<StageIndex>,
    #[cfg(debug_assertions)]
    label: String,
}

/// Changes of one committed vertex. Content is immutable, so only the
/// composition shape and the parent links change. `added` holds no base
/// link and `removed` only base links.
#[derive(Clone, Debug, Default)]
struct BaseDelta {
    composition: Option<Composition<StageIndex>>,
    added: Links<StageIndex>,
    removed: Links<CommittedIndex>,
}

/// The staged part of an overlay (staging plan §4.1).
#[derive(Clone, Debug)]
pub struct StagingGraph<G: GraphKind = BaseGraphKind> {
    /// By [`StagedIndex`], in creation order.
    vertices: Vec<StagedVertex<G>>,
    /// Atoms new in this stage.
    atoms: StageMap<AtomOf<G>, StagedIndex>,
    /// Copy-on-write compositions and link deltas of committed vertices.
    deltas: StageMap<CommittedIndex, BaseDelta>,
    /// Generation of the base when the stage was opened.
    base_generation: u64,
    /// Bumped by the first mutation of every edit of the stage.
    generation: u64,
}

impl<G: GraphKind> StagingGraph<G> {
    fn new(base_generation: u64) -> Self {
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
    fn commit_into(
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

/// [`CompositionRead`] of an overlay vertex in the stage's index space: a
/// staged composition, the copy-on-write composition of a committed host,
/// or an unchanged committed composition read in place (child indices
/// lifted without copying, [`Composition::lifted`]).
#[derive(Clone, Copy, Debug)]
pub struct Lifted<'a>(&'a [Occurrence<StageIndex>]);

impl<'a> Lifted<'a> {
    fn base(composition: &'a Composition<CommittedIndex>) -> Self {
        Self(composition.lifted())
    }
    fn stage(composition: &'a Composition<StageIndex>) -> Self {
        Self(composition.occurrences())
    }
}

impl Wide for Lifted<'_> {
    fn width(&self) -> TokenWidth {
        TokenWidth(slice::width(self.0))
    }
}

impl CompositionRead for Lifted<'_> {
    type Index = StageIndex;
    fn len(self) -> usize {
        self.0.len()
    }
    fn get(
        self,
        k: usize,
    ) -> Option<Occurrence<StageIndex>> {
        self.0.get(k).copied()
    }
    fn prefix(self) -> Occurrence<StageIndex> {
        self.0[0]
    }
    fn postfix(self) -> Occurrence<StageIndex> {
        *self
            .0
            .last()
            .expect("a composition has at least two occurrences")
    }
    fn covering(
        self,
        p: AtomPosition,
    ) -> Option<Occurrence<StageIndex>> {
        slice::covering(self.0, p).copied()
    }
    fn ending_at(
        self,
        e: AtomPosition,
    ) -> Option<Occurrence<StageIndex>> {
        slice::ending_at(self.0, e).copied()
    }
    fn crossing(
        self,
        p: AtomPosition,
    ) -> impl ExactSizeIterator<Item = Occurrence<StageIndex>> {
        slice::crossing(self.0, p).iter().copied()
    }
    fn occurrences(
        self
    ) -> impl DoubleEndedIterator<Item = Occurrence<StageIndex>> + ExactSizeIterator
    {
        self.0.iter().copied()
    }
}

/// The stored vertex behind an overlay index.
enum Entry<'a, G: GraphKind> {
    Base(&'a PositionedVertex<G>),
    Staged(&'a StagedVertex<G>),
}

/// Vertex object of an [`OverlayView`]: the view and a known index (two
/// words, as the committed vertex object; the stored vertex is looked up
/// per access, which is cheaper than copying it around).
pub struct OverlayVertex<'a, G: GraphKind> {
    view: &'a OverlayView<'a, G>,
    index: StageIndex,
}

impl<G: GraphKind> Clone for OverlayVertex<'_, G> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<G: GraphKind> Copy for OverlayVertex<'_, G> {}

impl<'a, G: GraphKind> OverlayVertex<'a, G> {
    #[inline(always)]
    fn get(
        view: &'a OverlayView<'a, G>,
        index: StageIndex,
    ) -> Option<Self> {
        let known = match index.split() {
            Either::Left(committed) => committed.0 < view.base.vertices.len(),
            Either::Right(staged) => staged.slot() < view.stage.vertices.len(),
        };
        known.then_some(Self { view, index })
    }
    #[inline(always)]
    fn entry(self) -> Entry<'a, G> {
        match self.index.split() {
            Either::Left(committed) =>
                Entry::Base(&self.view.base.vertices[committed.0]),
            Either::Right(staged) =>
                Entry::Staged(&self.view.stage.vertices[staged.slot()]),
        }
    }
    /// The changes of this committed vertex, if any.
    #[inline(always)]
    fn delta(
        self,
        vertex: &PositionedVertex<G>,
    ) -> Option<&'a BaseDelta> {
        let deltas = &self.view.stage.deltas;
        if deltas.is_empty() {
            None
        } else {
            deltas.get(&vertex.index)
        }
    }
    fn width_of(
        self,
        index: StageIndex,
    ) -> TokenWidth {
        match index.split() {
            Either::Left(committed) =>
                self.view.base.vertices[committed.0].width,
            Either::Right(staged) =>
                self.view.stage.vertices[staged.slot()].width,
        }
    }
}

impl<G: GraphKind> fmt::Debug for OverlayVertex<'_, G> {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        write!(f, "#{}(w{})", self.index, self.width().0)?;
        #[cfg(debug_assertions)]
        write!(f, " {:?}", self.label())?;
        Ok(())
    }
}

impl<G: GraphKind> HasVertexIndex for OverlayVertex<'_, G> {
    type Index = StageIndex;
    fn vertex_index(&self) -> StageIndex {
        self.index
    }
}

impl<G: GraphKind> Wide for OverlayVertex<'_, G> {
    fn width(&self) -> TokenWidth {
        match self.entry() {
            Entry::Base(vertex) => vertex.width,
            Entry::Staged(vertex) => vertex.width,
        }
    }
}

impl<'a, G: GraphKind> VertexRead<'a> for OverlayVertex<'a, G> {
    type Kind = G;
    type Composition = Lifted<'a>;

    #[inline(always)]
    fn node(self) -> NodeRef<'a, AtomOf<G>, Lifted<'a>> {
        match self.entry() {
            Entry::Base(vertex) => match &vertex.node {
                VertexNode::Atom(atom) => NodeRef::Atom(atom),
                VertexNode::Composite(composition) => NodeRef::Composite(
                    match self
                        .delta(vertex)
                        .and_then(|delta| delta.composition.as_ref())
                    {
                        Some(changed) => Lifted::stage(changed),
                        None => Lifted::base(composition),
                    },
                ),
            },
            Entry::Staged(vertex) => match &vertex.node {
                VertexNode::Atom(atom) => NodeRef::Atom(atom),
                VertexNode::Composite(composition) =>
                    NodeRef::Composite(Lifted::stage(composition)),
            },
        }
    }
    fn parents(
        self
    ) -> impl Iterator<Item = (Token<StageIndex>, AtomPosition)> {
        let token =
            move |parent: StageIndex| Token::new(parent, self.width_of(parent));
        let flatten = move |(&parent, offsets): (
            &StageIndex,
            &'a BTreeSet<AtomPosition>,
        )| {
            offsets.iter().map(move |&offset| (token(parent), offset))
        };
        match self.entry() {
            Entry::Base(vertex) => {
                let delta = self.delta(vertex);
                let removed = delta.map(|delta| &delta.removed);
                let base = vertex.parents.iter().flat_map(
                    move |(&parent, offsets)| {
                        offsets
                            .iter()
                            .filter(move |&&offset| {
                                !removed.is_some_and(|removed| {
                                    has_link(removed, &parent, offset)
                                })
                            })
                            .map(move |&offset| {
                                (token(StageIndex::lift(parent)), offset)
                            })
                    },
                );
                let added = delta.into_iter().flat_map(move |delta| {
                    delta.added.iter().flat_map(flatten)
                });
                Either::Left(base.chain(added))
            },
            Entry::Staged(vertex) =>
                Either::Right(vertex.parents.iter().flat_map(flatten)),
        }
    }
    fn parent_count(self) -> usize {
        match self.entry() {
            Entry::Base(vertex) if self.delta(vertex).is_none() =>
                vertex.parents.values().map(BTreeSet::len).sum(),
            Entry::Staged(vertex) =>
                vertex.parents.values().map(BTreeSet::len).sum(),
            Entry::Base(_) => self.parents().count(),
        }
    }
    fn is_parent_at(
        self,
        parent: StageIndex,
        offset: AtomPosition,
    ) -> bool {
        match self.entry() {
            Entry::Base(vertex) => {
                let delta = self.delta(vertex);
                let in_base = parent.committed().is_some_and(|committed| {
                    has_link(&vertex.parents, &committed, offset)
                        && !delta.is_some_and(|delta| {
                            has_link(&delta.removed, &committed, offset)
                        })
                });
                in_base
                    || delta.is_some_and(|delta| {
                        has_link(&delta.added, &parent, offset)
                    })
            },
            Entry::Staged(vertex) => has_link(&vertex.parents, &parent, offset),
        }
    }
    fn state(self) -> VertexState {
        if self.index.is_staged() {
            VertexState::Staged
        } else {
            VertexState::Committed
        }
    }
    fn resolve(
        self,
        index: StageIndex,
    ) -> Option<Self> {
        Self::get(self.view, index)
    }
    #[cfg(debug_assertions)]
    fn label(self) -> &'a str {
        match self.entry() {
            Entry::Base(vertex) => &vertex.label,
            Entry::Staged(vertex) => &vertex.label,
        }
    }
}

/// Write handle of one overlay vertex: staged vertices are written
/// directly, committed ones through their delta (the base is never
/// touched before the commit).
pub struct OverlayVertexMut<'a, G: GraphKind>(VertexMut<'a, G>);

enum VertexMut<'a, G: GraphKind> {
    Base {
        vertex: &'a PositionedVertex<G>,
        deltas: &'a mut StageMap<CommittedIndex, BaseDelta>,
    },
    Staged(&'a mut StagedVertex<G>),
}

impl<G: GraphKind> VertexMut<'_, G> {
    fn delta<'d>(
        deltas: &'d mut StageMap<CommittedIndex, BaseDelta>,
        vertex: &PositionedVertex<G>,
    ) -> &'d mut BaseDelta {
        deltas.entry(vertex.index).or_default()
    }
}

impl<G: GraphKind> raw::VertexWrite for OverlayVertexMut<'_, G> {
    type Kind = G;
    type Index = StageIndex;
    fn set_node(
        &mut self,
        node: VertexNode<AtomOf<G>, StageIndex>,
    ) {
        match &mut self.0 {
            VertexMut::Base { vertex, deltas } => {
                let VertexNode::Composite(composition) = node else {
                    panic!(
                        "committed vertex {} cannot become an atom",
                        vertex.index
                    );
                };
                VertexMut::delta(deltas, vertex).composition =
                    Some(composition);
            },
            VertexMut::Staged(vertex) => vertex.node = node,
        }
    }
    fn set_width(
        &mut self,
        width: TokenWidth,
    ) {
        match &mut self.0 {
            VertexMut::Base { vertex, .. } => assert_eq!(
                vertex.width, width,
                "the width of committed vertex {} is immutable",
                vertex.index
            ),
            VertexMut::Staged(vertex) => vertex.width = width,
        }
    }
    fn link_parent(
        &mut self,
        parent: StageIndex,
        offset: AtomPosition,
    ) {
        match &mut self.0 {
            VertexMut::Base { vertex, deltas } => {
                let delta = VertexMut::delta(deltas, vertex);
                let in_base = parent.committed().is_some_and(|committed| {
                    remove_link(&mut delta.removed, &committed, offset)
                        || has_link(&vertex.parents, &committed, offset)
                });
                if !in_base {
                    insert_link(&mut delta.added, parent, offset);
                }
            },
            VertexMut::Staged(vertex) =>
                insert_link(&mut vertex.parents, parent, offset),
        }
    }
    fn unlink_parent(
        &mut self,
        parent: StageIndex,
        offset: AtomPosition,
    ) {
        match &mut self.0 {
            VertexMut::Base { vertex, deltas } => {
                let delta = VertexMut::delta(deltas, vertex);
                if remove_link(&mut delta.added, &parent, offset) {
                    return;
                }
                if let Some(committed) = parent.committed()
                    && has_link(&vertex.parents, &committed, offset)
                {
                    insert_link(&mut delta.removed, committed, offset);
                }
            },
            VertexMut::Staged(vertex) => {
                remove_link(&mut vertex.parents, &parent, offset);
            },
        }
    }
    /// Labels of committed vertices are fixed with their content.
    #[cfg(debug_assertions)]
    fn set_label(
        &mut self,
        label: String,
    ) {
        match &mut self.0 {
            VertexMut::Base { vertex, .. } =>
                debug_assert_eq!(vertex.label, label),
            VertexMut::Staged(vertex) => vertex.label = label,
        }
    }
}

/// The store of a read with an overlay stage: the base read-locked (also
/// for writes, which go into the stage) and the stage.
pub struct OverlayView<'a, G: GraphKind = BaseGraphKind> {
    base: RwLockReadGuard<'a, PositionedGraph<G>>,
    stage: RefMut<'a, StagingGraph<G>>,
}

impl<G: GraphKind> OverlayView<'_, G> {
    /// Snapshot of the labels of base and stage (staged indices resolve
    /// through [`TokenNameResolver::staged_token_name`]).
    ///
    /// [`TokenNameResolver::staged_token_name`]: crate::graph::vertex::name_resolver::TokenNameResolver::staged_token_name
    #[cfg(debug_assertions)]
    pub fn name_resolver(&self) -> OverlayLabels {
        OverlayLabels {
            base: self.base.vertices.iter().map(|v| v.label.clone()).collect(),
            staged: self
                .stage
                .vertices
                .iter()
                .map(|v| v.label.clone())
                .collect(),
        }
    }
    /// Install [`Self::name_resolver`] for the current thread. Staged
    /// labels change at commit: reinstall the base's resolver afterwards.
    #[cfg(all(debug_assertions, any(test, feature = "test-api")))]
    pub fn register_name_resolver(&self) {
        crate::graph::vertex::name_resolver::set_token_name_resolver(Box::new(
            self.name_resolver(),
        ));
    }
}

/// Labels of an overlay, resolving committed and staged indices.
#[cfg(debug_assertions)]
#[derive(Clone, Debug)]
pub struct OverlayLabels {
    base: Vec<String>,
    staged: Vec<String>,
}

#[cfg(debug_assertions)]
impl crate::graph::vertex::name_resolver::TokenNameResolver for OverlayLabels {
    fn token_name(
        &self,
        index: usize,
    ) -> Option<String> {
        self.base.get(index).cloned()
    }
    fn staged_token_name(
        &self,
        slot: usize,
    ) -> Option<String> {
        self.staged.get(slot).cloned()
    }
}

impl<G: GraphKind> GraphRead for OverlayView<'_, G> {
    type Kind = G;
    type Index = StageIndex;
    type Vertex<'a>
        = OverlayVertex<'a, G>
    where
        Self: 'a;

    fn vertex(
        &self,
        index: impl HasVertexIndex<Index = StageIndex>,
    ) -> Option<OverlayVertex<'_, G>> {
        OverlayVertex::get(self, index.vertex_index())
    }
    fn atom(
        &self,
        value: &AtomOf<G>,
    ) -> Option<OverlayVertex<'_, G>> {
        let index = match self.stage.atoms.get(value) {
            Some(&staged) => staged.into(),
            None => StageIndex::from(*self.base.atoms.get(value)?),
        };
        self.vertex(index)
    }
    fn vertices(&self) -> impl Iterator<Item = OverlayVertex<'_, G>> + '_ {
        (0..self.base.vertices.len())
            .map(|slot| StageIndex::lift(VertexIndex(slot)))
            .chain(
                (0..self.stage.vertices.len())
                    .map(|slot| StagedIndex::new(slot).into()),
            )
            .map(move |index| OverlayVertex { view: self, index })
    }
    fn generation(&self) -> Generation {
        Generation {
            base: self.stage.base_generation,
            stage: self.stage.generation,
        }
    }
}

impl<G: GraphKind> raw::Write for OverlayView<'_, G> {
    type VertexMut<'a>
        = OverlayVertexMut<'a, G>
    where
        Self: 'a;

    fn vertex_mut(
        &mut self,
        _: Seal,
        index: StageIndex,
    ) -> Option<OverlayVertexMut<'_, G>> {
        let stage = &mut *self.stage;
        match index.split() {
            Either::Left(committed) =>
                Some(OverlayVertexMut(VertexMut::Base {
                    vertex: self.base.vertices.get(committed.0)?,
                    deltas: &mut stage.deltas,
                })),
            Either::Right(staged) => stage
                .vertices
                .get_mut(staged.slot())
                .map(|vertex| OverlayVertexMut(VertexMut::Staged(vertex))),
        }
    }
    fn next_index(
        &self,
        _: Seal,
    ) -> StageIndex {
        StagedIndex::new(self.stage.vertices.len()).into()
    }
    fn push(
        &mut self,
        _: Seal,
        node: VertexNode<AtomOf<G>, StageIndex>,
        width: TokenWidth,
    ) -> StageIndex {
        let index = StagedIndex::new(self.stage.vertices.len());
        self.stage.vertices.push(StagedVertex {
            width,
            node,
            parents: Links::new(),
            #[cfg(debug_assertions)]
            label: String::new(),
        });
        index.into()
    }
    fn register_atom(
        &mut self,
        _: Seal,
        value: AtomOf<G>,
        index: StageIndex,
    ) {
        let staged = index
            .staged()
            .unwrap_or_else(|| panic!("atom {index} is committed already"));
        self.stage.atoms.insert(value, staged);
    }
    fn bump_generation(
        &mut self,
        _: Seal,
    ) {
        self.stage.generation += 1;
    }
}

impl<G: GraphKind> Stage for OverlayView<'_, G> {
    fn staged(&self) -> impl Iterator<Item = StagedIndex> + '_ {
        (0..self.stage.vertices.len()).map(StagedIndex::new)
    }
}

impl<G: GraphKind> HasGraph for OverlayView<'_, G> {
    type Kind = G;
    type Index = StageIndex;
    type Guard<'a>
        = &'a Self
    where
        Self: 'a;
    fn graph(&self) -> &Self {
        self
    }
}

impl<G: GraphKind> HasGraphMut for OverlayView<'_, G> {
    type GuardMut<'a>
        = &'a mut Self
    where
        Self: 'a;
    fn graph_mut(&mut self) -> &mut Self {
        self
    }
}

/// A read's handle with an overlay stage (staging plan §4.1): the
/// committed graph and the stage. Views lock the base for reading and
/// borrow the stage per call (one view at a time; the stage belongs to one
/// read, so it needs no lock); only [`StageHandle::commit`] writes the base.
pub struct OverlayHandle<'g, G: GraphKind = BaseGraphKind> {
    base: &'g PositionedGraphRef<G>,
    stage: RefCell<StagingGraph<G>>,
}

impl<'g, G: GraphKind> OverlayHandle<'g, G> {
    /// Open an empty stage over `base`.
    pub fn open(base: &'g PositionedGraphRef<G>) -> Self {
        let generation = {
            let base = base.read();
            assert!(base.stage.is_none(), "a tagged stage is open");
            base.generation
        };
        Self {
            base,
            stage: RefCell::new(StagingGraph::new(generation)),
        }
    }
    /// # Panics
    /// If another view of this handle is alive.
    fn view(&self) -> OverlayView<'_, G> {
        OverlayView {
            base: self.base.read(),
            stage: self.stage.borrow_mut(),
        }
    }
    /// Number of staged vertices.
    pub fn staged_count(&self) -> usize {
        self.view().stage.vertices.len()
    }
}

impl<G: GraphKind> HasGraph for OverlayHandle<'_, G> {
    type Kind = G;
    type Index = StageIndex;
    type Guard<'a>
        = OverlayView<'a, G>
    where
        Self: 'a;
    fn graph(&self) -> OverlayView<'_, G> {
        self.view()
    }
}

impl<G: GraphKind> HasGraphMut for OverlayHandle<'_, G> {
    type GuardMut<'a>
        = OverlayView<'a, G>
    where
        Self: 'a;
    fn graph_mut(&mut self) -> OverlayView<'_, G> {
        self.view()
    }
}

impl<G: GraphKind> StageHandle for OverlayHandle<'_, G> {
    type View<'a>
        = OverlayView<'a, G>
    where
        Self: 'a;
    fn stage(&mut self) -> OverlayView<'_, G> {
        self.view()
    }
    fn commit(
        self,
        root: Token<StageIndex>,
    ) -> Result<(Token<CommittedIndex>, Translation), CommitError> {
        let stage = self.stage.into_inner();
        let mut base = self.base.write();
        if base.generation != stage.base_generation {
            return Err(CommitError::BaseChanged {
                opened: stage.base_generation,
                now: base.generation,
            });
        }
        let translation = stage.commit_into(&mut base);
        Ok((translation.token(root), translation))
    }
}
