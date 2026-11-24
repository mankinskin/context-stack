//! Staging implementation (b), the logical stage tag (staging plan §5): the
//! read writes into the committed graph itself, and every vertex carries its
//! [`VertexState`].
//!
//! - New composites are pushed as `Staged`; new atoms are committed at once
//!   (they are never extended, staging plan §2.2).
//! - Indices are [`StageIndex`]es whose tag bit mirrors the vertex state: a
//!   staged vertex in slot `i` is `StagedIndex(i)`, a committed one
//!   `CommittedIndex(i)`. The store keeps plain slots; the bit is derived on
//!   reading. Only the stage writes while it is open, so every composite at
//!   or after the stage's first slot is staged and every vertex there of
//!   width 1 is an atom (committed): the bit of a child or parent follows
//!   from its slot and width, without a lookup.
//! - [`StageHandle::commit`] flips the tags; its [`Translation`] only
//!   clears the bit.
//!
//! Other readers of the committed graph see staged vertices (one read at a
//! time); while a stage is open, only its views may edit the graph.

use std::{
    fmt,
    sync::RwLockWriteGuard,
};

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

/// The open tagged stage of a [`PositionedGraph`]: its first slot and the
/// graph's generation when it was opened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StageId {
    first: usize,
    generation: u64,
}

/// First slot of the open stage (no vertex is staged without one).
#[inline(always)]
fn first_slot<G: GraphKind>(graph: &PositionedGraph<G>) -> usize {
    graph.stage.map_or(usize::MAX, |stage| stage.first)
}

/// The stage index of the child of a stored occurrence.
#[inline(always)]
fn tag_child(
    first: usize,
    occurrence: &Occurrence,
) -> Occurrence<StageIndex> {
    let slot = occurrence.child.index.0;
    let staged = slot >= first && occurrence.child.width.0 > 1;
    Occurrence::new(
        occurrence.offset,
        Token::new(StageIndex::tagged(slot, staged), occurrence.child.width),
    )
}

/// The vertex in the slot of `index`. Its tag bit is only checked in debug
/// builds: the check delays every lookup on the vertex's state (about 15 %
/// of a periodic read), and the vertex object derives its index from the
/// state, not from `index`.
#[inline(always)]
fn lookup<G: GraphKind>(
    graph: &PositionedGraph<G>,
    index: StageIndex,
) -> Option<&PositionedVertex<G>> {
    let vertex = graph.vertices.get(index.slot());
    debug_assert!(
        vertex.is_none_or(|vertex| {
            (vertex.state == VertexState::Staged) == index.is_staged()
        }),
        "the tag bit of {index} disagrees with the vertex state"
    );
    vertex
}

/// [`CompositionRead`] of a tagged vertex: the stored composition with the
/// tag bits of its children derived from slot and width.
///
/// Two words (as the overlay's `Lifted`), so a [`NodeRef`] of it is
/// returned in registers.
#[derive(Clone, Copy, Debug)]
pub struct TaggedComposition<'a> {
    composition: &'a Composition,
    first: usize,
}

impl<'a> TaggedComposition<'a> {
    #[inline(always)]
    fn stored(self) -> &'a [Occurrence] {
        self.composition.occurrences()
    }
}

impl Wide for TaggedComposition<'_> {
    fn width(&self) -> TokenWidth {
        TokenWidth(slice::width(self.stored()))
    }
}

impl CompositionRead for TaggedComposition<'_> {
    type Index = StageIndex;
    fn len(self) -> usize {
        self.stored().len()
    }
    fn get(
        self,
        k: usize,
    ) -> Option<Occurrence<StageIndex>> {
        self.stored().get(k).map(|o| tag_child(self.first, o))
    }
    fn prefix(self) -> Occurrence<StageIndex> {
        tag_child(self.first, &self.stored()[0])
    }
    fn postfix(self) -> Occurrence<StageIndex> {
        tag_child(
            self.first,
            self.stored()
                .last()
                .expect("a composition has at least two occurrences"),
        )
    }
    fn covering(
        self,
        p: AtomPosition,
    ) -> Option<Occurrence<StageIndex>> {
        slice::covering(self.stored(), p).map(|o| tag_child(self.first, o))
    }
    fn ending_at(
        self,
        e: AtomPosition,
    ) -> Option<Occurrence<StageIndex>> {
        slice::ending_at(self.stored(), e).map(|o| tag_child(self.first, o))
    }
    fn crossing(
        self,
        p: AtomPosition,
    ) -> impl ExactSizeIterator<Item = Occurrence<StageIndex>> {
        slice::crossing(self.stored(), p)
            .iter()
            .map(move |o| tag_child(self.first, o))
    }
    fn occurrences(
        self
    ) -> impl DoubleEndedIterator<Item = Occurrence<StageIndex>> + ExactSizeIterator
    {
        self.stored()
            .iter()
            .map(move |o| tag_child(self.first, o))
    }
}

/// Vertex object of a [`TaggedView`]: the stored vertex and the graph (two
/// words, as the committed vertex object).
pub struct TaggedVertex<'a, G: GraphKind> {
    graph: &'a PositionedGraph<G>,
    vertex: &'a PositionedVertex<G>,
}

impl<G: GraphKind> Clone for TaggedVertex<'_, G> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<G: GraphKind> Copy for TaggedVertex<'_, G> {}

impl<'a, G: GraphKind> TaggedVertex<'a, G> {
    #[inline(always)]
    fn get(
        graph: &'a PositionedGraph<G>,
        index: StageIndex,
    ) -> Option<Self> {
        lookup(graph, index).map(|vertex| Self { graph, vertex })
    }
}

impl<G: GraphKind> fmt::Debug for TaggedVertex<'_, G> {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        write!(f, "#{}(w{})", self.vertex_index(), self.vertex.width.0)?;
        #[cfg(debug_assertions)]
        write!(f, " {:?}", self.vertex.label)?;
        Ok(())
    }
}

impl<G: GraphKind> HasVertexIndex for TaggedVertex<'_, G> {
    type Index = StageIndex;
    fn vertex_index(&self) -> StageIndex {
        StageIndex::tagged(
            self.vertex.index.0,
            self.vertex.state == VertexState::Staged,
        )
    }
}

impl<G: GraphKind> Wide for TaggedVertex<'_, G> {
    fn width(&self) -> TokenWidth {
        self.vertex.width
    }
}

impl<'a, G: GraphKind> VertexRead<'a> for TaggedVertex<'a, G> {
    type Kind = G;
    type Composition = TaggedComposition<'a>;

    #[inline(always)]
    fn node(self) -> NodeRef<'a, AtomOf<G>, TaggedComposition<'a>> {
        match &self.vertex.node {
            VertexNode::Atom(atom) => NodeRef::Atom(atom),
            VertexNode::Composite(composition) =>
                NodeRef::Composite(TaggedComposition {
                    composition,
                    first: first_slot(self.graph),
                }),
        }
    }
    fn parents(
        self
    ) -> impl Iterator<Item = (Token<StageIndex>, AtomPosition)> {
        let vertices = &self.graph.vertices;
        self.vertex
            .parents
            .iter()
            .flat_map(move |(&parent, offsets)| {
                let vertex = &vertices[parent.0];
                let parent = Token::new(
                    StageIndex::tagged(
                        parent.0,
                        vertex.state == VertexState::Staged,
                    ),
                    vertex.width,
                );
                offsets.iter().map(move |&offset| (parent, offset))
            })
    }
    fn parent_count(self) -> usize {
        self.vertex.parents.values().map(|offsets| offsets.len()).sum()
    }
    fn is_parent_at(
        self,
        parent: StageIndex,
        offset: AtomPosition,
    ) -> bool {
        self.vertex
            .parents
            .get(&VertexIndex(parent.slot()))
            .is_some_and(|offsets| offsets.contains(&offset))
    }
    fn state(self) -> VertexState {
        self.vertex.state
    }
    fn resolve(
        self,
        index: StageIndex,
    ) -> Option<Self> {
        Self::get(self.graph, index)
    }
    #[cfg(debug_assertions)]
    fn label(self) -> &'a str {
        &self.vertex.label
    }
}

/// Write handle of one tagged vertex. Vertices committed before the stage
/// opened keep their content: width and label are checked, not written.
pub struct TaggedVertexMut<'a, G: GraphKind> {
    vertex: &'a mut PositionedVertex<G>,
    fixed: bool,
}

impl<G: GraphKind> raw::VertexWrite for TaggedVertexMut<'_, G> {
    type Kind = G;
    type Index = StageIndex;
    fn set_node(
        &mut self,
        node: VertexNode<AtomOf<G>, StageIndex>,
    ) {
        self.vertex.node = match node {
            VertexNode::Composite(composition) =>
                VertexNode::Composite(composition.into_slots()),
            VertexNode::Atom(atom) => {
                assert!(
                    !self.fixed,
                    "committed vertex {} cannot become an atom",
                    self.vertex.index
                );
                VertexNode::Atom(atom)
            },
        };
    }
    fn set_width(
        &mut self,
        width: TokenWidth,
    ) {
        if self.fixed {
            assert_eq!(
                self.vertex.width, width,
                "the width of committed vertex {} is immutable",
                self.vertex.index
            );
        }
        self.vertex.width = width;
    }
    fn link_parent(
        &mut self,
        parent: StageIndex,
        offset: AtomPosition,
    ) {
        self.vertex
            .parents
            .entry(VertexIndex(parent.slot()))
            .or_default()
            .insert(offset);
    }
    fn unlink_parent(
        &mut self,
        parent: StageIndex,
        offset: AtomPosition,
    ) {
        let parent = VertexIndex(parent.slot());
        if let Some(offsets) = self.vertex.parents.get_mut(&parent) {
            offsets.remove(&offset);
            if offsets.is_empty() {
                self.vertex.parents.remove(&parent);
            }
        }
    }
    #[cfg(debug_assertions)]
    fn set_label(
        &mut self,
        label: String,
    ) {
        if self.fixed {
            debug_assert_eq!(self.vertex.label, label);
        } else {
            self.vertex.label = label;
        }
    }
}

/// The store of a read with a tagged stage: the committed graph
/// write-locked, with its stage open.
pub struct TaggedView<'a, G: GraphKind = BaseGraphKind> {
    graph: RwLockWriteGuard<'a, PositionedGraph<G>>,
    stage: StageId,
}

impl<G: GraphKind> TaggedView<'_, G> {
    /// Snapshot of the labels; staged indices resolve to the label of their
    /// slot.
    #[cfg(debug_assertions)]
    pub fn name_resolver(&self) -> TaggedLabels {
        TaggedLabels(
            self.graph.vertices.iter().map(|v| v.label.clone()).collect(),
        )
    }
    /// Install [`Self::name_resolver`] for the current thread.
    #[cfg(all(debug_assertions, any(test, feature = "test-api")))]
    pub fn register_name_resolver(&self) {
        crate::graph::vertex::name_resolver::set_token_name_resolver(Box::new(
            self.name_resolver(),
        ));
    }
}

/// Labels of a tagged graph by slot, for committed and staged indices.
#[cfg(debug_assertions)]
#[derive(Clone, Debug)]
pub struct TaggedLabels(Vec<String>);

#[cfg(debug_assertions)]
impl crate::graph::vertex::name_resolver::TokenNameResolver for TaggedLabels {
    fn token_name(
        &self,
        index: usize,
    ) -> Option<String> {
        self.0.get(index).cloned()
    }
    fn staged_token_name(
        &self,
        slot: usize,
    ) -> Option<String> {
        self.0.get(slot).cloned()
    }
}

impl<G: GraphKind> GraphRead for TaggedView<'_, G> {
    type Kind = G;
    type Index = StageIndex;
    type Vertex<'a>
        = TaggedVertex<'a, G>
    where
        Self: 'a;

    fn vertex(
        &self,
        index: impl HasVertexIndex<Index = StageIndex>,
    ) -> Option<TaggedVertex<'_, G>> {
        TaggedVertex::get(&self.graph, index.vertex_index())
    }
    fn atom(
        &self,
        value: &AtomOf<G>,
    ) -> Option<TaggedVertex<'_, G>> {
        let &index = self.graph.atoms.get(value)?;
        self.vertex(StageIndex::lift(index))
    }
    fn vertices(&self) -> impl Iterator<Item = TaggedVertex<'_, G>> + '_ {
        let graph = &*self.graph;
        graph
            .vertices
            .iter()
            .map(move |vertex| TaggedVertex { graph, vertex })
    }
    fn generation(&self) -> Generation {
        Generation {
            base: self.stage.generation,
            stage: self.graph.generation - self.stage.generation,
        }
    }
}

impl<G: GraphKind> raw::Write for TaggedView<'_, G> {
    type VertexMut<'a>
        = TaggedVertexMut<'a, G>
    where
        Self: 'a;

    fn vertex_mut(
        &mut self,
        _: Seal,
        index: StageIndex,
    ) -> Option<TaggedVertexMut<'_, G>> {
        lookup(&self.graph, index)?;
        let fixed = index.slot() < self.stage.first;
        Some(TaggedVertexMut {
            vertex: &mut self.graph.vertices[index.slot()],
            fixed,
        })
    }
    /// The index of the next composite (atoms are committed at once).
    fn next_index(
        &self,
        _: Seal,
    ) -> StageIndex {
        StageIndex::tagged(self.graph.vertices.len(), true)
    }
    fn push(
        &mut self,
        _: Seal,
        node: VertexNode<AtomOf<G>, StageIndex>,
        width: TokenWidth,
    ) -> StageIndex {
        let slot = self.graph.vertices.len();
        let (node, state) = match node {
            VertexNode::Atom(atom) =>
                (VertexNode::Atom(atom), VertexState::Committed),
            VertexNode::Composite(composition) => (
                VertexNode::Composite(composition.into_slots()),
                VertexState::Staged,
            ),
        };
        self.graph.vertices.push(PositionedVertex {
            index: VertexIndex(slot),
            key: VertexKey::default(),
            width,
            state,
            #[cfg(debug_assertions)]
            label: String::new(),
            node,
            parents: Default::default(),
        });
        StageIndex::tagged(slot, state == VertexState::Staged)
    }
    fn register_atom(
        &mut self,
        _: Seal,
        value: AtomOf<G>,
        index: StageIndex,
    ) {
        let committed = index
            .committed()
            .unwrap_or_else(|| panic!("atom {index} is staged"));
        self.graph.atoms.insert(value, committed);
    }
    fn bump_generation(
        &mut self,
        _: Seal,
    ) {
        self.graph.generation += 1;
    }
}

impl<G: GraphKind> Stage for TaggedView<'_, G> {
    fn staged(&self) -> impl Iterator<Item = StagedIndex> + '_ {
        self.graph.vertices[self.stage.first..]
            .iter()
            .filter(|vertex| vertex.state == VertexState::Staged)
            .map(|vertex| StagedIndex::new(vertex.index.0))
    }
}

impl<G: GraphKind> HasGraph for TaggedView<'_, G> {
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

impl<G: GraphKind> HasGraphMut for TaggedView<'_, G> {
    type GuardMut<'a>
        = &'a mut Self
    where
        Self: 'a;
    fn graph_mut(&mut self) -> &mut Self {
        self
    }
}

/// A read's handle with a tagged stage (staging plan §5.1): the committed
/// graph and the open stage id. Views write-lock the graph per call.
///
/// A stage is always committed (insert plan D10): dropping an uncommitted
/// handle commits it as well.
pub struct TaggedHandle<'g, G: GraphKind = BaseGraphKind> {
    graph: &'g PositionedGraphRef<G>,
    stage: StageId,
    open: bool,
}

impl<'g, G: GraphKind> TaggedHandle<'g, G> {
    /// Open a stage on `graph`.
    ///
    /// # Panics
    /// If `graph` has an open stage.
    pub fn open(graph: &'g PositionedGraphRef<G>) -> Self {
        let mut locked = graph.write();
        assert!(locked.stage.is_none(), "a tagged stage is open already");
        let stage = StageId {
            first: locked.vertices.len(),
            generation: locked.generation,
        };
        locked.stage = Some(stage);
        drop(locked);
        Self {
            graph,
            stage,
            open: true,
        }
    }
    fn view(&self) -> TaggedView<'_, G> {
        let graph = self.graph.write();
        debug_assert_eq!(graph.stage, Some(self.stage), "foreign stage");
        TaggedView {
            graph,
            stage: self.stage,
        }
    }
    /// Number of staged vertices.
    pub fn staged_count(&self) -> usize {
        self.view().staged().count()
    }
    /// Flip the tags of the staged vertices and close the stage.
    fn close(&mut self) -> Translation {
        self.open = false;
        let mut graph = self.graph.write();
        let mut staged = 0;
        for vertex in &mut graph.vertices[self.stage.first..] {
            if vertex.state == VertexState::Staged {
                vertex.state = VertexState::Committed;
                staged += 1;
            }
        }
        graph.stage = None;
        Translation::untag(staged)
    }
}

impl<G: GraphKind> Drop for TaggedHandle<'_, G> {
    fn drop(&mut self) {
        if self.open {
            self.close();
        }
    }
}

impl<G: GraphKind> HasGraph for TaggedHandle<'_, G> {
    type Kind = G;
    type Index = StageIndex;
    type Guard<'a>
        = TaggedView<'a, G>
    where
        Self: 'a;
    fn graph(&self) -> TaggedView<'_, G> {
        self.view()
    }
}

impl<G: GraphKind> HasGraphMut for TaggedHandle<'_, G> {
    type GuardMut<'a>
        = TaggedView<'a, G>
    where
        Self: 'a;
    fn graph_mut(&mut self) -> TaggedView<'_, G> {
        self.view()
    }
}

impl<G: GraphKind> StageHandle for TaggedHandle<'_, G> {
    type View<'a>
        = TaggedView<'a, G>
    where
        Self: 'a;
    fn stage(&mut self) -> TaggedView<'_, G> {
        self.view()
    }
    /// Never fails: the stage writes into the committed graph directly.
    fn commit(
        mut self,
        root: Token<StageIndex>,
    ) -> Result<(Token<CommittedIndex>, Translation), CommitError> {
        let translation = self.close();
        Ok((translation.token(root), translation))
    }
}
