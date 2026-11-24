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
    Token,
    UpKey,
    UpPosition,
    graph::vertex::{
        CommittedIndex,
        VertexId,
        wide::Wide,
    },
    positioned::{
        access::GraphRead,
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

#[derive(Debug, Clone, Deref, DerefMut)]
pub struct ParentBatch<I: VertexId = CommittedIndex> {
    pub parents: VecDeque<ParentState<I>>,
}

impl<I: VertexId> Default for ParentBatch<I> {
    fn default() -> Self {
        Self {
            parents: VecDeque::new(),
        }
    }
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
pub struct ParentState<I: VertexId = CommittedIndex> {
    pub prev_pos: UpPosition,
    pub root_pos: UpPosition,
    pub span: Range<AtomPosition>,
    pub path: StartPath<I>,
}

impl<I: VertexId> ParentState<I> {
    /// Seed: `start`, at query position `query_pos`, occurs in `parent` at
    /// `offset`.
    pub fn new(
        start: Token<I>,
        parent: Token<I>,
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
    pub fn root(&self) -> Token<I> {
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
        parent: Token<I>,
        offset: impl Into<AtomPosition>,
    ) {
        let offset = offset.into();
        let width = self.root().width().0;
        self.prev_pos = self.root_pos;
        self.root_pos += width - self.span.end.0;
        self.path.raise(parent, offset, &self.span);
        self.span = offset + self.span.start.0..offset + width;
    }
}

impl<I: VertexId> StateAdvance for ParentState<I> {
    type Index = I;
    type Next = RootChildState<I>;
    /// The next graph token: [`Navigate::start_at`](crate::positioned::Navigate::start_at)`(root,
    /// span.end)`. `Err(self)` at the root end (then the state is raised).
    fn advance_state<G: GraphRead<Index = I> + ?Sized>(
        self,
        graph: &G,
    ) -> Result<Self::Next, Self> {
        let Some(end) = EndPath::start_at(graph, self.root(), self.span.end) else {
            return Err(self);
        };
        let anchor = self.anchor();
        let entry_pos = anchor.up(self.path.root_child_end());
        let exit_pos = anchor.down(end.root_child_offset());
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

impl<I: VertexId> RootKey<I> for ParentState<I> {
    fn root_key(&self) -> UpKey<I> {
        UpKey::new(self.root(), self.root_pos)
    }
}

impl<I: VertexId> TargetKey<I> for ParentState<I> {
    fn target_key(&self) -> DirectedKey<I> {
        self.root_key().into()
    }
}

impl<I: VertexId> Ord for ParentState<I> {
    fn cmp(
        &self,
        other: &Self,
    ) -> Ordering {
        self.root().cmp(&other.root())
    }
}

impl<I: VertexId> PartialOrd for ParentState<I> {
    fn partial_cmp(
        &self,
        other: &Self,
    ) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
