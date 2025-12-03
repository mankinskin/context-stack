use std::marker::PhantomData;

use super::core::{
    CompareParentBatch,
    RootCursor,
};
use crate::{
    compare::{
        parent::ParentCompareState,
        state::CompareState,
    },
    cursor::{
        Candidate,
        MarkMatchState,
        Matched,
        PathCursor,
    },
    policy::{
        policy::DirectedTraversalPolicy,
        SearchKind,
    },
    state::{
        end::PathCoverage,
        matched::MatchResult,
    },
};
use context_trace::{
    path::RolePathUtils,
    End,
    Start,
    *,
};
use tracing::debug;

impl<K: SearchKind> RootCursor<K, Matched, Matched> where K::Trav: Clone {}

impl<K: SearchKind> RootCursor<K, Candidate, Candidate>
where
    K::Trav: Clone,
{
    ///// Create a MatchResult from the current candidate state based on the end reason
    //pub(crate) fn create_end_state(
    //    self,
    //    reason: EndReason,
    //) -> MatchResult {
    //    let state = *self.state;

    //    // Choose the path based on end reason
    //    // For Mismatch or ChildExhausted, use checkpoint path (state at last match)
    //    // For QueryExhausted, use current child path
    //    let mut path = match reason {
    //        EndReason::QueryExhausted =>
    //            state.child.current().child_state.path.clone(),
    //        EndReason::Mismatch | EndReason::ChildExhausted =>
    //            state.child.checkpoint().child_state.path.clone(),
    //    };

    //    // Simplify path to remove redundant segments at token borders
    //    path.child_path_mut::<Start, _>().simplify(&self.trav);
    //    path.child_path_mut::<End, _>().simplify(&self.trav);

    //    // Get the target token from the path
    //    let target_token = path.role_rooted_leaf_token::<End, _>(&self.trav);

    //    // Use entry_pos from the ChildState - it already tracks the correct position
    //    // For QueryExhausted: use current child cursor position
    //    // For Mismatch or ChildExhausted: use checkpoint position (last confirmed match)
    //    let child_state = match reason {
    //        EndReason::QueryExhausted => &state.child.current().child_state,
    //        EndReason::Mismatch | EndReason::ChildExhausted =>
    //            &state.child.checkpoint().child_state,
    //    };

    //    let _start_pos = child_state.start_pos;

    //    // root_pos is where we entered the root (beginning of the match)
    //    let root_pos = child_state.entry_pos;

    //    // target should use root_pos (where the target token starts)
    //    let target = DownKey::new(target_token, root_pos.into());

    //    // end_pos is where matching ended (checkpoint cursor's position)
    //    let end_pos = state.query.checkpoint().atom_position;

    //    // Use the non-annotated path for PathCoverage
    //    let path_enum = PathCoverage::from_range_path(
    //        path, root_pos, target, end_pos, &self.trav,
    //    );

    //    // Use checkpoint cursor (last confirmed match position)
    //    // The checkpoint already has the correct atom_position for matched tokens
    //    let result_cursor = state.query.checkpoint().clone();

    //    MatchResult {
    //        cursor: result_cursor,
    //        path: path_enum,
    //    }
    //}

    ///// Create a checkpoint Mismatch state from the current candidate state
    ///// Used when iterator completes without definitive match/mismatch - needs parent exploration
    ///// Always returns Mismatch since the root ended without completing the query
    //pub(crate) fn create_checkpoint_from_state(&self) -> MatchResult {
    //    let checkpoint = self.state.query.checkpoint();
    //    let checkpoint_child = self.state.child.checkpoint();

    //    let mut path = checkpoint_child.child_state.path.clone();

    //    // Simplify paths
    //    path.start_path_mut().simplify(&self.trav);
    //    path.end_path_mut().simplify(&self.trav);

    //    // Get the target token from the path
    //    let target_token = path.role_rooted_leaf_token::<End, _>(&self.trav);

    //    // Use entry_pos from checkpoint_child - it already has the correct position
    //    let _start_pos = checkpoint_child.child_state.start_pos;
    //    let root_pos = checkpoint_child.child_state.entry_pos;
    //    let end_pos = root_pos; // The end position is the same as root for the matched segment

    //    let target = DownKey::new(target_token, end_pos.into());

    //    let path_enum = PathCoverage::from_range_path(
    //        path, root_pos, target, end_pos, &self.trav,
    //    );

    //    // Use current query cursor (advanced beyond checkpoint when child cannot advance)
    //    // This ensures end_index points to next token to match, not last matched
    //    MatchResult {
    //        cursor: self.state.query.current().clone().mark_match(),
    //        path: path_enum,
    //    }
    //}

    ///// Convert <Candidate, Candidate> to <Candidate, Matched> for parent exploration
    //pub(crate) fn into_candidate_matched(
    //    self
    //) -> RootCursor<K, Candidate, Matched> {
    //    let state = *self.state;
    //    RootCursor {
    //        state: Box::new(CompareState {
    //            child: state.child.mark_match(),
    //            query: state.query,
    //            target: state.target,
    //            mode: state.mode,
    //        }),
    //        trav: self.trav,
    //    }
    //}
}
