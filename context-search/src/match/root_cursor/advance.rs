use super::core::{
    AdvanceCursorsResult,
    AdvanceToEndResult,
    RootCursor,
};
use crate::{
    compare::state::{
        IndexAdvanceResult,
        QueryAdvanceResult,
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
    path::RolePathUtils,
    End,
    HasRootChildIndex,
    Start,
    *,
};
use tracing::{
    debug,
    trace,
};

impl<K: SearchKind> RootCursor<K, Matched, Matched>
where
    K::Trav: Clone,
{
    /// Advance through matches until we reach a conclusive end state
    ///
    /// This is the main entry point for processing a matched root cursor.
    /// It advances both cursors (query and child) through the comparison process
    /// until either:
    /// - A conclusive match is found (QueryExhausted or Mismatch with progress)
    /// - Parent exploration is needed (child exhausted but query continues)
    ///
    /// Returns Completed with MatchResult if conclusive end reached
    /// Returns NeedsParentExploration if more tokens needed to continue matching
    #[context_trace::instrument_sig(level = "debug", skip(self))]
    pub(crate) fn advance_until_conclusion(self) -> AdvanceToEndResult<K> {
        let root_parent =
            self.state.child.current().child_state.path.root_parent();
        debug!(
            root = %root_parent,
            width = root_parent.width.0,
            checkpoint_pos = *self.state.query.checkpoint().atom_position.as_ref(),
            "→ advance_until_conclusion: starting advancement for root"
        );

        // Try to advance to the next match (advance query + advance child)
        match self.advance_to_next_match() {
            Ok(candidate_cursor) => {
                let root = candidate_cursor
                    .state
                    .child
                    .current()
                    .child_state
                    .path
                    .root_parent();
                debug!(
                    root = %root,
                    "→ advance_until_conclusion: got <Candidate, Candidate> cursor, calling iterate_until_conclusion"
                );
                // We have a <Candidate, Candidate> cursor - iterate to find end
                candidate_cursor.iterate_until_conclusion()
            },
            Err(Ok(matched_state)) => {
                debug!(
                    root = %matched_state.root_parent(),
                    "→ advance_until_conclusion: query ended immediately (QueryExhausted)"
                );
                // Query ended immediately - return the matched state
                AdvanceToEndResult::Completed(matched_state)
            },
            Err(Err(need_parent)) => {
                let root = need_parent
                    .state
                    .child
                    .current()
                    .child_state
                    .path
                    .root_parent();
                let checkpoint_pos = *need_parent
                    .state
                    .query
                    .checkpoint()
                    .atom_position
                    .as_ref();
                debug!(
                    root = %root,
                    checkpoint_pos = checkpoint_pos,
                    "→ advance_until_conclusion: index ended before query (need parent exploration)"
                );
                // Need parent exploration immediately (index ended before query)
                // Create checkpoint state for this root
                let checkpoint_state =
                    need_parent.create_parent_exploration_state();
                debug!(
                    checkpoint_root = %checkpoint_state.root_parent(),
                    checkpoint_width = checkpoint_state.root_parent().width.0,
                    "→ advance_until_conclusion: created checkpoint state for parent exploration"
                );
                AdvanceToEndResult::NeedsParentExploration {
                    checkpoint: checkpoint_state,
                    cursor: need_parent,
                }
            },
        }
    }

    /// Advance to the next match by: 1. advancing query cursor, 2. advancing child path
    /// Returns Ok(<Candidate, Candidate>) if both advanced successfully
    /// Returns Err(Ok(MatchResult)) if query ended (complete match)
    /// Returns Err(Err(<Candidate, Matched>)) if child path ended but query continues (need parent exploration)
    #[context_trace::instrument_sig(level = "trace", skip(self))]
    fn advance_to_next_match(
        self
    ) -> Result<
        RootCursor<K, Candidate, Candidate>,
        Result<MatchResult, RootCursor<K, Candidate, Matched>>,
    > {
        trace!("  → advance_to_next_match: Step 1 - calling advance_query");
        // Step 1: Advance query cursor
        let query_advanced = match self.advance_query() {
            Ok(cursor) => {
                let root =
                    cursor.state.child.current().child_state.path.root_parent();
                trace!(
                    root = %root,
                    query_pos = *cursor.state.query.current().atom_position.as_ref(),
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

        trace!("  → advance_to_next_match: Step 2 - calling advance_child");
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
                    child_pos = ?context_trace::path::accessors::path_accessor::StatePosition::target_pos(&both_advanced.state.child.current().child_state).unwrap(),
                    "  → advance_to_next_match: Step 2 complete - child advanced successfully, got <Candidate, Candidate>"
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
                    "  → advance_to_next_match: Step 2 - child ended (need parent exploration)"
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

        let matched_state = *self.state;
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
                    state: Box::new(query_advanced),
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

                // For exhausted queries, the checkpoint points at the last matched token
                // but its atom_position doesn't include that token's width yet
                // We need to add the width to get the total consumed tokens
                let mut checkpoint_cursor =
                    matched_state.query.checkpoint().clone();
                let new_position = usize::from(checkpoint_cursor.atom_position)
                    + last_token_width_value.0;
                checkpoint_cursor.atom_position = new_position.into();

                let final_end_index =
                    HasRootChildIndex::<End>::root_child_index(
                        &checkpoint_cursor.path,
                    );

                tracing::trace!(
                    checkpoint_pos=%checkpoint_cursor.atom_position,
                    final_end_index,
                    last_token_width=%last_token_width_value,
                    "advance_query: returning checkpoint cursor for exhausted query (includes last token width)"
                );

                let target = DownKey::new(target_index, root_pos.into());
                Err(MatchResult {
                    cursor: checkpoint_cursor,
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
        let state = *self.state;
        let trav = self.trav;

        let root_parent = state.child.current().child_state.path.root_parent();
        let child_pos_before =
            *context_trace::path::accessors::path_accessor::StatePosition::target_pos(&state.child.current().child_state).unwrap();
        trace!(
            root = %root_parent,
            child_pos = ?child_pos_before,
            "    → advance_child: attempting to advance child (index) cursor"
        );

        // Try to advance index cursor
        match state.advance_index_cursor(&trav) {
            IndexAdvanceResult::Advanced(both_advanced) => {
                let child_pos_after =
                    context_trace::path::accessors::path_accessor::StatePosition::target_pos(
                        &both_advanced
                            .child
                            .current()
                            .child_state
                    ).unwrap();
                debug!(
                    root = %root_parent,
                    child_pos_before = ?child_pos_before,
                    child_pos_after = ?child_pos_after,
                    "    → advance_child: SUCCESS - child cursor advanced"
                );
                // Both cursors advanced - return Candidate cursor
                Ok(RootCursor {
                    state: Box::new(both_advanced),
                    trav,
                })
            },
            IndexAdvanceResult::Exhausted(query_only_advanced) => {
                debug!(
                    root = %root_parent,
                    child_pos = ?child_pos_before,
                    "    → advance_child: CHILD ENDED - need parent exploration"
                );
                // Index ended but query continues - need parent exploration
                Err(RootCursor {
                    state: Box::new(query_only_advanced),
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
        // Use query checkpoint position for root_pos (not child position)
        let root_pos = checkpoint.atom_position;

        // Simplify path to remove redundant segments
        path.child_path_mut::<Start, _>().simplify(&self.trav);
        path.child_path_mut::<End, _>().simplify(&self.trav);

        let target_index = path.role_rooted_leaf_token::<End, _>(&self.trav);

        // Use current query cursor's PATH (advanced beyond checkpoint when child cannot advance)
        // This ensures end_index points to next token to match, not last matched
        // But keep checkpoint's atom_position (number of tokens successfully matched)
        let end_cursor = PathCursor {
            path: self.state.query.current().path.clone(),
            atom_position: checkpoint.atom_position,
            _state: std::marker::PhantomData::<Matched>,
        };
        let end_pos = checkpoint.atom_position;

        let target = DownKey::new(target_index, root_pos.into());
        let path_enum = PathCoverage::from_range_path(
            path, root_pos, target, end_pos, &self.trav,
        );

        MatchResult {
            cursor: end_cursor,
            path: path_enum,
        }
    }
}

impl<K: SearchKind> RootCursor<K, Matched, Matched> {
    /// Advance both cursors after finding a match, transitioning from Matched to Candidate state
    ///
    /// This method is called after a successful comparison finds a match.
    /// It attempts to advance both the query and child cursors to prepare for the next comparison.
    ///
    /// Returns BothAdvanced if both cursors successfully moved forward
    /// Returns QueryExhausted if the query pattern is complete
    /// Returns ChildExhausted if the child path ended but query continues (needs parent exploration)
    pub(crate) fn advance_both_from_match(self) -> AdvanceCursorsResult<K> {
        let matched_state = *self.state;

        // Step 1: Try to advance QUERY cursor
        match matched_state.advance_query_cursor(&self.trav) {
            QueryAdvanceResult::Advanced(query_advanced) => {
                // Step 2: Try to advance INDEX cursor
                match query_advanced.advance_index_cursor(&self.trav) {
                    IndexAdvanceResult::Advanced(both_advanced) => {
                        tracing::trace!("both cursors advanced successfully");
                        // Both cursors advanced - return as Candidate state
                        AdvanceCursorsResult::BothAdvanced(RootCursor {
                            state: Box::new(both_advanced),
                            trav: self.trav,
                        })
                    },
                    IndexAdvanceResult::Exhausted(_query_only_advanced) => {
                        tracing::trace!(
                            "index cursor cannot advance - graph path ended"
                        );
                        // INDEX ENDED, QUERY CONTINUES
                        // Return cursor in <Candidate, Matched> state for parent exploration
                        AdvanceCursorsResult::ChildExhausted(RootCursor {
                            state: Box::new(_query_only_advanced),
                            trav: self.trav,
                        })
                    },
                }
            },
            QueryAdvanceResult::Exhausted(_matched_state) => {
                tracing::trace!(
                    "query cursor cannot advance - query pattern ended"
                );
                // QUERY ENDED - no cursor to return
                AdvanceCursorsResult::QueryExhausted
            },
        }
    }
}
