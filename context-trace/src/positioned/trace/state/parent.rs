//! Parent states (legacy `trace::state::parent`).

use std::{
    cmp::Ordering,
    collections::VecDeque,
    ops::Range,
};

use derive_more::{
    Deref,
    DerefMut,
};

use crate::{
    AtomPosition,
    DirectedKey,
    GraphKind,
    Token,
    UpKey,
    UpPosition,
    graph::vertex::wide::Wide,
    positioned::{
        graph::PositionedGraph,
        path::{
            EndPath,
            RangePath,
            StartPath,
        },
        trace::{
            QueryAnchor,
            cache::key::{
                RootKey,
                TargetKey,
            },
            child::state::{
                ChildState,
                RootChildState,
            },
            state::StateAdvance,
        },
    },
};

#[derive(Debug, Clone, Deref, DerefMut, Default)]
pub struct ParentBatch {
    pub parents: VecDeque<ParentState>,
}

/// A match that ends at the end of the root's entry child: the start path
/// is matched from the match start to `span.end`.
///
/// - `span`: matched atoms in absolute offsets of the root; `span.end` is
///   the end of the root entry child.
/// - `root_pos`: query position at `span.end`.
/// - `prev_pos`: `root_pos` before the last raise; equals `root_pos` for
///   seeds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParentState {
    pub prev_pos: UpPosition,
    pub root_pos: UpPosition,
    pub span: Range<AtomPosition>,
    pub path: StartPath,
}

impl ParentState {
    /// Seed: `start`, at query position `query_pos`, occurs in `parent` at
    /// `offset`.
    pub fn new(
        start: Token,
        parent: Token,
        offset: impl Into<AtomPosition>,
        query_pos: impl Into<AtomPosition>,
    ) -> Self {
        let offset = offset.into();
        let root_pos = UpPosition(query_pos.into() + start.width().0);
        Self {
            prev_pos: root_pos,
            root_pos,
            span: offset..offset + start.width().0,
            path: StartPath::new(parent, offset, start),
        }
    }
    pub fn root(&self) -> Token {
        self.path.root()
    }
    /// Root offsets ↔ query positions of this match.
    pub fn anchor(&self) -> QueryAnchor {
        QueryAnchor {
            root: self.span.start,
            query: AtomPosition(self.root_pos.0.0 - (self.span.end.0 - self.span.start.0)),
        }
    }
    /// Raise into `parent`, where the root occurs at `offset`.
    ///
    /// Only called when the match reaches the root end (as legacy). Span
    /// `[s, e)` becomes `[o + s, o + width(root))`, i.e. `[o + s, o + e)` for
    /// a state at the root end; for [`ChildState::parent_state`] of an
    /// exhausted root the matched rest after the entry child is added, as
    /// legacy adds the rest of the row. `prev_pos` takes the old `root_pos`.
    /// Canonical start path (2.4): if the match starts at the old root's
    /// start (`s == 0`), the path becomes `[(parent, o, root)]`, otherwise
    /// the step is prepended.
    pub fn path_raise(
        &mut self,
        parent: Token,
        offset: impl Into<AtomPosition>,
    ) {
        let offset = offset.into();
        let width = self.root().width().0;
        self.prev_pos = self.root_pos;
        self.root_pos += width - self.span.end.0;
        let at_root_start = self.span.start.0 == 0;
        self.span = offset + self.span.start.0..offset + width;
        self.path.raise(parent, offset, at_root_start);
    }
}

impl StateAdvance for ParentState {
    type Next = RootChildState;
    /// The next graph token: [`PositionedGraph::start_at`]`(root,
    /// span.end)`. `Err(self)` at the root end (then the state is raised).
    fn advance_state<G: GraphKind>(
        self,
        graph: &PositionedGraph<G>,
    ) -> Result<Self::Next, Self> {
        let Some(end) = EndPath::start_at(graph, self.root(), self.span.end) else {
            return Err(self);
        };
        let anchor = self.anchor();
        let entry_pos = anchor.up(self.path.entry_end());
        let exit_pos = anchor.down(end.root_exit().offset);
        Ok(RootChildState {
            child_state: ChildState {
                entry_pos,
                exit_pos,
                start_pos: self.prev_pos.0,
                path: RangePath::new(self.span.clone(), self.path.clone(), end),
            },
            root_parent: self,
        })
    }
}

impl RootKey for ParentState {
    fn root_key(&self) -> UpKey {
        UpKey::new(self.root(), self.root_pos)
    }
}

impl TargetKey for ParentState {
    fn target_key(&self) -> DirectedKey {
        self.root_key().into()
    }
}

impl Ord for ParentState {
    fn cmp(
        &self,
        other: &Self,
    ) -> Ordering {
        self.root().cmp(&other.root())
    }
}

impl PartialOrd for ParentState {
    fn partial_cmp(
        &self,
        other: &Self,
    ) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
