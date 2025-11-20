use crate::{
    compare::state::CompareState,
    cursor::{
        Candidate,
        ChildCursor,
        PathCursor,
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
    pub(crate) token: CompareState<Candidate, Candidate>,
    pub(crate) root_parent: ParentState,
}

#[derive(Clone, Debug, Deref, DerefMut)]
pub(crate) struct ParentCompareState {
    #[deref]
    #[deref_mut]
    pub(crate) parent_state: ParentState,
    pub(crate) cursor: PatternCursor,
}

#[context_trace::instrument_trait_impl]
impl StateAdvance for ParentCompareState {
    type Next = CompareRootState;
    #[context_trace::instrument_sig(skip(self, trav), fields(parent_state=%self.parent_state, cursor=%self.cursor))]
    fn advance_state<G: HasGraph>(
        self,
        trav: &G,
    ) -> Result<Self::Next, Self> {
        match self.parent_state.advance_state(trav) {
            Ok(next) => {
                // Keep the cursor as a range path to properly track start/end positions
                let cursor = PathCursor {
                    path: self.cursor.path.clone(),
                    atom_position: self.cursor.atom_position,
                    _state: PhantomData,
                };
                debug!(
                    child_cursor=%next.child_state,
                    "Created child_cursor from parent_state"
                );
                // Get the token that index_cursor points to
                let index_token = next
                    .child_state
                    .leaf_token(trav)
                    .expect("child_state should point to a valid token");
                let cursor_position = self.cursor.atom_position;

                // Clone and simplify the child state path for checkpoint_child
                let mut simplified_child_state = next.child_state.clone();
                simplified_child_state
                    .path
                    .child_path_mut::<Start>()
                    .simplify(trav);
                simplified_child_state
                    .path
                    .child_path_mut::<End>()
                    .simplify(trav);

                Ok(CompareRootState {
                    token: CompareState {
                        child_cursor: ChildCursor {
                            child_state: next.child_state.clone(),
                            _state: PhantomData,
                        },
                        cursor,
                        checkpoint: self.cursor,
                        checkpoint_child: ChildCursor {
                            child_state: simplified_child_state,
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
