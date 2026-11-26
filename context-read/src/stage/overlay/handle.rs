//! [`OverlayHandle`]: a read's handle with an overlay stage.

use context_insert::edit::EditExt as _;
use std::cell::RefCell;

use super::{
    OverlayView,
    StagingGraph,
};
use crate::{
    stage::{
        CommitError,
        StageHandle,
        Translation,
    },
};
use context_trace::{
    AtomOf,
    BaseGraphKind,
    graph::{
        vertex::{
            CommittedIndex,
            StagedIndex,
            StageIndex,
        },
    },
    GraphKind,
    HasVertexIndex,
    positioned::{
        GraphRead,
        PositionedGraphRef,
        HasGraph,
        HasGraphMut,
        VertexNode,
    },
    Token,
};

/// A read's handle with an overlay stage (staging plan §4.1): the
/// committed graph and the stage. Views lock the base for reading and
/// borrow the stage per call (one view at a time; the stage belongs to one
/// read, so it needs no lock); only [`Self::atom`] (new atoms) and
/// [`StageHandle::commit`] write the base.
pub struct OverlayHandle<'g, G: GraphKind = BaseGraphKind> {
    base: &'g PositionedGraphRef<G>,
    stage: RefCell<StagingGraph<G>>,
}

impl<'g, G: GraphKind> OverlayHandle<'g, G> {
    /// Open an empty stage over `base`.
    pub fn open(base: &'g PositionedGraphRef<G>) -> Self {
        let generation = base.read().generation().base;
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
    /// The atom for `value`. A new atom is created in the committed graph at
    /// once (staged vertices never are atoms), which requires `&mut self`:
    /// no view may be alive.
    ///
    /// The stage stays bound to the base generation it saw last, so a commit
    /// still detects edits by others.
    ///
    /// # Errors
    /// [`CommitError::BaseChanged`] if the base was edited since the stage
    /// was opened (or since its last own atom).
    pub fn atom(
        &mut self,
        value: AtomOf<G>,
    ) -> Result<Token<StageIndex>, CommitError> {
        let known = self.base.read().atom(&value).map(|vertex| vertex.vertex_index());
        if let Some(index) = known {
            return Ok(Token::new(StageIndex::lift(index), 1));
        }
        let mut base = self.base.write();
        let stage = self.stage.get_mut();
        if base.generation().base != stage.base_generation {
            return Err(CommitError::BaseChanged {
                opened: stage.base_generation,
                now: base.generation().base,
            });
        }
        let mut edit = base.edit();
        let token = edit.atom(value);
        edit.finish();
        stage.base_generation = base.generation().base;
        Ok(Token::new(StageIndex::lift(token.index), token.width))
    }
    /// Number of live staged vertices.
    pub fn staged_count(&self) -> usize {
        self.view().stage.live().count()
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
        if base.generation().base != stage.base_generation {
            return Err(CommitError::BaseChanged {
                opened: stage.base_generation,
                now: base.generation().base,
            });
        }
        for (slot, vertex) in stage.live() {
            let index = StageIndex::from(StagedIndex::new(slot));
            let count: usize =
                vertex.parents.values().map(|offsets| offsets.len()).sum();
            if index != root.index
                && matches!(vertex.node, VertexNode::Composite(_))
                && count < 2
            {
                panic!(
                    "invalid vertex set: staged {index} has {count} parent occurrences"
                );
            }
        }
        let translation = stage.commit_into(&mut base);
        Ok((translation.token(root), translation))
    }
}
