//! Scope-restricted insertion of boundary complements.
//!
//! Given a scope token and a connected path from the scope to a boundary
//! token `P` (its prefix or postfix), these operations return the canonical
//! token for the scope's remaining content and guarantee that it is
//! represented inside the scope: reachable from the scope through child
//! patterns at the complement's offset. Unlike `ToInsertCtx::insert`,
//! no global search is performed; only the scope (and its descendants) is
//! split and joined.
//!
//! The split+join pipeline is seeded with a root-only [`TraceCache`]:
//! - prefix (`[0, w)`): a top-down edge keyed by the start offset of the
//!   scope child containing atom `w - 1`, with `end_bound = w`;
//! - postfix (`[o, |scope|)`): a bottom-up edge keyed by the width of the
//!   scope child containing atom `o` that lies at or after `o`.
//!
//! Nested children are split by propagating the root's split positions.
//!
//! Before splitting, the scope is narrowed to a first (prefix) or last
//! (postfix) child covering the complement. Wrapper boundaries that would lie
//! inside a child of an alternative decomposition are materialized first,
//! because the join cannot resolve partitions strictly inside unsplit
//! children.

use crate::{
    insert::context::InsertCtx,
    interval::init::InitInterval,
};
use context_search::ErrorState;
use context_trace::{
    trace::{
        BottomUp,
        TopDown,
        cache::new::NewTraceEdge,
    },
    *,
};

/// Find a token of `width` at atom `offset` reachable from `scope` through
/// child patterns, or `scope` itself if it spans exactly that range.
pub fn find_represented(
    graph: &Hypergraph,
    scope: Token,
    offset: usize,
    width: usize,
) -> Option<Token> {
    if offset == 0 && width == scope.width().0 {
        return Some(scope);
    }
    let vertex = graph.expect_vertex_data(scope);
    for pattern in vertex.child_patterns().values() {
        let mut start = 0;
        for &child in pattern.iter() {
            let end = start + child.width().0;
            if start <= offset && offset + width <= end {
                if let Some(token) =
                    find_represented(graph, child, offset - start, width)
                {
                    return Some(token);
                }
                break;
            }
            if end > offset {
                break;
            }
            start = end;
        }
    }
    None
}

/// Assert that `path` connects `scope` to its prefix (`prefix`) or postfix
/// boundary token and return that token.
fn boundary_target(
    graph: &Hypergraph,
    scope: Token,
    path: &[ChildLocation],
    prefix: bool,
) -> Token {
    let mut parent = scope;
    for location in path {
        assert_eq!(location.parent, parent, "disconnected boundary path");
        let pattern = graph.expect_pattern_at(*location);
        assert_eq!(
            location.sub_index,
            if prefix { 0 } else { pattern.len() - 1 },
            "boundary path leaves the scope boundary"
        );
        parent = pattern[location.sub_index];
    }
    parent
}

/// Locate the child of `pattern` containing atom `offset`.
///
/// Returns the child index and its start offset.
fn child_at(
    pattern: &Pattern,
    offset: usize,
) -> (usize, usize) {
    let mut start = 0;
    for (index, child) in pattern.iter().enumerate() {
        let end = start + child.width().0;
        if offset < end {
            return (index, start);
        }
        start = end;
    }
    panic!("offset exceeds pattern width")
}

/// Whether `offset` lies strictly inside a child of any pattern of `scope`.
fn splits_child(
    patterns: &ChildPatterns,
    offset: usize,
) -> bool {
    patterns
        .values()
        .any(|pattern| child_at(pattern, offset).1 < offset)
}

/// Return the canonical token for `scope[0..width]`, represented in `scope`.
///
/// Reuses a represented occurrence, otherwise splits and joins inside the
/// smallest scope reached through first children, rewriting decompositions
/// as needed. Wrapper boundaries that would split children of alternative
/// decompositions are materialized first, so the join only sees boundaries
/// it can resolve in every decomposition.
///
/// # Panics
///
/// Panics if `width` is zero or exceeds the scope, or if insertion does not
/// leave the result represented inside the scope.
pub fn insert_scope_prefix_width(
    graph: &HypergraphRef,
    scope: Token,
    width: usize,
) -> Result<Token, ErrorState> {
    assert!(
        width > 0 && width <= scope.width().0,
        "invalid scoped prefix width"
    );
    let token = scoped_prefix(graph, scope, width, true)?;
    assert_eq!(token.width().0, width, "scoped prefix width differs");
    assert_eq!(
        find_represented(graph, scope, 0, width),
        Some(token),
        "scoped prefix is not represented inside the scope"
    );
    Ok(token)
}

fn scoped_prefix(
    graph: &HypergraphRef,
    scope: Token,
    width: usize,
    wrap: bool,
) -> Result<Token, ErrorState> {
    if let Some(token) = find_represented(graph, scope, 0, width) {
        return Ok(token);
    }
    let patterns = graph.expect_child_patterns(scope);
    if let Some(&first) = patterns
        .values()
        .map(|pattern| &pattern[0])
        .find(|first| first.width().0 > width)
    {
        return scoped_prefix(graph, first, width, true);
    }
    if wrap {
        // Wrapper ends of patterns splitting a non-last child.
        let wrapper = patterns
            .values()
            .filter_map(|pattern| {
                let (index, start) = child_at(pattern, width - 1);
                let end = start + pattern[index].width().0;
                (end > width && index + 1 < pattern.len()).then_some(end)
            })
            .filter(|&end| splits_child(&patterns, end))
            .max();
        if let Some(end) = wrapper {
            scoped_prefix(graph, scope, end, true)?;
            return scoped_prefix(graph, scope, width, false);
        }
    }
    let (pattern_id, pattern) = patterns
        .iter()
        .next()
        .map(|(id, pattern)| (*id, pattern.clone()))
        .expect("scope must have a child pattern");
    let (exit, offset) = child_at(&pattern, width - 1);
    let pos = DownPosition(offset.into());
    let mut cache = TraceCache::new(pattern[0]);
    cache.add_state(NewTraceEdge::<TopDown>::new(
        DownKey::new(scope, pos),
        DownKey::new(pattern[exit], pos),
        ChildLocation::new(scope, pattern_id, exit),
    ));
    InsertCtx::<Token>::from(graph.clone()).insert_init(
        (),
        InitInterval {
            root: scope,
            cache,
            end_bound: width.into(),
        },
    )
}

/// Return the canonical token for the last `width` atoms of `scope`,
/// represented in `scope`.
///
/// Mirrors [`insert_scope_prefix_width`], narrowing through last children.
///
/// # Panics
///
/// Panics if `width` is zero or exceeds the scope, or if insertion does not
/// leave the result represented inside the scope.
pub fn insert_scope_postfix_width(
    graph: &HypergraphRef,
    scope: Token,
    width: usize,
) -> Result<Token, ErrorState> {
    assert!(
        width > 0 && width <= scope.width().0,
        "invalid scoped postfix width"
    );
    let start = scope.width().0 - width;
    let token = scoped_postfix(graph, scope, start, true)?;
    assert_eq!(token.width().0, width, "scoped postfix width differs");
    assert_eq!(
        find_represented(graph, scope, start, width),
        Some(token),
        "scoped postfix is not represented inside the scope"
    );
    Ok(token)
}

fn scoped_postfix(
    graph: &HypergraphRef,
    scope: Token,
    start: usize,
    wrap: bool,
) -> Result<Token, ErrorState> {
    let total = scope.width().0;
    if let Some(token) = find_represented(graph, scope, start, total - start) {
        return Ok(token);
    }
    let patterns = graph.expect_child_patterns(scope);
    if let Some(&last) = patterns
        .values()
        .map(|pattern| pattern.last().unwrap())
        .find(|last| last.width().0 > total - start)
    {
        let offset = total - last.width().0;
        return scoped_postfix(graph, last, start - offset, true);
    }
    if wrap {
        // Wrapper starts of patterns splitting a non-first child.
        let wrapper = patterns
            .values()
            .filter_map(|pattern| {
                let (index, offset) = child_at(pattern, start);
                (offset < start && index > 0).then_some(offset)
            })
            .filter(|&offset| splits_child(&patterns, offset))
            .min();
        if let Some(offset) = wrapper {
            scoped_postfix(graph, scope, offset, true)?;
            return scoped_postfix(graph, scope, start, false);
        }
    }
    let (pattern_id, pattern) = patterns
        .iter()
        .next()
        .map(|(id, pattern)| (*id, pattern.clone()))
        .expect("scope must have a child pattern");
    let (entry, offset) = child_at(&pattern, start);
    let child = pattern[entry];
    // Width of the entry child at or after the split position.
    let pos = UpPosition((offset + child.width().0 - start).into());
    let mut cache = TraceCache::new(child);
    cache.add_state(NewTraceEdge::<BottomUp>::new(
        UpKey::new(child, pos),
        UpKey::new(scope, pos),
        ChildLocation::new(scope, pattern_id, entry),
    ));
    InsertCtx::<Token>::from(graph.clone()).insert_init(
        (),
        InitInterval {
            root: scope,
            cache,
            end_bound: (total - start).into(),
        },
    )
}

/// Return the canonical token for `scope`'s content before its postfix `P`.
///
/// `postfix_path` must connect `scope` to `P` through last children.
pub fn insert_scope_prefix(
    graph: &HypergraphRef,
    scope: Token,
    postfix_path: &[ChildLocation],
) -> Result<Token, ErrorState> {
    let shared = boundary_target(graph, scope, postfix_path, false);
    insert_scope_prefix_width(graph, scope, scope.width().0 - shared.width().0)
}

/// Return the canonical token for `scope`'s content after its prefix `P`.
///
/// `prefix_path` must connect `scope` to `P` through first children.
pub fn insert_scope_postfix(
    graph: &HypergraphRef,
    scope: Token,
    prefix_path: &[ChildLocation],
) -> Result<Token, ErrorState> {
    let shared = boundary_target(graph, scope, prefix_path, true);
    insert_scope_postfix_width(graph, scope, scope.width().0 - shared.width().0)
}
