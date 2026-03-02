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
    search_path::EdgeRef,
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
fn strip_edge(t: &Transition) -> Transition {
    match t {
        Transition::VisitParent {
            from,
            to,
            entry_pos,
            width,
            ..
        } => Transition::VisitParent {
            from: *from,
            to: *to,
            entry_pos: *entry_pos,
            width: *width,
            edge: E,
        },
        Transition::VisitChild {
            from,
            to,
            child_index,
            width,
            replace,
            ..
        } => Transition::VisitChild {
            from: *from,
            to: *to,
            child_index: *child_index,
            width: *width,
            replace: *replace,
            edge: E,
        },
        Transition::CandidateMatch { root, width, .. } => {
            Transition::CandidateMatch {
                root: *root,
                width: *width,
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
pub fn assert_events(events: &[GraphOpEvent], expected: &[Transition]) {
    // Step counters must be 0..N
    let steps: Vec<usize> = events.iter().map(|e| e.step).collect();
    assert_eq!(
        steps,
        (0..events.len()).collect::<Vec<_>>(),
        "Step counters are not sequential",
    );

    let actual: Vec<Transition> =
        events.iter().map(|e| strip_edge(&e.transition)).collect();
    let expected: Vec<Transition> =
        expected.iter().map(|t| strip_edge(t)).collect();

    pretty_assertions::assert_eq!(actual, expected);
}

// ── builder functions ────────────────────────────────────────────────────

/// Shorthand: extract raw vertex index.
fn n(t: &Token) -> usize {
    t.index.0
}
/// Shorthand: extract width as `usize`.
fn w(t: &Token) -> usize {
    t.width.0
}

/// `StartNode` — first event of every search.
pub fn start(token: &Token) -> Transition {
    Transition::StartNode {
        node: n(token),
        width: w(token),
    }
}

/// `ParentExplore` — root boundary reached, candidates queued.
pub fn explore(root: &Token, candidates: &[&Token]) -> Transition {
    Transition::ParentExplore {
        current_root: n(root),
        parent_candidates: candidates.iter().map(|t| n(t)).collect(),
    }
}

/// `VisitParent` — ascending from `from` to `to`.
pub fn up(from: &Token, to: &Token) -> Transition {
    Transition::VisitParent {
        from: n(from),
        to: n(to),
        entry_pos: 0,
        width: 1,
        edge: E,
    }
}

/// `VisitChild` — descending from `from` to `to` (child_index = 0).
pub fn down(from: &Token, to: &Token, replace: bool) -> Transition {
    Transition::VisitChild {
        from: n(from),
        to: n(to),
        child_index: 0,
        width: w(to),
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
        from: n(from),
        to: n(to),
        child_index,
        width: w(to),
        replace,
        edge: E,
    }
}

/// `ChildMatch` — child comparison succeeded.
pub fn matched(token: &Token, cursor_pos: usize) -> Transition {
    Transition::ChildMatch {
        node: n(token),
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
        node: n(token),
        cursor_pos,
        expected: n(expected),
        actual: n(actual),
    }
}

/// `CandidateMatch` — parent became the new root.
pub fn root_match(token: &Token) -> Transition {
    Transition::CandidateMatch {
        root: n(token),
        width: 1,
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
        node: n(token),
        queue_remaining,
        is_parent,
    }
}

/// `Done` — search completed successfully.
pub fn done_ok(token: &Token) -> Transition {
    Transition::Done {
        final_node: Some(n(token)),
        success: true,
    }
}
