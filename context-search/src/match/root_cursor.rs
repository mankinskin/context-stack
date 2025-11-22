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
        ChildCursor,
        CursorState,
        MarkMatchState,
        Matched,
        PatternCursor,
    },
    state::{
        end::{
            EndReason,
            EndState,
            PathCoverage,
        },
        matched::MatchedEndState,
    },
    traversal::{
        policy::DirectedTraversalPolicy,
        SearchKind,
    },
};
use context_trace::{
    path::RolePathUtils,
    End,
    *,
};
use tracing::debug;

use derive_more::{
    Deref,
    DerefMut,
};
use std::{
    collections::VecDeque,
    fmt::Debug,
    ops::ControlFlow::{
        self,
        Break,
        Continue,
    },
};

// Type alias for advance_cursors return type
/// Result of advancing both cursors:
/// - Ok: Both cursors advanced successfully
/// - Err: Contains EndReason and optionally a cursor needing parent exploration
pub(crate) type AdvanceCursorsResult<K> = Result<
    RootCursor<K, Candidate, Candidate>,
    (EndReason, Option<RootCursor<K, Candidate, Matched>>),
>;

#[derive(Debug)]
pub(crate) struct RootCursor<
    K: SearchKind,
    Q: CursorState = Matched,
    I: CursorState = Matched,
> {
    pub(crate) state: Box<CompareState<Q, I>>,
    pub(crate) trav: K::Trav,
}

impl<K: SearchKind> RootCursor<K, Matched, Matched>
where
    K::Trav: Clone,
{
    /// Advance through matches until we reach an end state
    /// Returns Ok(MatchedEndState) if we reach QueryExhausted or Mismatch with progress
    /// Returns Err((checkpoint_state, root_cursor)) if we need parent exploration
    pub(crate) fn advance_to_end(
        self
    ) -> Result<
        MatchedEndState,
        (MatchedEndState, RootCursor<K, Candidate, Matched>),
    > {
        let root_parent =
            self.state.child_cursor.child_state.path.root_parent();
        debug!(
            root = %root_parent,
            width = root_parent.width.0,
            checkpoint_pos = *self.state.checkpoint.atom_position.as_ref(),
            "→ advance_to_end: starting advancement for root"
        );

        // Try to advance to the next match (advance query + advance child)
        match self.advance_to_next_match() {
            Ok(candidate_cursor) => {
                let root = candidate_cursor
                    .state
                    .child_cursor
                    .child_state
                    .path
                    .root_parent();
                debug!(
                    root = %root,
                    "→ advance_to_end: got <Candidate, Candidate> cursor, calling advance_to_matched"
                );
                // We have a <Candidate, Candidate> cursor - iterate to find end
                candidate_cursor.advance_to_matched()
            },
            Err(Ok(matched_state)) => {
                debug!(
                    root = %matched_state.root_parent(),
                    "→ advance_to_end: query ended immediately (QueryExhausted)"
                );
                // Query ended immediately - return the matched state
                Ok(matched_state)
            },
            Err(Err(need_parent)) => {
                let root = need_parent
                    .state
                    .child_cursor
                    .child_state
                    .path
                    .root_parent();
                let checkpoint_pos =
                    *need_parent.state.checkpoint.atom_position.as_ref();
                debug!(
                    root = %root,
                    checkpoint_pos = checkpoint_pos,
                    "→ advance_to_end: index ended before query (need parent exploration)"
                );
                // Need parent exploration immediately (index ended before query)
                // Create checkpoint state for this root
                let checkpoint_state = need_parent.create_checkpoint_state();
                debug!(
                    checkpoint_root = %checkpoint_state.root_parent(),
                    checkpoint_width = checkpoint_state.root_parent().width.0,
                    "→ advance_to_end: created checkpoint state for parent exploration"
                );
                Err((checkpoint_state, need_parent))
            },
        }
    }

    /// Advance to the next match by: 1. advancing query cursor, 2. advancing child path
    /// Returns Ok(<Candidate, Candidate>) if both advanced successfully
    /// Returns Err(Ok(MatchedEndState)) if query ended (complete match)
    /// Returns Err(Err(<Candidate, Matched>)) if child path ended but query continues (need parent exploration)
    fn advance_to_next_match(
        self
    ) -> Result<
        RootCursor<K, Candidate, Candidate>,
        Result<MatchedEndState, RootCursor<K, Candidate, Matched>>,
    > {
        debug!("  → advance_to_next_match: Step 1 - calling advance_query");
        // Step 1: Advance query cursor
        let query_advanced = match self.advance_query() {
            Ok(cursor) => {
                let root =
                    cursor.state.child_cursor.child_state.path.root_parent();
                debug!(
                    root = %root,
                    query_pos = *cursor.state.cursor.atom_position.as_ref(),
                    "  → advance_to_next_match: Step 1 complete - query advanced successfully"
                );
                cursor
            },
            Err(matched_state) => {
                debug!(
                    root = %matched_state.root_parent(),
                    "  → advance_to_next_match: Step 1 - query ended (QueryExhausted)"
                );
                return Err(Ok(matched_state));
            },
        };

        debug!("  → advance_to_next_match: Step 2 - calling advance_child");
        // Step 2: Advance child path (index)
        match query_advanced.advance_child() {
            Ok(both_advanced) => {
                let root = both_advanced
                    .state
                    .child_cursor
                    .child_state
                    .path
                    .root_parent();
                debug!(
                    root = %root,
                    child_pos = *both_advanced.state.child_cursor.child_state.target_pos().as_ref(),
                    "  → advance_to_next_match: Step 2 complete - child advanced successfully, got <Candidate, Candidate>"
                );
                Ok(both_advanced)
            },
            Err(need_parent) => {
                let root = need_parent
                    .state
                    .child_cursor
                    .child_state
                    .path
                    .root_parent();
                debug!(
                    root = %root,
                    "  → advance_to_next_match: Step 2 - child ended (need parent exploration)"
                );
                Err(Err(need_parent))
            },
        }
    }

    /// Step 1: Advance the query cursor
    /// Returns Ok(<Candidate, Matched>) if query advanced
    /// Returns Err(MatchedEndState) if query ended (QueryExhausted)
    fn advance_query(
        self
    ) -> Result<RootCursor<K, Candidate, Matched>, MatchedEndState> {
        let root_parent =
            self.state.child_cursor.child_state.path.root_parent();
        let query_pos_before = *self.state.cursor.atom_position.as_ref();
        debug!(
            root = %root_parent,
            query_pos = query_pos_before,
            "    → advance_query: attempting to advance query cursor"
        );

        let matched_state = *self.state;
        let trav = self.trav;

        // Try to advance query cursor
        match matched_state.advance_query_cursor(&trav) {
            Ok(query_advanced) => {
                let query_pos_after =
                    *query_advanced.cursor.atom_position.as_ref();
                debug!(
                    root = %root_parent,
                    query_pos_before = query_pos_before,
                    query_pos_after = query_pos_after,
                    "    → advance_query: SUCCESS - query cursor advanced"
                );
                // Query advanced successfully
                Ok(RootCursor {
                    state: Box::new(query_advanced),
                    trav,
                })
            },
            Err(matched_state) => {
                debug!(
                    root = %root_parent,
                    query_pos = query_pos_before,
                    "    → advance_query: QUERY ENDED - creating QueryExhausted state"
                );
                // Query ended - create complete match state
                // Use query position for root_pos (not child position)
                let root_pos = matched_state.cursor.atom_position;
                let path = matched_state.child_cursor.child_state.path.clone();
                let start_pos =
                    matched_state.child_cursor.child_state.start_pos;
                let root_parent = path.root_parent();
                let target_index = path.role_rooted_leaf_token::<End, _>(&trav);
                let last_token_width_value = target_index.width();
                let end_pos = AtomPosition::from(
                    *matched_state.cursor.atom_position
                        - *last_token_width_value,
                );
                tracing::debug!(
                    "root_cursor advance_query: root_parent={}, root_pos={}, cursor.atom_position={}, last_token_width={}, end_pos={}",
                    root_parent, usize::from(root_pos), *matched_state.cursor.atom_position,
                    last_token_width_value, usize::from(end_pos)
                );

                let target = DownKey::new(target_index, root_pos.into());
                Err(MatchedEndState {
                    cursor: matched_state.checkpoint,
                    path: PathCoverage::from_range_path(
                        path, root_pos, target, end_pos, &trav,
                    ),
                })
            },
        }
    }
}

impl<K: SearchKind> RootCursor<K, Candidate, Matched> {
    /// Step 2: Advance the child path (index cursor)
    /// Returns Ok(<Candidate, Candidate>) if child advanced
    /// Returns Err(<Candidate, Matched>) if child ended but query continues (need parent exploration)
    fn advance_child(
        self
    ) -> Result<
        RootCursor<K, Candidate, Candidate>,
        RootCursor<K, Candidate, Matched>,
    > {
        let root_parent =
            self.state.child_cursor.child_state.path.root_parent();
        let child_pos_before =
            *self.state.child_cursor.child_state.target_pos().as_ref();
        debug!(
            root = %root_parent,
            child_pos = child_pos_before,
            "    → advance_child: attempting to advance child (index) cursor"
        );

        let state = *self.state;
        let trav = self.trav;

        // Try to advance index cursor
        match state.advance_index_cursor(&trav) {
            Ok(both_advanced) => {
                let child_pos_after = *both_advanced
                    .child_cursor
                    .child_state
                    .target_pos()
                    .as_ref();
                debug!(
                    root = %root_parent,
                    child_pos_before = child_pos_before,
                    child_pos_after = child_pos_after,
                    "    → advance_child: SUCCESS - child cursor advanced"
                );
                // Both cursors advanced - return Candidate cursor
                Ok(RootCursor {
                    state: Box::new(both_advanced),
                    trav,
                })
            },
            Err(query_only_advanced) => {
                debug!(
                    root = %root_parent,
                    child_pos = child_pos_before,
                    "    → advance_child: CHILD ENDED - need parent exploration"
                );
                // Index ended but query continues - need parent exploration
                Err(RootCursor {
                    state: Box::new(CompareState {
                        child_cursor: query_only_advanced.child_cursor,
                        cursor: query_only_advanced.cursor,
                        checkpoint: query_only_advanced.checkpoint,
                        checkpoint_child: query_only_advanced.checkpoint_child,
                        target: query_only_advanced.target,
                        mode: query_only_advanced.mode,
                    }),
                    trav,
                })
            },
        }
    }
}

impl<K: SearchKind> RootCursor<K, Candidate, Matched> {
    /// Create a QueryExhausted state from this root cursor's checkpoint
    /// Used when the root matched successfully but needs parent exploration
    pub(crate) fn create_checkpoint_state(&self) -> MatchedEndState {
        // Extract checkpoint information
        let checkpoint = &self.state.checkpoint;
        let checkpoint_child = &self.state.checkpoint_child;

        // Use checkpoint_child path as it represents the matched state
        let mut path = checkpoint_child.child_state.path.clone();
        let start_pos = checkpoint_child.child_state.start_pos;
        // Use query checkpoint position for root_pos (not child position)
        let root_pos = checkpoint.atom_position;

        // Simplify path to remove redundant segments
        path.child_path_mut::<Start, _>().simplify(&self.trav);
        path.child_path_mut::<End, _>().simplify(&self.trav);

        let target_index = path.role_rooted_leaf_token::<End, _>(&self.trav);
        //let last_token_width_value = target_index.width();

        let end_cursor = checkpoint.clone();
        let end_pos = checkpoint.atom_position;

        let target = DownKey::new(target_index, root_pos.into());
        let path_enum = PathCoverage::from_range_path(
            path, root_pos, target, end_pos, &self.trav,
        );

        MatchedEndState {
            cursor: end_cursor,
            path: path_enum,
        }
    }
}
impl<K: SearchKind> RootCursor<K, Matched, Matched> {
    ///// Process a matched cursor: advance and convert to either iterable candidate cursor or immediate end
    //pub(crate) fn process_match(
    //    self
    //) -> Result<RootCursor<K, Candidate, Candidate>, EndReason> {
    //    match self.advance_cursors() {
    //        Ok(candidate_cursor) => {
    //            // Both cursors advanced - can continue iterating
    //            Ok(candidate_cursor)
    //        },
    //        Err((reason, _)) => {
    //            // Could not advance
    //            Err(reason)
    //        },
    //    }
    //}

    /// Advance cursors after a match and transition to Candidate state
    /// Returns Ok with both-advanced state, or Err with reason for failure
    pub(crate) fn advance_cursors(self) -> AdvanceCursorsResult<K> {
        let matched_state = *self.state;

        // Step 1: Try to advance QUERY cursor
        match matched_state.advance_query_cursor(&self.trav) {
            Ok(query_advanced) => {
                // Step 2: Try to advance INDEX cursor
                match query_advanced.advance_index_cursor(&self.trav) {
                    Ok(both_advanced) => {
                        tracing::debug!("both cursors advanced successfully");
                        // Both cursors advanced - return as Candidate state
                        Ok(RootCursor {
                            state: Box::new(both_advanced),
                            trav: self.trav,
                        })
                    },
                    Err(_query_only_advanced) => {
                        tracing::debug!(
                            "index cursor cannot advance - graph path ended"
                        );
                        // INDEX ENDED, QUERY CONTINUES
                        // Return cursor in <Candidate, Matched> state for parent exploration
                        Err((
                            EndReason::Mismatch,
                            Some(RootCursor {
                                state: Box::new(CompareState {
                                    child_cursor: _query_only_advanced
                                        .child_cursor,
                                    cursor: _query_only_advanced.cursor,
                                    checkpoint: _query_only_advanced.checkpoint,
                                    checkpoint_child: _query_only_advanced
                                        .checkpoint_child,
                                    target: _query_only_advanced.target,
                                    mode: _query_only_advanced.mode,
                                }),
                                trav: self.trav,
                            }),
                        ))
                    },
                }
            },
            Err(_matched_state) => {
                tracing::debug!(
                    "query cursor cannot advance - query pattern ended"
                );
                // QUERY ENDED - no cursor to return
                Err((EndReason::QueryExhausted, None))
            },
        }
    }
}

impl<K: SearchKind> Iterator for RootCursor<K, Candidate, Candidate>
where
    K::Trav: Clone,
{
    type Item = ControlFlow<EndReason>;

    fn next(&mut self) -> Option<Self::Item> {
        let prev_state = self.state.clone();

        tracing::debug!("comparing current candidate");
        // Compare the current candidate state
        match CompareIterator::<K>::new(self.trav.clone(), *self.state.clone())
            .compare()
        {
            FoundMatch(matched_state) => {
                tracing::debug!(
                    "got Match, creating Matched RootCursor to advance"
                );

                // Create a Matched RootCursor and try to advance
                let matched_cursor = RootCursor {
                    state: Box::new(matched_state),
                    trav: self.trav.clone(),
                };

                match matched_cursor.advance_cursors() {
                    Ok(candidate_cursor) => {
                        // Both cursors advanced - update to candidate state and continue
                        *self = candidate_cursor;
                        Some(Continue(()))
                    },
                    Err((reason, cursor_opt)) => {
                        // Could not advance one or both cursors
                        match (reason, cursor_opt) {
                            (EndReason::QueryExhausted, None) => {
                                // Query cursor ended - QueryExhausted match
                                tracing::debug!("query pattern ended - QueryExhausted match found");
                                Some(Break(EndReason::QueryExhausted))
                            },
                            (EndReason::Mismatch, Some(_)) => {
                                // Index ended but query continues - need parent exploration
                                tracing::debug!("index ended, query continues - returning None for parent exploration");
                                None
                            },
                            _ => unreachable!("invalid advance state"),
                        }
                    },
                }
            },
            Mismatch(_) => {
                // Mismatch found - check if this is after some matches (partial match)
                // or immediate mismatch (no match)
                // The checkpoint atom_position tells us if we've made progress
                if self.state.checkpoint.atom_position != AtomPosition::from(0)
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
    /// Iterate through candidate comparisons until we reach an end state or need parent exploration
    /// Returns Ok(MatchedEndState) if we reach QueryExhausted or Mismatch with progress
    /// Returns Err((checkpoint_state, root_cursor)) if iterator completed without conclusion - need parent exploration
    pub(crate) fn advance_to_matched(
        mut self
    ) -> Result<
        MatchedEndState,
        (MatchedEndState, RootCursor<K, Candidate, Matched>),
    > {
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
                        *self.state.checkpoint.atom_position.as_ref();
                    let root_parent =
                        self.state.child_cursor.child_state.path.root_parent();

                    // Check if this is a valid match before destructuring
                    if reason == EndReason::Mismatch && checkpoint_pos == 0 {
                        // No progress - not a valid match, continue iteration
                        debug!(
                            root = %root_parent,
                            reason = "Mismatch with checkpoint=0 (no progress)",
                            "Discarding invalid match - continuing iteration"
                        );
                        continue;
                    }

                    debug!(
                        root = %root_parent,
                        root_width = root_parent.width.0,
                        checkpoint_pos = checkpoint_pos,
                        reason = ?reason,
                        "Valid match found - creating MatchedEndState"
                    );

                    // Create matched end state from current state
                    return Ok(self.create_end_state(reason));
                },
                None => {
                    // Iterator completed without Break - need parent exploration
                    // Create checkpoint state and return cursor for parent exploration
                    let checkpoint_state = self.create_checkpoint_from_state();
                    let root_cursor = self.into_candidate_matched();
                    return Err((checkpoint_state, root_cursor));
                },
            }
        }
    }

    /// Create a MatchedEndState from the current candidate state based on the end reason
    fn create_end_state(
        self,
        reason: EndReason,
    ) -> MatchedEndState {
        let CompareState {
            child_cursor,
            cursor,
            checkpoint,
            checkpoint_child,
            ..
        } = *self.state;

        // Choose the path based on end reason
        // For Mismatch, use checkpoint_child path (state at last match)
        // For QueryExhausted, use current child_cursor path
        let mut path = match reason {
            EndReason::QueryExhausted => child_cursor.child_state.path.clone(),
            EndReason::Mismatch => checkpoint_child.child_state.path.clone(),
        };

        // Simplify path to remove redundant segments at token borders
        path.child_path_mut::<Start, _>().simplify(&self.trav);
        path.child_path_mut::<End, _>().simplify(&self.trav);

        // Get the target token from the path
        let target_token = path.role_rooted_leaf_token::<End, _>(&self.trav);

        // Use entry_pos from the ChildState - it already tracks the correct position
        // For QueryExhausted: use current child cursor position
        // For Mismatch: use checkpoint_child position (last confirmed match)
        let child_state = match reason {
            EndReason::QueryExhausted => &child_cursor.child_state,
            EndReason::Mismatch => &checkpoint_child.child_state,
        };

        let start_pos = child_state.start_pos;
        let root_pos = child_state.entry_pos;

        // target should use root_pos (where the target token starts)
        let target = DownKey::new(target_token, root_pos.into());

        // end_pos is where matching ended (checkpoint cursor's position)
        let end_pos = checkpoint.atom_position;

        // Use the non-annotated path for PathCoverage
        let path_enum = PathCoverage::from_range_path(
            path, root_pos, target, end_pos, &self.trav,
        );

        // Choose cursor based on end reason:
        // - Mismatch: use checkpoint (last confirmed match)
        // - QueryExhausted: use current cursor (advanced to end of query)
        let result_cursor = match reason {
            EndReason::QueryExhausted => cursor.mark_match(),
            EndReason::Mismatch => checkpoint.clone(),
        };

        MatchedEndState {
            cursor: result_cursor,
            path: path_enum,
        }
    }

    /// Create a checkpoint Mismatch state from the current candidate state
    /// Used when iterator completes without definitive match/mismatch - needs parent exploration
    /// Always returns Mismatch since the root ended without completing the query
    fn create_checkpoint_from_state(&self) -> MatchedEndState {
        let checkpoint = &self.state.checkpoint;
        let checkpoint_child = &self.state.checkpoint_child;

        let mut path = checkpoint_child.child_state.path.clone();

        // Simplify paths
        path.start_path_mut().simplify(&self.trav);
        path.end_path_mut().simplify(&self.trav);

        // Get the target token from the path
        let target_token = path.role_rooted_leaf_token::<End, _>(&self.trav);

        // Use entry_pos from checkpoint_child - it already has the correct position
        let start_pos = checkpoint_child.child_state.start_pos;
        let root_pos = checkpoint_child.child_state.entry_pos;
        let end_pos = root_pos; // The end position is the same as root for the matched segment

        let target = DownKey::new(target_token, end_pos.into());

        let path_enum = PathCoverage::from_range_path(
            path, root_pos, target, end_pos, &self.trav,
        );

        // Create checkpoint - no need to determine if query is complete here
        // The cursor's atom_position indicates how far we matched
        // Completeness can be determined later by comparing position to query length
        MatchedEndState {
            cursor: checkpoint.clone(),
            path: path_enum,
        }
    }

    /// Convert <Candidate, Candidate> to <Candidate, Matched> for parent exploration
    fn into_candidate_matched(self) -> RootCursor<K, Candidate, Matched> {
        RootCursor {
            state: Box::new(CompareState {
                child_cursor: self.state.child_cursor.mark_match(),
                cursor: self.state.cursor.clone(),
                checkpoint: self.state.checkpoint.clone(),
                checkpoint_child: self.state.checkpoint_child.clone(),
                target: self.state.target,
                mode: self.state.mode,
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
                child_cursor: state.child_cursor.as_candidate(),
                cursor: state.cursor,
                checkpoint: state.checkpoint,
                checkpoint_child: state.checkpoint_child,
                target: state.target,
                mode: state.mode,
            }),
            trav: self.trav,
        }
    }

    pub(crate) fn next_parents(
        self,
        trav: &K::Trav,
    ) -> Result<(ParentCompareState, CompareParentBatch), Box<EndState>> {
        // Convert to Candidate first, then call next_parents
        self.into_candidate().next_parents(trav)
    }
}

impl<K: SearchKind> RootCursor<K, Candidate, Candidate> {
    pub(crate) fn next_parents(
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

#[derive(Debug, Clone, Deref, DerefMut)]
pub(crate) struct CompareParentBatch {
    #[deref]
    #[deref_mut]
    pub(crate) batch: ParentBatch,
    pub(crate) cursor: PatternCursor,
}
impl CompareParentBatch {
    pub(crate) fn into_compare_batch(self) -> VecDeque<ParentCompareState> {
        self.batch
            .parents
            .into_iter()
            .map(|parent_state| ParentCompareState {
                parent_state,
                cursor: self.cursor.clone(),
            })
            .collect()
    }
}
