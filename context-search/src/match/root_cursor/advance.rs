use super::core::{
    ConclusiveEnd,
    RootAdvanceResult,
    RootCursor,
    RootEndResult,
};
use crate::{
    compare::{
        iterator::CompareIterator,
        state::{
            CompareEndResult,
            IndexAdvanceResult,
            QueryAdvanceResult,
        },
    },
    cursor::{
        Candidate,
        MarkMatchState,
        Matched,
        PathCursor,
    },
    state::{
        end::PathCoverage,
        matched::MatchResult,
    },
    traversal::SearchKind,
};
use context_trace::{
    path::{
        accessors::path_accessor::HasTargetOffset,
        RolePathUtils,
    },
    End,
    HasRootChildIndex,
    Start,
    *,
};
use std::marker::PhantomData;
use tracing::{
    debug,
    info,
    trace,
};

impl<K: SearchKind> RootCursor<K, Matched, Matched>
where
    K::Trav: Clone,
{
    /// Advance RootCursor to the next matched state
    ///
    /// This is the main driver for RootCursor advancement. It:
    /// 1. Advances both cursors to get a Candidate state
    /// 2. Calls CompareIterator to compare tokens at the candidate position
    /// 3. Returns either Advanced(new Matched cursor) or Finished(end result)
    ///
    /// # Returns
    /// - `Advanced(RootCursor<Matched, Matched>)`: Successfully found next match
    /// - `Finished(Conclusive(Mismatch))`: Found mismatch after progress - maximum match reached
    /// - `Finished(Conclusive(Exhausted))`: Query pattern exhausted - complete match
    /// - `Finished(Inconclusive)`: Root boundary reached - needs parent exploration
    #[context_trace::instrument_sig(level = "debug", skip(self))]
    pub(crate) fn advance_to_next_match(self) -> RootAdvanceResult<K> {
        info!("→ advance_to_next_match: starting advancement");

        // Save trav before moving self
        let trav = self.trav.clone();

        // Step 1: Try to advance both cursors to get Candidate state
        let candidate_cursor = match self.advance_both_cursors_internal() {
            Ok(candidate) => {
                info!("→ advance_to_next_match: both cursors advanced to Candidate");
                candidate
            },
            Err(Ok(())) => {
                // Query exhausted - complete match found
                debug!(
                    "→ advance_to_next_match: query exhausted - complete match"
                );
                return RootAdvanceResult::Finished(RootEndResult::Conclusive(
                    ConclusiveEnd::Exhausted,
                ));
            },
            Err(Err(need_parent)) => {
                // Child exhausted but query continues - need parent exploration
                debug!("→ advance_to_next_match: child exhausted - needs parent exploration");
                return RootAdvanceResult::Finished(
                    RootEndResult::Inconclusive(need_parent),
                );
            },
        };

        // Step 2: Compare tokens at candidate position
        info!(
            "→ advance_to_next_match: comparing tokens at candidate position"
        );
        let prev_candidate = candidate_cursor.state.clone();
        match CompareIterator::<K>::new(trav.clone(), candidate_cursor.state)
            .compare()
        {
            CompareEndResult::FoundMatch(matched_state) => {
                // Successfully matched - return new Matched cursor
                // CompareEndResult already has Matched states, SearchIterator will create checkpoint
                debug!(
                    root = %matched_state.child.current().child_state.path.root_parent(),
                    query_pos = *matched_state.query.current().atom_position.as_ref(),
                    "→ advance_to_next_match: match found - returning Advanced"
                );

                RootAdvanceResult::Advanced(RootCursor {
                    state: matched_state,
                    trav,
                })
            },
            CompareEndResult::Mismatch(_) => {
                // Found mismatch - check if we made progress
                let checkpoint_pos =
                    *prev_candidate.query.checkpoint().atom_position.as_ref();
                if checkpoint_pos == 0 {
                    // No progress - not a valid match, this shouldn't happen after a Matched state
                    info!("→ advance_to_next_match: immediate mismatch after match - this is unexpected");
                    return RootAdvanceResult::Finished(
                        RootEndResult::Conclusive(ConclusiveEnd::Mismatch(
                            RootCursor {
                                state: prev_candidate,
                                trav: trav.clone(),
                            },
                        )),
                    );
                }

                // Found mismatch after progress - this is the maximum match for this root
                info!(
                    checkpoint_pos,
                    "→ advance_to_next_match: mismatch after progress - maximum match reached"
                );
                RootAdvanceResult::Finished(RootEndResult::Conclusive(
                    ConclusiveEnd::Mismatch(RootCursor {
                        state: prev_candidate,
                        trav,
                    }),
                ))
            },
        }
    }

    /// Internal method to advance both cursors
    /// Returns Ok(Candidate) if both advanced
    /// Returns Err(Ok(())) if query exhausted
    /// Returns Err(Err(Candidate, Matched)) if child exhausted
    fn advance_both_cursors_internal(
        self
    ) -> Result<
        RootCursor<K, Candidate, Candidate>,
        Result<(), RootCursor<K, Candidate, Matched>>,
    > {
        trace!(
            "  → advance_both_cursors_internal: Step 1 - calling advance_query"
        );
        // Step 1: Advance query cursor
        let query_advanced = match self.advance_query() {
            Ok(cursor) => {
                let root =
                    cursor.state.child.current().child_state.path.root_parent();
                trace!(
                    root = %root,
                    query_pos = *cursor.state.query.current().atom_position.as_ref(),
                    "  → advance_both_cursors_internal: Step 1 complete - query advanced successfully"
                );
                cursor
            },
            Err(_matched_result) => {
                debug!("  → advance_both_cursors_internal: Step 1 - query ended (QueryExhausted)");
                return Err(Ok(()));
            },
        };

        trace!(
            "  → advance_both_cursors_internal: Step 2 - calling advance_child"
        );
        // Step 2: Advance child path (index)
        match query_advanced.advance_child() {
            Ok(both_advanced) => {
                let root = both_advanced
                    .state
                    .child
                    .current()
                    .child_state
                    .path
                    .root_parent();
                trace!(
                    root = %root,
                    child_pos = ?both_advanced.state.child.current().child_state.target_offset(),
                    "  → advance_both_cursors_internal: Step 2 complete - child advanced successfully, got <Candidate, Candidate>"
                );
                Ok(both_advanced)
            },
            Err(need_parent) => {
                let root = need_parent
                    .state
                    .child
                    .current()
                    .child_state
                    .path
                    .root_parent();
                debug!(
                    root = %root,
                    "  → advance_both_cursors_internal: Step 2 - child ended (need parent exploration)"
                );
                Err(Err(need_parent))
            },
        }
    }

    /// Step 1: Advance the query cursor
    /// Returns Ok(<Candidate, Matched>) if query advanced
    /// Returns Err(MatchResult) if query ended (QueryExhausted)
    #[context_trace::instrument_sig(level = "trace", skip(self))]
    fn advance_query(
        self
    ) -> Result<RootCursor<K, Candidate, Matched>, MatchResult> {
        let root_parent =
            self.state.child.current().child_state.path.root_parent();
        let query_pos_before =
            *self.state.query.current().atom_position.as_ref();
        trace!(
            root = %root_parent,
            query_pos = query_pos_before,
            "    → advance_query: attempting to advance query cursor"
        );

        let matched_state = self.state;
        let trav = self.trav;

        // Try to advance query cursor
        match matched_state.advance_query_cursor(&trav) {
            QueryAdvanceResult::Advanced(query_advanced) => {
                let query_pos_after =
                    *query_advanced.query.current().atom_position.as_ref();
                debug!(
                    root = %root_parent,
                    query_pos_before = query_pos_before,
                    query_pos_after = query_pos_after,
                    "    → advance_query: SUCCESS - query cursor advanced"
                );
                // Query advanced successfully
                Ok(RootCursor {
                    state: query_advanced,
                    trav,
                })
            },
            QueryAdvanceResult::Exhausted(matched_state) => {
                debug!(
                    root = %root_parent,
                    query_pos = query_pos_before,
                    "    → advance_query: QUERY ENDED - creating QueryExhausted state"
                );
                // Query ended - create complete match state
                // Use entry_pos (where we entered the root) for root_pos
                let root_pos =
                    matched_state.child.current().child_state.entry_pos;
                let path =
                    matched_state.child.current().child_state.path.clone();
                let _start_pos =
                    matched_state.child.current().child_state.start_pos;
                let root_parent = path.root_parent();
                let target_index = path.role_rooted_leaf_token::<End, _>(&trav);
                let last_token_width_value = target_index.width();
                // end_pos is where matching ended (checkpoint position)
                let end_pos = matched_state.query.checkpoint().atom_position;
                tracing::trace!(
                    "root_cursor advance_query: root_parent={}, root_pos={}, checkpoint.atom_position={}, last_token_width={}, end_pos={}",
                    root_parent, usize::from(root_pos), *matched_state.query.checkpoint().atom_position,
                    last_token_width_value, usize::from(end_pos)
                );

                // For exhausted queries, the checkpoint already includes all matched tokens
                // after we checkpoint on each successful advance
                let checkpoint_cursor =
                    matched_state.query.checkpoint().clone();

                let final_end_index =
                    HasRootChildIndex::<End>::root_child_index(
                        &checkpoint_cursor.path,
                    );

                tracing::trace!(
                    checkpoint_pos=%checkpoint_cursor.atom_position,
                    final_end_index,
                    "advance_query: returning checkpoint cursor for exhausted query"
                );

                let target = DownKey::new(target_index, root_pos.into());

                // Wrap cursor in Checkpointed (at checkpoint, no candidate)
                use crate::cursor::{
                    checkpointed::Checkpointed,
                    PathCursor,
                };
                let cursor_state =
                    Checkpointed::<PathCursor<_>>::new(checkpoint_cursor);

                Err(MatchResult {
                    cursor: cursor_state,
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
    #[context_trace::instrument_sig(level = "trace", skip(self))]
    fn advance_child(
        self
    ) -> Result<
        RootCursor<K, Candidate, Candidate>,
        RootCursor<K, Candidate, Matched>,
    > {
        let state = self.state;
        let trav = self.trav;

        let root_parent = state.child.current().child_state.path.root_parent();
        trace!(
            root = %root_parent,
            "    → advance_child: attempting to advance child (index) cursor"
        );

        // Try to advance index cursor
        match state.advance_index_cursor(&trav) {
            IndexAdvanceResult::Advanced(both_advanced) => {
                debug!(
                    root = %root_parent,
                    "    → advance_child: SUCCESS - child cursor advanced"
                );
                // Both cursors advanced - return Candidate cursor
                Ok(RootCursor {
                    state: both_advanced,
                    trav,
                })
            },
            IndexAdvanceResult::Exhausted(query_only_advanced) => {
                debug!(
                    root = %root_parent,
                    "    → advance_child: CHILD ENDED - need parent exploration"
                );
                // Index ended but query continues - need parent exploration
                Err(RootCursor {
                    state: query_only_advanced,
                    trav,
                })
            },
        }
    }
}

impl<K: SearchKind> RootCursor<K, Candidate, Matched> {
    /// Create a QueryExhausted state from this root cursor's checkpoint
    /// Used when the root matched successfully but needs parent exploration
    #[context_trace::instrument_sig(level = "trace", skip(self))]
    pub(crate) fn create_parent_exploration_state(&self) -> MatchResult {
        let checkpoint = self.state.query.checkpoint();
        let checkpoint_child = self.state.child.checkpoint();

        // Use checkpoint_child path as it represents the matched state
        let mut path = checkpoint_child.child_state.path.clone();
        let _start_pos = checkpoint_child.child_state.start_pos;
        // root_pos is where we entered the root (beginning of the match)
        let root_pos = checkpoint_child.child_state.entry_pos;

        // Simplify path to remove redundant segments
        path.child_path_mut::<Start, _>().simplify(&self.trav);
        path.child_path_mut::<End, _>().simplify(&self.trav);

        let target_index = path.role_rooted_leaf_token::<End, _>(&self.trav);

        // Clone the Checkpointed<PatternCursor> preserving both checkpoint and candidate
        // Convert from Checkpointed<PathCursor<_, Candidate>> to Checkpointed<PathCursor<_, Matched>>
        // This is a type-level conversion only - the candidate represents an exploration position,
        // not a confirmed match. We preserve it so the next search can start from the advanced position.
        use crate::cursor::checkpointed::Checkpointed;
        let cursor_state = Checkpointed {
            checkpoint: self.state.query.checkpoint().clone(),
            candidate: self.state.query.candidate.as_ref().map(|c| {
                // Type-level conversion Candidate → Matched without semantic state change
                PathCursor {
                    path: c.path.clone(),
                    atom_position: c.atom_position,
                    _state: std::marker::PhantomData::<Matched>,
                }
            }),
            _state: PhantomData,
        };

        let end_pos = checkpoint.atom_position;

        let target = DownKey::new(target_index, root_pos.into());
        let path_enum = PathCoverage::from_range_path(
            path, root_pos, target, end_pos, &self.trav,
        );

        MatchResult {
            cursor: cursor_state,
            path: path_enum,
        }
    }
}
