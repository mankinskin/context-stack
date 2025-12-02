use crate::{
    compare::state::CompareState,
    cursor::{
        Candidate,
        Checkpointed,
        ChildCursor,
        HasCandidate,
        PatternCursor,
    },
};
use context_trace::*;
use derive_more::{
    Deref,
    DerefMut,
};
use std::{
    fmt::Debug,
    marker::PhantomData,
};
use tracing::debug;

use crate::compare::state::PathPairMode::GraphMajor;
use context_trace::trace::cache::key::directed::down::{
    DownKey,
    DownPosition,
};
#[derive(Clone, Debug, PartialEq, Eq, Deref, DerefMut)]
pub(crate) struct CompareRootState {
    #[deref]
    #[deref_mut]
    pub(crate) candidate:
        CompareState<Candidate, Candidate, PositionAnnotated<ChildLocation>>,
    pub(crate) root_parent: ParentState,
}

#[derive(Clone, Debug, Deref, DerefMut)]
pub(crate) struct ParentCompareState {
    #[deref]
    #[deref_mut]
    pub(crate) parent_state: ParentState,
    pub(crate) cursor: Checkpointed<PatternCursor<Candidate>, HasCandidate>,
}

#[context_trace::instrument_trait_impl]
impl StateAdvance for ParentCompareState {
    type Next = CompareRootState;
    #[context_trace::instrument_sig(level = "debug", skip(self, trav), fields(parent_state=%self.parent_state, cursor=%self.cursor))]
    fn advance_state<G: HasGraph>(
        self,
        trav: &G,
    ) -> Result<Self::Next, Self> {
        match self.parent_state.advance_state(trav) {
            Ok(next) => {
                // Keep the cursor as a range path to properly track start/end positions
                debug!(
                    child_cursor=%next.child_state,
                    "Created child_cursor from parent_state"
                );
                // Get the token that index_cursor points to
                let index_token = next
                    .child_state
                    .role_leaf_token::<End, _>(trav)
                    .expect("parent should have valid end token");

                // Clone and simplify the path, then convert to position-annotated
                let mut simplified_path = next.child_state.path.clone();
                simplified_path.child_path_mut::<Start, _>().simplify(trav);
                simplified_path.child_path_mut::<End, _>().simplify(trav);

                // Convert to position-annotated path for both working cursor and checkpoint
                let annotated_path = simplified_path
                    .with_positions(next.child_state.exit_pos.0, trav);
                let child_state = ChildState {
                    entry_pos: next.child_state.entry_pos,
                    exit_pos: next.child_state.exit_pos,
                    start_pos: next.child_state.start_pos,
                    path: annotated_path.clone(),
                };

                let cursor_position = self.cursor.candidate().atom_position;

                Ok(CompareRootState {
                    candidate: CompareState {
                        query: self.cursor,
                        child: Checkpointed {
                            checkpoint: ChildCursor {
                                child_state: child_state.clone(),
                                _state: PhantomData,
                            },
                            candidate: ChildCursor {
                                child_state,
                                _state: PhantomData,
                            },
                            _state: PhantomData,
                        },
                        mode: GraphMajor,
                        target: DownKey::new(
                            index_token,
                            DownPosition(cursor_position),
                        ),
                    },
                    root_parent: next.root_parent,
                })
            },
            Err(parent_state) => Err(Self {
                parent_state,
                cursor: self.cursor,
            }),
        }
    }
}
