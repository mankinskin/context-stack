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
//!
//! Prefix and postfix entry points share boundary-width validation, narrowing,
//! wrapper completion and insertion; only boundary geometry and cache seeding
//! depend on the selected side.
//! Path-based adapters validate their witnesses with
//! [`context_trace::boundary_target`], also used by expansion closure.

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
    insert_scope_boundary(graph, scope, width, Boundary::Prefix)
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
    insert_scope_boundary(graph, scope, width, Boundary::Postfix)
}

#[derive(Clone, Copy, Debug)]
enum Boundary {
    Prefix,
    Postfix,
}

impl Boundary {
    fn start(
        self,
        total: usize,
        width: usize,
    ) -> usize {
        match self {
            Self::Prefix => 0,
            Self::Postfix => total - width,
        }
    }

    fn split_offset(
        self,
        total: usize,
        width: usize,
    ) -> usize {
        match self {
            Self::Prefix => width,
            Self::Postfix => total - width,
        }
    }

    fn split_atom(
        self,
        total: usize,
        width: usize,
    ) -> usize {
        match self {
            Self::Prefix => width - 1,
            Self::Postfix => total - width,
        }
    }

    fn child(
        self,
        pattern: &Pattern,
    ) -> Token {
        match self {
            Self::Prefix => pattern[0],
            Self::Postfix => *pattern.last().unwrap(),
        }
    }

    fn wrapper_width(
        self,
        pattern: &Pattern,
        total: usize,
        width: usize,
    ) -> Option<usize> {
        let (index, start) = child_at(pattern, self.split_atom(total, width));
        match self {
            Self::Prefix => {
                let end = start + pattern[index].width().0;
                (end > width && index + 1 < pattern.len()).then_some(end)
            }
            Self::Postfix => {
                (start < total - width && index > 0).then_some(total - start)
            }
        }
    }

    fn seed(
        self,
        scope: Token,
        pattern_id: PatternId,
        pattern: &Pattern,
        width: usize,
    ) -> InitInterval {
        let total = scope.width().0;
        let (index, offset) = child_at(pattern, self.split_atom(total, width));
        let child = pattern[index];
        let location = ChildLocation::new(scope, pattern_id, index);
        let cache = match self {
            Self::Prefix => {
                let pos = DownPosition(offset.into());
                let mut cache = TraceCache::new(pattern[0]);
                cache.add_state(NewTraceEdge::<TopDown>::new(
                    DownKey::new(scope, pos),
                    DownKey::new(child, pos),
                    location,
                ));
                cache
            }
            Self::Postfix => {
                // Width of the entry child at or after the split position.
                let pos = UpPosition(
                    (offset + child.width().0 - (total - width)).into(),
                );
                let mut cache = TraceCache::new(child);
                cache.add_state(NewTraceEdge::<BottomUp>::new(
                    UpKey::new(child, pos),
                    UpKey::new(scope, pos),
                    location,
                ));
                cache
            }
        };
        InitInterval {
            root: scope,
            cache,
            end_bound: width.into(),
        }
    }
}

fn insert_scope_boundary(
    graph: &HypergraphRef,
    scope: Token,
    width: usize,
    boundary: Boundary,
) -> Result<Token, ErrorState> {
    assert!(
        width > 0 && width <= scope.width().0,
        "invalid scoped {:?} width",
        boundary
    );
    let start = boundary.start(scope.width().0, width);
    let token = scoped_boundary(graph, scope, width, boundary, true)?;
    assert_eq!(token.width().0, width, "scoped boundary width differs");
    assert_eq!(
        find_represented(graph, scope, start, width),
        Some(token),
        "scoped boundary is not represented inside the scope"
    );
    Ok(token)
}

fn scoped_boundary(
    graph: &HypergraphRef,
    scope: Token,
    width: usize,
    boundary: Boundary,
    wrap: bool,
) -> Result<Token, ErrorState> {
    let total = scope.width().0;
    let start = boundary.start(total, width);
    if let Some(token) = find_represented(graph, scope, start, width) {
        return Ok(token);
    }
    let patterns = graph.expect_child_patterns(scope);
    if let Some(child) = patterns
        .values()
        .map(|pattern| boundary.child(pattern))
        .find(|child| child.width().0 > width)
    {
        return scoped_boundary(graph, child, width, boundary, true);
    }
    if wrap {
        let wrapper = patterns
            .values()
            .filter_map(|pattern| boundary.wrapper_width(pattern, total, width))
            .filter(|&width| {
                splits_child(&patterns, boundary.split_offset(total, width))
            })
            .max();
        if let Some(wrapper_width) = wrapper {
            scoped_boundary(graph, scope, wrapper_width, boundary, true)?;
            return scoped_boundary(graph, scope, width, boundary, false);
        }
    }
    let (pattern_id, pattern) = patterns
        .iter()
        .next()
        .map(|(id, pattern)| (*id, pattern.clone()))
        .expect("scope must have a child pattern");
    InsertCtx::<Token>::from(graph.clone()).insert_init(
        (),
        boundary.seed(scope, pattern_id, &pattern, width),
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
