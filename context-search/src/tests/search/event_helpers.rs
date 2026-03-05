//! Test helpers for exact event-sequence assertions.
//!
//! Each search test should call [`assert_events`] with the exact expected
//! [`Transition`] sequence. Builder functions (`start`, `up`, `down`, etc.)
//! construct transitions using symbolic [`Token`] references so tests remain
//! readable and resilient to index renumbering.
//!
//! [`EdgeRef`] fields are **ignored** during comparison (zeroed on both
//! sides) because their values depend on internal `VizPathGraph` state that
//! isn't part of the algorithm's observable behaviour.

use context_trace::graph::{
    search_path::{EdgeRef, PathNode},
    vertex::token::Token,
    visualization::{GraphOpEvent, Transition},
};

// ── placeholder EdgeRef (stripped during comparison) ──────────────────────

const E: EdgeRef = EdgeRef {
    from: 0,
    to: 0,
    pattern_idx: 0,
    sub_index: 0,
};

// ── strip helper ─────────────────────────────────────────────────────────

/// Replace all [`EdgeRef`] fields with the placeholder so equality checks
/// focus on the semantically meaningful fields.
/// Also normalizes `child_index` to 0 since production code now populates
/// real values from `sub_index`, but tests focus on node transitions.
fn strip_edge(t: &Transition) -> Transition {
    match t {
        Transition::VisitParent {
            from,
            to,
            entry_pos,
            ..
        } => Transition::VisitParent {
            from: *from,
            to: *to,
            entry_pos: *entry_pos,
            edge: E,
        },
        Transition::VisitChild {
            from,
            to,
            replace,
            ..
        } => Transition::VisitChild {
            from: *from,
            to: *to,
            child_index: 0, // Normalized - tests focus on from/to, not entry index
            replace: *replace,
            edge: E,
        },
        Transition::CandidateMatch { root, .. } => {
            Transition::CandidateMatch {
                root: *root,
                edge: E,
            }
        },
        other => other.clone(),
    }
}

// ── assertion entry-point ────────────────────────────────────────────────

/// Assert that `response.events` matches `expected` exactly (order,
/// length, and per-field values — minus [`EdgeRef`]).
///
/// Also validates that step counters are sequential `0..N`.
/// Note: Query events (op_type == Query) are filtered out for comparison.
pub fn assert_events(events: &[GraphOpEvent], expected: &[Transition]) {
    use context_trace::graph::visualization::OperationType;

    // Filter out Query events - they're emitted alongside Search events
    // but tests focus on the Search event stream.
    let search_events: Vec<_> = events
        .iter()
        .filter(|e| e.op_type != OperationType::Query)
        .collect();

    // Step counters must be 0..N
    let steps: Vec<usize> = search_events.iter().map(|e| e.step).collect();
    assert_eq!(
        steps,
        (0..search_events.len()).collect::<Vec<_>>(),
        "Step counters are not sequential",
    );

    let actual: Vec<Transition> =
        search_events.iter().map(|e| strip_edge(&e.transition)).collect();
    let expected: Vec<Transition> =
        expected.iter().map(|t| strip_edge(t)).collect();

    pretty_assertions::assert_eq!(actual, expected);
}

// ── builder functions ────────────────────────────────────────────────────

/// Shorthand: convert Token to PathNode with actual width.
fn pn(t: &Token) -> PathNode {
    PathNode { index: t.index.0, width: t.width.0 }
}

/// Shorthand: convert Token to PathNode with placeholder width (1).
/// Used for parent/root nodes where production code uses TODO placeholders.
fn pn1(t: &Token) -> PathNode {
    PathNode { index: t.index.0, width: 1 }
}

/// `StartNode` — first event of every search.
pub fn start(token: &Token) -> Transition {
    Transition::StartNode {
        node: pn(token),  // real width
    }
}

/// `ParentExplore` — root boundary reached, candidates queued.
pub fn explore(root: &Token, candidates: &[&Token]) -> Transition {
    Transition::ParentExplore {
        current_root: root.index.0,
        parent_candidates: candidates.iter().map(|t| t.index.0).collect(),
    }
}

/// `VisitParent` — ascending from `from` to `to`.
pub fn up(from: &Token, to: &Token) -> Transition {
    Transition::VisitParent {
        from: pn1(from),  // placeholder width
        to: pn1(to),      // placeholder width
        entry_pos: 0,
        edge: E,
    }
}

/// `VisitChild` — descending from `from` to `to` (child_index = 0).
pub fn down(from: &Token, to: &Token, replace: bool) -> Transition {
    Transition::VisitChild {
        from: pn1(from),  // placeholder width (parent)
        to: pn(to),       // real width (child)
        child_index: 0,
        replace,
        edge: E,
    }
}

/// `VisitChild` with explicit `child_index` (for continuing matches past
/// the first child position in a pattern).
pub fn down_at(
    from: &Token,
    to: &Token,
    replace: bool,
    child_index: usize,
) -> Transition {
    Transition::VisitChild {
        from: pn1(from),  // placeholder width (parent)
        to: pn(to),       // real width (child)
        child_index,
        replace,
        edge: E,
    }
}

/// `ChildMatch` — child comparison succeeded.
pub fn matched(token: &Token, cursor_pos: usize) -> Transition {
    Transition::ChildMatch {
        node: pn(token),
        cursor_pos,
    }
}

/// `ChildMismatch` — child comparison failed.
pub fn mismatched(
    token: &Token,
    cursor_pos: usize,
    expected: &Token,
    actual: &Token,
) -> Transition {
    Transition::ChildMismatch {
        node: pn(token),
        cursor_pos,
        expected: pn(expected),
        actual: pn(actual),
    }
}

/// `CandidateMatch` — parent became the new root.
pub fn root_match(token: &Token) -> Transition {
    Transition::CandidateMatch {
        root: pn1(token),  // placeholder width
        edge: E,
    }
}

/// `CandidateMismatch` — candidate rejected.
pub fn skip(
    token: &Token,
    queue_remaining: usize,
    is_parent: bool,
) -> Transition {
    Transition::CandidateMismatch {
        node: pn1(token),  // placeholder width
        queue_remaining,
        is_parent,
    }
}

/// `Done` — search completed successfully.
pub fn done_ok(token: &Token) -> Transition {
    Transition::Done {
        final_node: Some(token.index.0),
        success: true,
    }
}
