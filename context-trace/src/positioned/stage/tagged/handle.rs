//! [`TaggedHandle`]: a read's handle with a tagged stage.

use super::{
    StageId,
    TaggedView,
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
            Stage,
            StageHandle,
            Translation,
        },
        store::{
            HasGraph,
            HasGraphMut,
            VertexState,
        },
    },
};

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
