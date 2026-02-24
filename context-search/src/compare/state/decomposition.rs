use super::core::{
    CompareEndResult,
    CompareState,
    PathPairMode,
};
use crate::{
    compare::state::core::CompareLeafResult,
    cursor::{
        Candidate,
        Checkpointed,
        ChildCursor,
        CursorState,
        MarkMatchState,
        PathCursor,
    },
};
use context_trace::{
    graph::vertex::token::{
        HasSubLocation,
        SubToken,
    },
    path::{
        accessors::{
            child::HasRootedLeafToken,
            has_path::HasRootedPath,
        },
        RolePathUtils,
    },
    *,
};
use itertools::Itertools;
use std::{
    cmp::Ordering,
    collections::VecDeque,
    marker::PhantomData,
};
use tracing::{
    debug,
    trace,
};
use PathPairMode::*;
/// Helper function to decompose a token into its prefix children.
/// Reduces code duplication across trait implementations.
fn decompose_token_to_prefixes<G, State>(
    leaf: Token,
    trav: &G,
    update_state: impl Fn(SubToken, ChildLocation) -> State,
) -> VecDeque<(SubToken, State)>
where
    G: HasGraph,
{
    debug!(
        leaf = %leaf,
        "getting prefix_children"
    );
    let prefix_children =
        trav.graph().expect_vertex_data(leaf).prefix_children::<G>();
    debug!(num_children = prefix_children.len(), "got prefix_children");

    let result = prefix_children
        .iter()
        .sorted_unstable_by(|a: &&SubToken, b: &&SubToken| {
            b.token().width().cmp(&a.token().width())
        })
        .map(|sub: &SubToken| {
            let child_location = leaf.to_child_location(*sub.sub_location());
            let next_state = update_state(sub.clone(), child_location);
            (sub.clone(), next_state)
        })
        .collect();
    debug!("returning prefixes");
    result
}

pub(crate) trait PrefixStates: Sized + Clone {
    fn decompose_into_prefixes<G: HasGraph>(
        &self,
        trav: &G,
    ) -> VecDeque<(SubToken, Self)>;
}

// Implementation for ChildState with plain ChildLocation paths
impl PrefixStates for ChildState<ChildLocation> {
    fn decompose_into_prefixes<G: HasGraph>(
        &self,
        trav: &G,
    ) -> VecDeque<(SubToken, Self)> {
        let leaf = self.role_rooted_leaf_token::<End, _>(trav);
        decompose_token_to_prefixes(leaf, trav, |_sub, child_location| {
            let mut next = self.clone();
            next.path_append(child_location);
            next
        })
    }
}

// Specific implementation for ChildState with position-annotated paths
impl PrefixStates for ChildState<PositionAnnotated<ChildLocation>> {
    fn decompose_into_prefixes<G: HasGraph>(
        &self,
        trav: &G,
    ) -> VecDeque<(SubToken, Self)> {
        // Get the end leaf by accessing the path directly
        let leaf_location =
            self.path.end_path().last().map(|annotated| annotated.node);

        let leaf = if let Some(loc) = leaf_location {
            trav.graph().expect_child_at(loc)
        } else {
            // If path is empty, use root child
            self.path.role_root_child_token::<End, _>(trav)
        };

        // Use exit_pos as the position for appended nodes
        let position = self.exit_pos.0;

        decompose_token_to_prefixes(leaf, trav, |_sub, child_location| {
            let mut next = self.clone();
            // Append with proper position annotation matching entry_pos
            let annotated = PositionAnnotated {
                node: child_location,
                position,
            };
            // Directly append to the path with the annotated version
            next.path.path_append(annotated);
            next
        })
    }
}

// Separate implementation for PathCursor that correctly tracks atom_position
impl<P, S> PathCursor<P, S>
where
    P: HasRootedLeafToken<End> + PathAppend + Clone,
    S: CursorState,
{
    pub(crate) fn decompose_at_position<G: HasGraph>(
        &self,
        trav: &G,
        base_position: AtomPosition,
    ) -> VecDeque<(SubToken, Self)> {
        let leaf = self.path.role_rooted_leaf_token::<End, _>(trav);

        debug!(
            leaf = %leaf,
            "getting prefix_children"
        );
        let prefix_children =
            trav.graph().expect_vertex_data(leaf).prefix_children::<G>();
        debug!(num_children = prefix_children.len(), "got prefix_children");

        let sorted_children: Vec<_> = prefix_children
            .iter()
            .sorted_unstable_by(|a: &&SubToken, b: &&SubToken| {
                b.token().width().cmp(&a.token().width())
            })
            .collect();

        let mut result = VecDeque::new();

        for sub in sorted_children {
            let child_location = leaf.to_child_location(*sub.sub_location());
            let mut next_path = self.path.clone();
            next_path.path_append(child_location);

            // Position is base + this prefix's width (no accumulation across prefixes)
            let cursor_position = base_position + *sub.token().width();

            let cursor = PathCursor {
                path: next_path,
                atom_position: cursor_position,
                _state: PhantomData,
            };
            result.push_back((sub.clone(), cursor));
        }

        debug!("returning prefixes");
        result
    }
}

// Implementation for PositionAnnotated<ChildLocation> - these methods use the role_rooted_leaf_token helper
impl CompareState<Candidate, Candidate, PositionAnnotated<ChildLocation>> {
    /// Compare a candidate with position-annotated paths
    /// Extracts ChildLocations and delegates to token comparison logic
    pub(crate) fn compare_leaf_tokens<G: HasGraph>(
        self,
        trav: &G,
    ) -> CompareLeafResult<PositionAnnotated<ChildLocation>> {
        use Ordering::*;
        let path_leaf =
            self.rooted_path().role_rooted_leaf_token::<End, _>(trav);
        let query_leaf =
            (*self.query.current()).role_rooted_leaf_token::<End, _>(trav);

        let cursor_end_index = HasRootChildIndex::<End>::root_child_index(
            &self.query.current().path,
        );
        trace!(
            path_leaf = %path_leaf,
            query_leaf = %query_leaf,
            path_width = *path_leaf.width(),
            query_width = *query_leaf.width(),
            cursor_pos = %self.query.current().atom_position,
            cursor_end_index = cursor_end_index,
            checkpoint_pos = %self.query.checkpoint().atom_position,
            mode = %self.mode,
            "comparing candidate tokens (position-annotated)"
        );

        if path_leaf == query_leaf {
            trace!(
                token = *path_leaf.index,
                width = *path_leaf.width(),
                "tokens matched"
            );
            CompareLeafResult::Finished(CompareEndResult::FoundMatch(
                self.mark_match(),
            ))
        } else {
            match path_leaf.width().cmp(&query_leaf.width()) {
                Equal if path_leaf.width() == TokenWidth(1) => {
                    trace!("atom mismatch: different atoms");
                    CompareLeafResult::Finished(CompareEndResult::Mismatch(
                        self.mark_mismatch(),
                    ))
                },
                Equal => {
                    trace!("equal width but not matching: need prefixes of both tokens");
                    CompareLeafResult::Prefixes(
                        self.mode_prefixes(trav, GraphMajor)
                            .into_iter()
                            .chain(self.mode_prefixes(trav, QueryMajor))
                            .collect(),
                    )
                },
                Greater => {
                    trace!("GraphMajor: path_width > query_width");
                    CompareLeafResult::Prefixes(
                        self.mode_prefixes(trav, GraphMajor),
                    )
                },
                Less => {
                    trace!("QueryMajor: path_width < query_width");
                    CompareLeafResult::Prefixes(
                        self.mode_prefixes(trav, QueryMajor),
                    )
                },
            }
        }
    }

    fn mode_prefixes<G: HasGraph>(
        &self,
        trav: &G,
        mode: PathPairMode,
    ) -> ChildQueue<
        CompareState<Candidate, Candidate, PositionAnnotated<ChildLocation>>,
    > {
        debug!(
            old_mode = %self.mode,
            new_mode = %mode,
            "creating new state with different mode (position-annotated)"
        );
        CompareState {
            mode,
            ..self.clone()
        }
        .expand_to_prefix_comparisons(trav)
    }

    /// Generate token states for index prefixes with position tracking
    pub(crate) fn expand_to_prefix_comparisons<G: HasGraph>(
        &self,
        trav: &G,
    ) -> ChildQueue<
        CompareState<Candidate, Candidate, PositionAnnotated<ChildLocation>>,
    > {
        debug!(
            mode = %self.mode,
            child_state = %self.child.current().child_state,
            cursor = %self.query.current(),
            "entering prefix_states (position-annotated)"
        );

        match self.mode {
            GraphMajor => {
                let checkpoint_pos = *self.query.checkpoint().cursor_pos();
                debug!("calling child_state.prefix_states");
                let prefixes = self
                    .child
                    .current()
                    .child_state
                    .decompose_into_prefixes(trav);

                trace!(
                    mode = "GraphMajor",
                    num_prefixes = prefixes.len(),
                    checkpoint_pos = %checkpoint_pos,
                    "decomposing graph path token into prefixes (position-annotated)"
                );

                let result: ChildQueue<
                    CompareState<
                        Candidate,
                        Candidate,
                        PositionAnnotated<ChildLocation>,
                    >,
                > = prefixes
                    .into_iter()
                    .enumerate()
                    .map(
                        |(i, (sub, child_state)): (
                            usize,
                            (
                                SubToken,
                                ChildState<PositionAnnotated<ChildLocation>>,
                            ),
                        )| {
                            let token = sub.token();
                            let target_pos = checkpoint_pos.into();
                            debug!(
                                prefix_idx = i,
                                sub_width = *token.width(),
                                "creating prefix state (position-annotated)"
                            );
                            CompareState {
                                target: DownKey::new(token, target_pos),
                                child: Checkpointed {
                                    checkpoint: self.child.checkpoint().clone(),
                                    candidate: ChildCursor {
                                        child_state,
                                        _state: PhantomData,
                                    },
                                    _state: PhantomData,
                                },
                                mode: self.mode,
                                query: self.query.clone(),
                            }
                        },
                    )
                    .collect();
                debug!(
                    num_results = result.len(),
                    "exiting prefix_states (GraphMajor, position-annotated)"
                );
                result
            },
            QueryMajor => {
                let base_position = self.query.checkpoint().atom_position;
                debug!("calling cursor.prefix_states_from");
                let cursor_prefixes = self
                    .query
                    .current()
                    .decompose_at_position(trav, base_position);

                trace!(
                    mode = "QueryMajor",
                    cursor_pos = %self.query.current().atom_position,
                    base_pos = %base_position,
                    num_prefixes = cursor_prefixes.len(),
                    "decomposing query cursor token into prefixes (position-annotated)"
                );

                let result: ChildQueue<
                    CompareState<
                        Candidate,
                        Candidate,
                        PositionAnnotated<ChildLocation>,
                    >,
                > = cursor_prefixes
                    .into_iter()
                    .enumerate()
                    .map(
                        |(i, (sub, cursor)): (
                            usize,
                            (SubToken, PathCursor<PatternRangePath, Candidate>),
                        )| {
                            trace!(
                                prefix_idx = i,
                                sub_width = *sub.token().width(),
                                cursor_pos = %cursor.atom_position,
                                "created prefix state (position-annotated)"
                            );
                            CompareState {
                                target: DownKey::new(
                                    sub.token(),
                                    (*self.query.checkpoint().cursor_pos())
                                        .into(),
                                ),
                                child: self.child.clone(),
                                mode: self.mode,
                                query: Checkpointed {
                                    checkpoint: self.query.checkpoint().clone(),
                                    candidate: cursor,
                                    _state: PhantomData,
                                },
                            }
                        },
                    )
                    .collect();
                debug!(
                    num_results = result.len(),
                    "exiting prefix_states (QueryMajor, position-annotated)"
                );
                result
            },
        }
    }
}
