//! Event derivation from algorithm result types.
//!
//! This module provides the [`IntoTransitions`] trait and [`EventContext`] struct
//! for converting algorithm result types (like [`CompareInfo`]) into visualization
//! events ([`Transition`]).

use crate::r#match::{CompareInfo, CompareOutcome, PrefixChildInfo};
use context_trace::graph::{
    search_path::{EdgeRef, PathNode},
    visualization::Transition,
};

/// Context required for event derivation.
///
/// Contains state from the search algorithm that's needed to properly
/// construct [`Transition`] events.
#[derive(Debug, Clone)]
pub(crate) struct EventContext {
    /// The parent node index (for edge `from` values).
    pub parent_node: usize,
    /// Whether the end_path is empty (determines `replace` flag for VisitChild).
    pub end_path_empty: bool,
}

impl EventContext {
    /// Create a new event context.
    pub fn new(parent_node: usize, end_path_empty: bool) -> Self {
        Self { parent_node, end_path_empty }
    }

    /// Create a PathNode for the parent with placeholder width.
    pub fn parent_path_node(&self) -> PathNode {
        PathNode { index: self.parent_node, width: 1 } // TODO: get real width
    }

    /// Whether VisitChild should use replace=true.
    pub fn should_replace(&self) -> bool {
        !self.end_path_empty
    }
}

/// Trait for converting algorithm result types into visualization events.
///
/// Implementors produce a list of `(Transition, description)` tuples that
/// can be emitted via the event system.
pub(crate) trait IntoTransitions {
    /// Convert this value into visualization transitions.
    fn into_transitions(self, ctx: &EventContext) -> Vec<(Transition, String)>;
}

// ── RootCursor advance event data ────────────────────────────────────────────

/// Pre-computed event data for a successful advance match.
///
/// Extracted from RootCursor state in `finish_root_cursor`.
#[derive(Debug, Clone)]
pub(crate) struct MatchAdvanceData {
    /// The parent/root node.
    pub parent_node: PathNode,
    /// The matched child node.
    pub child_node: PathNode,
    /// Child entry index within parent pattern.
    pub sub_index: usize,
    /// Query cursor position at match.
    pub cursor_pos: usize,
}

impl MatchAdvanceData {
    /// Create from raw values extracted from RootCursor state.
    pub fn new(
        parent_idx: usize,
        child_idx: usize,
        child_width: usize,
        sub_index: usize,
        cursor_pos: usize,
    ) -> Self {
        Self {
            parent_node: PathNode { index: parent_idx, width: 1 }, // TODO: real parent width
            child_node: PathNode { index: child_idx, width: child_width },
            sub_index,
            cursor_pos,
        }
    }
}

impl IntoTransitions for MatchAdvanceData {
    fn into_transitions(self, ctx: &EventContext) -> Vec<(Transition, String)> {
        vec![
            (
                Transition::VisitChild {
                    from: self.parent_node,
                    to: self.child_node,
                    child_index: self.sub_index,
                    edge: EdgeRef {
                        from: self.parent_node.index,
                        to: self.child_node.index,
                        pattern_idx: 0,
                        sub_index: self.sub_index,
                    },
                    replace: ctx.should_replace(),
                },
                format!("Visiting child {} from root {}", self.child_node.index, self.parent_node.index),
            ),
            (
                Transition::ChildMatch {
                    node: self.child_node,
                    cursor_pos: self.cursor_pos,
                },
                format!(
                    "Child token match at node {} (query pos {})",
                    self.child_node.index, self.cursor_pos
                ),
            ),
        ]
    }
}

/// Pre-computed event data for a mismatch during advance.
#[derive(Debug, Clone)]
pub(crate) struct MismatchAdvanceData {
    /// The parent/root node.
    pub parent_node: PathNode,
    /// The actual (mismatched) child node.
    pub actual_node: PathNode,
    /// The expected query token.
    pub expected_node: PathNode,
    /// Child entry index within parent pattern.
    pub sub_index: usize,
    /// Query cursor position at mismatch.
    pub cursor_pos: usize,
}

impl MismatchAdvanceData {
    /// Create from raw values extracted from RootCursor state.
    pub fn new(
        parent_idx: usize,
        actual_idx: usize,
        actual_width: usize,
        expected_idx: usize,
        expected_width: usize,
        sub_index: usize,
        cursor_pos: usize,
    ) -> Self {
        Self {
            parent_node: PathNode { index: parent_idx, width: 1 }, // TODO: real parent width
            actual_node: PathNode { index: actual_idx, width: actual_width },
            expected_node: PathNode { index: expected_idx, width: expected_width },
            sub_index,
            cursor_pos,
        }
    }
}

impl IntoTransitions for MismatchAdvanceData {
    fn into_transitions(self, ctx: &EventContext) -> Vec<(Transition, String)> {
        vec![
            (
                Transition::VisitChild {
                    from: self.parent_node,
                    to: self.actual_node,
                    child_index: self.sub_index,
                    edge: EdgeRef {
                        from: self.parent_node.index,
                        to: self.actual_node.index,
                        pattern_idx: 0,
                        sub_index: self.sub_index,
                    },
                    replace: ctx.should_replace(),
                },
                format!("Visiting child {} from root {}", self.actual_node.index, self.parent_node.index),
            ),
            (
                Transition::ChildMismatch {
                    node: self.actual_node,
                    cursor_pos: self.cursor_pos,
                    expected: self.expected_node,
                    actual: self.actual_node,
                },
                format!(
                    "Child mismatch at node {} (expected {}, got {})",
                    self.actual_node.index, self.expected_node.index, self.actual_node.index
                ),
            ),
        ]
    }
}

// ── CompareInfo implementation ───────────────────────────────────────────────

impl IntoTransitions for CompareInfo {
    fn into_transitions(self, ctx: &EventContext) -> Vec<(Transition, String)> {
        let parent_path_node = ctx.parent_path_node();
        let parent_node = ctx.parent_node;

        match self.outcome {
            CompareOutcome::Match => {
                vec![
                    (
                        Transition::VisitChild {
                            from: parent_path_node,
                            to: self.token,
                            child_index: self.sub_index,
                            edge: EdgeRef {
                                from: parent_node,
                                to: self.token.index,
                                pattern_idx: 0,
                                sub_index: self.sub_index,
                            },
                            replace: ctx.should_replace(),
                        },
                        format!("Comparing child node {}", self.token.index),
                    ),
                    (
                        Transition::ChildMatch {
                            node: self.token,
                            cursor_pos: self.cursor_pos,
                        },
                        format!(
                            "Child match at node {} (query pos {})",
                            self.token.index, self.cursor_pos
                        ),
                    ),
                ]
            },
            CompareOutcome::Mismatch { expected, actual } => {
                vec![
                    (
                        Transition::VisitChild {
                            from: parent_path_node,
                            to: self.token,
                            child_index: self.sub_index,
                            edge: EdgeRef {
                                from: parent_node,
                                to: self.token.index,
                                pattern_idx: 0,
                                sub_index: self.sub_index,
                            },
                            replace: ctx.should_replace(),
                        },
                        format!("Comparing child node {}", self.token.index),
                    ),
                    (
                        Transition::ChildMismatch {
                            node: self.token,
                            cursor_pos: self.cursor_pos,
                            expected,
                            actual,
                        },
                        format!(
                            "Child mismatch at node {} (expected {}, got {})",
                            self.token.index, expected.index, actual.index
                        ),
                    ),
                ]
            },
            CompareOutcome::Prefixes(children) => {
                children
                    .into_iter()
                    .map(|child| prefix_child_transition(ctx, child))
                    .collect()
            },
        }
    }
}

/// Create a VisitChild transition for a prefix child.
fn prefix_child_transition(
    ctx: &EventContext,
    child: PrefixChildInfo,
) -> (Transition, String) {
    let parent_node = ctx.parent_node;
    (
        Transition::VisitChild {
            from: ctx.parent_path_node(),
            to: child.token,
            child_index: child.sub_index,
            edge: EdgeRef {
                from: parent_node,
                to: child.token.index,
                pattern_idx: 0,
                sub_index: child.sub_index,
            },
            replace: false, // Prefix children never replace
        },
        format!("Visiting prefix child {}", child.token.index),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use context_trace::graph::search_path::PathNode;

    #[test]
    fn compare_info_match_produces_two_events() {
        let info = CompareInfo {
            token: PathNode { index: 5, width: 1 },
            query_token: PathNode { index: 10, width: 1 },
            cursor_pos: 3,
            sub_index: 2,
            outcome: CompareOutcome::Match,
        };
        let ctx = EventContext::new(1, true);
        let events = info.into_transitions(&ctx);

        assert_eq!(events.len(), 2);
        assert!(matches!(events[0].0, Transition::VisitChild { .. }));
        assert!(matches!(events[1].0, Transition::ChildMatch { .. }));
    }

    #[test]
    fn compare_info_mismatch_produces_two_events() {
        let info = CompareInfo {
            token: PathNode { index: 5, width: 1 },
            query_token: PathNode { index: 10, width: 1 },
            cursor_pos: 3,
            sub_index: 2,
            outcome: CompareOutcome::Mismatch {
                expected: PathNode { index: 10, width: 1 },
                actual: PathNode { index: 5, width: 1 },
            },
        };
        let ctx = EventContext::new(1, true);
        let events = info.into_transitions(&ctx);

        assert_eq!(events.len(), 2);
        assert!(matches!(events[0].0, Transition::VisitChild { .. }));
        assert!(matches!(events[1].0, Transition::ChildMismatch { .. }));
    }

    #[test]
    fn compare_info_prefixes_produces_one_event_per_child() {
        let info = CompareInfo {
            token: PathNode { index: 5, width: 3 },
            query_token: PathNode { index: 10, width: 1 },
            cursor_pos: 3,
            sub_index: 0,
            outcome: CompareOutcome::Prefixes(vec![
                PrefixChildInfo { token: PathNode { index: 6, width: 1 }, sub_index: 0 },
                PrefixChildInfo { token: PathNode { index: 7, width: 1 }, sub_index: 1 },
            ]),
        };
        let ctx = EventContext::new(5, true);
        let events = info.into_transitions(&ctx);

        assert_eq!(events.len(), 2);
        assert!(matches!(events[0].0, Transition::VisitChild { .. }));
        assert!(matches!(events[1].0, Transition::VisitChild { .. }));
    }

    #[test]
    fn replace_flag_depends_on_end_path_empty() {
        let info = CompareInfo {
            token: PathNode { index: 5, width: 1 },
            query_token: PathNode { index: 10, width: 1 },
            cursor_pos: 3,
            sub_index: 2,
            outcome: CompareOutcome::Match,
        };

        // end_path empty -> replace = false
        let ctx_empty = EventContext::new(1, true);
        let events = info.clone().into_transitions(&ctx_empty);
        if let Transition::VisitChild { replace, .. } = &events[0].0 {
            assert!(!replace);
        }

        // end_path not empty -> replace = true
        let ctx_nonempty = EventContext::new(1, false);
        let events = info.into_transitions(&ctx_nonempty);
        if let Transition::VisitChild { replace, .. } = &events[0].0 {
            assert!(replace);
        }
    }
}
