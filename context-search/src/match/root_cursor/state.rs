use super::core::{
    AdvanceCursorsResult,
    AdvanceToEndResult,
    CompareParentBatch,
    RootCursor,
};
use crate::{
    compare::{
        iterator::CompareIterator,
        parent::ParentCompareState,
        state::{
            CompareResult::*,
            CompareState,
        },
    },
    cursor::{
        Candidate,
        MarkMatchState,
        Matched,
    },
    state::{
        end::{
            EndReason,
            EndState,
            PathCoverage,
        },
        matched::MatchResult,
    },
    traversal::{
        policy::DirectedTraversalPolicy,
        SearchKind,
    },
};
use context_trace::{
    path::RolePathUtils,
    AtomPosition,
    End,
    Start,
    *,
};
use std::ops::ControlFlow::{
    self,
    Break,
    Continue,
};
use tracing::debug;

impl<K: SearchKind> Iterator for RootCursor<K, Candidate, Candidate>
where
    K::Trav: Clone,
{
    type Item = ControlFlow<EndReason>;

    fn next(&mut self) -> Option<Self::Item> {
        let prev_state = self.state.clone();

        tracing::trace!("comparing current candidate");
        // Compare the current candidate state
        match CompareIterator::<K>::new(self.trav.clone(), *self.state.clone())
            .compare()
        {
            FoundMatch(matched_state) => {
                tracing::trace!(
                    "got Match, creating Matched RootCursor to advance"
                );

                // Create a Matched RootCursor and try to advance
                let matched_cursor = RootCursor {
                    state: Box::new(matched_state),
                    trav: self.trav.clone(),
                };

                match matched_cursor.advance_both_from_match() {
                    AdvanceCursorsResult::BothAdvanced(candidate_cursor) => {
                        // Both cursors advanced - update to candidate state and continue
                        *self = candidate_cursor;
                        Some(Continue(()))
                    },
                    AdvanceCursorsResult::QueryExhausted => {
                        // Query cursor ended - QueryExhausted match
                        tracing::trace!(
                            "query pattern ended - QueryExhausted match found"
                        );
                        Some(Break(EndReason::QueryExhausted))
                    },
                    AdvanceCursorsResult::ChildExhausted(_) => {
                        // Index ended but query continues - need parent exploration
                        tracing::trace!("index ended, query continues - returning None for parent exploration");
                        None
                    },
                }
            },
            Mismatch(_) => {
                // Mismatch found - check if this is after some matches (partial match)
                // or immediate mismatch (no match)
                // The checkpoint atom_position tells us if we've made progress
                if self.state.query.checkpoint().atom_position
                    != AtomPosition::from(0)
                {
                    // We had matches before this mismatch
                    // This is a PARTIAL MATCH - the success case!
                    // This root contains the largest contiguous match
                    Some(Break(EndReason::Mismatch))
                } else {
                    // Immediate mismatch, no matches yet
                    // Revert and break to try other paths
                    self.state = prev_state;
                    Some(Break(EndReason::Mismatch))
                }
            },
            Prefixes(_) => unreachable!("compare() never returns Prefixes"),
        }
    }
}

impl<K: SearchKind> RootCursor<K, Candidate, Candidate>
where
    K::Trav: Clone,
{
    /// Iterate through candidate comparisons until reaching a conclusive end state
    ///
    /// This method drives the Iterator implementation for Candidate cursors.
    /// It repeatedly compares the current candidate state and advances cursors
    /// on successful matches until either:
    /// - A conclusive end is reached (QueryExhausted or Mismatch with progress)
    /// - The iterator completes without conclusion (needs parent exploration)
    ///
    /// Returns Completed with MatchResult if QueryExhausted or Mismatch with progress
    /// Returns NeedsParentExploration if more tokens are needed to continue
    pub(crate) fn iterate_until_conclusion(mut self) -> AdvanceToEndResult<K> {
        // Iterate until we get a match or need to stop
        loop {
            match self.next() {
                Some(Continue(())) => {
                    // Comparison resulted in match and both cursors advanced
                    // self has been updated to new candidate state, continue
                    continue;
                },
                Some(Break(reason)) => {
                    // Hit an end condition (QueryExhausted or Mismatch)
                    let checkpoint_pos =
                        *self.state.query.checkpoint().atom_position.as_ref();
                    let root_parent = self
                        .state
                        .child
                        .current()
                        .child_state
                        .path
                        .root_parent();

                    // Check if this is a valid match before destructuring
                    if matches!(
                        reason,
                        EndReason::Mismatch | EndReason::ChildExhausted
                    ) && checkpoint_pos == 0
                    {
                        // No progress - not a valid match, continue iteration
                        debug!(
                            root = %root_parent,
                            reason = ?reason,
                            "Discarding invalid match - no progress made, continuing iteration"
                        );
                        continue;
                    }

                    debug!(
                        root = %root_parent,
                        root_width = root_parent.width.0,
                        checkpoint_pos = checkpoint_pos,
                        reason = ?reason,
                        "Valid match found - creating MatchResult"
                    );

                    // Create matched end state from current state
                    return AdvanceToEndResult::Completed(
                        self.create_end_state(reason),
                    );
                },
                None => {
                    // Iterator completed without Break - need parent exploration
                    // Create checkpoint state and return cursor for parent exploration
                    let checkpoint_state = self.create_checkpoint_from_state();
                    let root_cursor = self.into_candidate_matched();
                    return AdvanceToEndResult::NeedsParentExploration {
                        checkpoint: checkpoint_state,
                        cursor: root_cursor,
                    };
                },
            }
        }
    }

    /// Create a MatchResult from the current candidate state based on the end reason
    fn create_end_state(
        self,
        reason: EndReason,
    ) -> MatchResult {
        let state = *self.state;

        // Choose the path based on end reason
        // For Mismatch or ChildExhausted, use checkpoint path (state at last match)
        // For QueryExhausted, use current child path
        let mut path = match reason {
            EndReason::QueryExhausted =>
                state.child.current().child_state.path.clone(),
            EndReason::Mismatch | EndReason::ChildExhausted =>
                state.child.checkpoint().child_state.path.clone(),
        };

        // Simplify path to remove redundant segments at token borders
        path.child_path_mut::<Start, _>().simplify(&self.trav);
        path.child_path_mut::<End, _>().simplify(&self.trav);

        // Get the target token from the path
        let target_token = path.role_rooted_leaf_token::<End, _>(&self.trav);

        // Use entry_pos from the ChildState - it already tracks the correct position
        // For QueryExhausted: use current child cursor position
        // For Mismatch or ChildExhausted: use checkpoint position (last confirmed match)
        let child_state = match reason {
            EndReason::QueryExhausted => &state.child.current().child_state,
            EndReason::Mismatch | EndReason::ChildExhausted =>
                &state.child.checkpoint().child_state,
        };

        let _start_pos = child_state.start_pos;

        // root_pos is where we entered the root (beginning of the match)
        let root_pos = child_state.entry_pos;

        // target should use root_pos (where the target token starts)
        let target = DownKey::new(target_token, root_pos.into());

        // end_pos is where matching ended (checkpoint cursor's position)
        let end_pos = state.query.checkpoint().atom_position;

        // Use the non-annotated path for PathCoverage
        let path_enum = PathCoverage::from_range_path(
            path, root_pos, target, end_pos, &self.trav,
        );

        // Use current query cursor (which may be advanced beyond checkpoint)
        // This ensures end_index points to next token to match (not last matched)
        let result_cursor = state.query.current().clone().mark_match();

        MatchResult {
            cursor: result_cursor,
            path: path_enum,
        }
    }

    /// Create a checkpoint Mismatch state from the current candidate state
    /// Used when iterator completes without definitive match/mismatch - needs parent exploration
    /// Always returns Mismatch since the root ended without completing the query
    fn create_checkpoint_from_state(&self) -> MatchResult {
        let checkpoint = self.state.query.checkpoint();
        let checkpoint_child = self.state.child.checkpoint();

        let mut path = checkpoint_child.child_state.path.clone();

        // Simplify paths
        path.start_path_mut().simplify(&self.trav);
        path.end_path_mut().simplify(&self.trav);

        // Get the target token from the path
        let target_token = path.role_rooted_leaf_token::<End, _>(&self.trav);

        // Use entry_pos from checkpoint_child - it already has the correct position
        let _start_pos = checkpoint_child.child_state.start_pos;
        let root_pos = checkpoint_child.child_state.entry_pos;
        let end_pos = root_pos; // The end position is the same as root for the matched segment

        let target = DownKey::new(target_token, end_pos.into());

        let path_enum = PathCoverage::from_range_path(
            path, root_pos, target, end_pos, &self.trav,
        );

        // Use current query cursor (advanced beyond checkpoint when child cannot advance)
        // This ensures end_index points to next token to match, not last matched
        MatchResult {
            cursor: self.state.query.current().clone().mark_match(),
            path: path_enum,
        }
    }

    /// Convert <Candidate, Candidate> to <Candidate, Matched> for parent exploration
    fn into_candidate_matched(self) -> RootCursor<K, Candidate, Matched> {
        let state = *self.state;
        RootCursor {
            state: Box::new(CompareState {
                child: state.child.mark_match(),
                query: state.query,
                target: state.target,
                mode: state.mode,
            }),
            trav: self.trav,
        }
    }
}

impl<K: SearchKind> RootCursor<K, Candidate, Matched> {
    /// Convert to Candidate state for both cursors to enable parent exploration
    pub(crate) fn into_candidate(self) -> RootCursor<K, Candidate, Candidate> {
        let state = *self.state;
        RootCursor {
            state: Box::new(CompareState {
                child: state.child.as_candidate(),
                query: state.query,
                target: state.target,
                mode: state.mode,
            }),
            trav: self.trav,
        }
    }

    pub(crate) fn get_parent_batch(
        self,
        trav: &K::Trav,
    ) -> Result<(ParentCompareState, CompareParentBatch), Box<EndState>> {
        // Convert to Candidate first, then call get_parent_batch
        self.into_candidate().get_parent_batch(trav)
    }
}

impl<K: SearchKind> RootCursor<K, Candidate, Candidate> {
    pub(crate) fn get_parent_batch(
        self,
        trav: &K::Trav,
    ) -> Result<(ParentCompareState, CompareParentBatch), Box<EndState>> {
        let parent = self.state.parent_state();
        // Note: The cursor should already be advanced by the `advanced()` method
        // before this function is called. We don't advance it again here.
        if let Some(batch) = K::Policy::next_batch(trav, &parent.parent_state) {
            let batch = CompareParentBatch {
                batch,
                cursor: parent.cursor.clone(),
            };
            Ok((parent, batch))
        } else {
            Err(Box::new(EndState::mismatch(trav, parent)))
        }
    }
}
