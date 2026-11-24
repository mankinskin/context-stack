//! [`OverlayHandle`]: a read's handle with an overlay stage.

use std::cell::RefCell;

use super::{
    OverlayView,
    StagingGraph,
};
use crate::{
    BaseGraphKind,
    GraphKind,
    Token,
    graph::vertex::{
        CommittedIndex,
        StageIndex,
    },
    positioned::{
        committed::PositionedGraphRef,
        stage::{
            CommitError,
            StageHandle,
            Translation,
        },
        store::{
            HasGraph,
            HasGraphMut,
        },
    },
};

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
