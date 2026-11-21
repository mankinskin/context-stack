//! Child states (legacy `trace::child::state`): a range path inside one
//! root, with the end path at the next graph token to compare.

use std::cmp::Ordering;

use derive_more::derive::{
    Deref,
    DerefMut,
};

use crate::{
    AtomPosition,
    DirectedKey,
    DownPosition,
    GraphKind,
    Token,
    UpKey,
    UpPosition,
    positioned::{
        graph::PositionedGraph,
        path::RangePath,
        trace::{
            QueryAnchor,
            cache::key::{
                RootKey,
                TargetKey,
            },
            state::{
                StateAdvance,
                parent::ParentState,
            },
        },
    },
};

/// A child state together with the parent state it was entered from.
#[derive(Clone, Debug, PartialEq, Eq, Deref, DerefMut)]
pub struct RootChildState {
    #[deref]
    #[deref_mut]
    pub child_state: ChildState,
    pub root_parent: ParentState,
}

/// A match inside `path.root`: `path.span` is matched, the end path points
/// at the next graph token (starting at `span.end`) or, after a match, at
/// the matched leaf.
///
/// - `entry_pos`: query position at the end of the root entry child.
/// - `exit_pos`: down position of the root child on the end path (query
///   position at its start, clamped to the match start).
/// - `start_pos`: `prev_pos` of the parent state (legacy, for tracing).
#[derive(Clone, Debug, PartialEq, Eq, Deref, DerefMut)]
pub struct ChildState {
    pub entry_pos: UpPosition,
    pub exit_pos: DownPosition,
    pub start_pos: AtomPosition,
    #[deref]
    #[deref_mut]
    pub path: RangePath,
}

impl ChildState {
    pub fn root(&self) -> Token {
        self.path.root()
    }
    /// Root offsets ↔ query positions of this match.
    pub fn anchor(&self) -> QueryAnchor {
        let entry_end = self.path.start.root_child_end();
        QueryAnchor {
            root: self.path.span.start,
            query: AtomPosition(self.entry_pos.0.0 - (entry_end.0 - self.path.span.start.0)),
        }
    }
    /// The start path as a parent state (legacy: start path and entry
    /// position; the end is dropped). Raising it adds the matched rest of
    /// the root.
    pub fn parent_state(&self) -> ParentState {
        ParentState {
            path: self.path.start.clone(),
            span: self.path.span.start..self.path.start.root_child_end(),
            prev_pos: self.entry_pos,
            root_pos: self.entry_pos,
        }
    }
}

impl StateAdvance for ChildState {
    type Next = Self;
    /// Move the end path to the token starting at `span.end` (corrected pop
    /// rule, equal to `start_at(root, span.end)`). `Err(self)` at the root
    /// end.
    fn advance_state<G: GraphKind>(
        mut self,
        graph: &PositionedGraph<G>,
    ) -> Result<Self, Self> {
        let end = self.path.span.end;
        if self.path.end.advance(graph, end).is_continue() {
            self.exit_pos = self.anchor().down(self.path.end.root_child_offset());
            Ok(self)
        } else {
            Err(self)
        }
    }
}

impl RootKey for ChildState {
    fn root_key(&self) -> UpKey {
        UpKey::new(self.root(), self.entry_pos)
    }
}

impl TargetKey for ChildState {
    fn target_key(&self) -> DirectedKey {
        self.root_key().into()
    }
}

impl Ord for ChildState {
    fn cmp(
        &self,
        other: &Self,
    ) -> Ordering {
        self.root().cmp(&other.root())
    }
}

impl PartialOrd for ChildState {
    fn partial_cmp(
        &self,
        other: &Self,
    ) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
