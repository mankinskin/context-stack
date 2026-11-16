//! Factored closure of a canonical root-postfix/expansion-prefix overlap.
//!
//! For scope `X = L + P` and expansion `E = P + T`, construct
//! `[X,T...] = [L...,E]` without allocating complement-only tokens.
//! Preparation is read-only; applying the plan may mutate the graph but leaves
//! accumulation updates to the sink. Neither phase provides rollback.

use crate::pipeline::root::{
    RootManager,
    Span,
};
use context_search::Find;
use context_trace::*;

/// Validated structural closure and its intended root integration operation.
pub(crate) enum ClosurePlan {
    /// Replace an entire editable tail by its expansion without a closure token.
    Tail {
        root: Token,
        old: Token,
        token: Token,
    },
    /// Materialize or reuse a factored closure around the selected scope.
    Closed {
        root: Token,
        /// Entire last child for a tail edit, otherwise the accumulated root.
        scope: Token,
        /// Integrate by replacing the root's tail rather than adopting a new root.
        tail: bool,
        /// The alternatives `[X,T...]` and `[L...,E]`, respectively.
        patterns: [Pattern; 2],
        /// Canonical token with the closure content, if already represented.
        existing: Option<Token>,
    },
}

impl ClosurePlan {
    /// Materialize the prepared closure and return the sink operation.
    ///
    /// Adds missing decompositions to an existing canonical result or allocates
    /// the closure token. Direct tail expansion does not modify the graph here.
    pub(crate) fn apply(
        self,
        graph: &HypergraphRef,
    ) -> Span {
        match self {
            Self::Tail { root, old, token } => Span::ReplaceTail {
                previous: root,
                old,
                token,
            },
            Self::Closed {
                root,
                scope,
                tail,
                patterns,
                existing,
            } => {
                let token = match existing {
                    Some(token) => {
                        for pattern in patterns {
                            if !graph
                                .expect_child_patterns(token)
                                .values()
                                .any(|p| p == &pattern)
                            {
                                graph.add_pattern_with_update(token, pattern);
                            }
                        }
                        token
                    }
                    None => graph.insert_patterns(patterns),
                };
                if tail {
                    Span::ReplaceTail {
                        previous: root,
                        old: scope,
                        token,
                    }
                } else {
                    Span::ReplaceRoot {
                        previous: root,
                        token,
                    }
                }
            }
        }
    }
}

/// Prepare closure using current ownership and connected overlap paths.
///
/// Call after materializing `expansion` and reconnecting `root_path`.
/// Editable roots close around their entire last child; non-editable roots
/// and self-expansions close around the root. This function does not mutate.
///
/// # Panics
///
/// Panics on missing or disconnected boundary paths, invalid complement widths,
/// or repeated boundaries in the selected decompositions or existing result.
pub(crate) fn prepare(
    graph: &HypergraphRef,
    sink: &RootManager,
    root: Token,
    postfix: Token,
    root_path: &[ChildLocation],
    expansion: Token,
) -> ClosurePlan {
    assert!(
        expansion.width() > postfix.width(),
        "overlap must consume input"
    );
    validate_path(graph, root, postfix, root_path, false);
    let editable = sink.is_editable(root)
        && expansion.vertex_index() != root.vertex_index();
    let (scope, path) = if editable {
        let vertex = root.vertex(graph);
        let (_, pattern) = vertex.expect_any_child_pattern();
        let scope = *pattern.last().expect("editable root must have a tail");
        assert_eq!(
            root_path[0].parent, root,
            "postfix path must start at root"
        );
        (scope, &root_path[1..])
    } else {
        (root, root_path)
    };
    if editable && scope == postfix {
        return ClosurePlan::Tail {
            root,
            old: scope,
            token: expansion,
        };
    }
    validate_path(graph, scope, postfix, path, false);
    let prefix_path = boundary_path(graph, expansion, postfix, true)
        .expect("expansion must represent the overlap prefix");
    let left = complement(graph, scope, postfix, path, false);
    let right = complement(graph, expansion, postfix, &prefix_path, true);
    assert!(
        !left.is_empty() && !right.is_empty(),
        "closure requires proper complements"
    );
    let primary: Pattern = std::iter::once(scope).chain(right).collect();
    let alternative: Pattern =
        left.into_iter().chain(std::iter::once(expansion)).collect();
    assert_eq!(
        pattern_width(&primary),
        pattern_width(&alternative),
        "closure widths differ"
    );
    let existing = find_complete(graph, &primary);
    if let Some(token) = existing {
        let patterns = graph.expect_child_patterns(token);
        for proposed in [&primary, &alternative] {
            for current in patterns.values() {
                assert!(
                    current == proposed
                        || boundaries(current)
                            .is_disjoint(&boundaries(proposed)),
                    "closure would duplicate an existing boundary"
                );
            }
        }
    }
    ClosurePlan::Closed {
        root,
        scope,
        tail: editable,
        patterns: [primary, alternative],
        existing,
    }
}

/// Look up an entire token matching a nonempty pattern without materialization.
///
/// Returns `None` for absent or partial matches; unexpected search errors panic.
pub(crate) fn find_complete(
    graph: &HypergraphRef,
    pattern: &Pattern,
) -> Option<Token> {
    if pattern.len() == 1 {
        return Some(pattern[0]);
    }
    match graph.find_ancestor(pattern.to_vec()) {
        Ok(response)
            if response.query_exhausted()
                && response.is_entire_root()
                && response.root_token().width() == pattern_width(pattern) =>
        {
            Some(response.root_token())
        }
        Ok(_)
        | Err(
            ErrorReason::SingleIndex(_)
            | ErrorReason::NoParents
            | ErrorReason::NotFound,
        ) => None,
        Err(error) => panic!("closure content lookup failed: {:?}", error),
    }
}

fn boundaries(pattern: &Pattern) -> std::collections::BTreeSet<usize> {
    let mut width = 0;
    pattern
        .iter()
        .take(pattern.len() - 1)
        .map(|token| {
            width += token.width().0;
            width
        })
        .collect()
}

/// Find a connected path to a token at the root's prefix or postfix boundary.
///
/// `prefix` selects the first child at each level; otherwise select the last.
/// Returns an empty path for `root == target`, or `None` if no path exists.
pub(crate) fn boundary_path(
    graph: &HypergraphRef,
    root: Token,
    target: Token,
    prefix: bool,
) -> Option<Vec<ChildLocation>> {
    if root == target {
        return Some(Vec::new());
    }
    let vertex = root.vertex(graph);
    for (&pattern_id, pattern) in vertex.child_patterns() {
        let sub_index = if prefix { 0 } else { pattern.len() - 1 };
        let child = pattern[sub_index];
        if child.width() < target.width() {
            continue;
        }
        if let Some(mut path) = boundary_path(graph, child, target, prefix) {
            path.insert(0, ChildLocation::new(root, pattern_id, sub_index));
            return Some(path);
        }
    }
    None
}

/// Assert that a path connects root to target using only prefix/postfix children.
///
/// # Panics
///
/// Panics if a location is missing, disconnected, off the selected boundary,
/// or does not terminate at the target token.
pub(crate) fn validate_path(
    graph: &HypergraphRef,
    root: Token,
    target: Token,
    path: &[ChildLocation],
    prefix: bool,
) {
    let mut parent = root;
    for location in path {
        assert_eq!(location.parent, parent, "disconnected overlap path");
        let pattern = graph.expect_pattern_at(*location);
        assert_eq!(
            location.sub_index,
            if prefix { 0 } else { pattern.len() - 1 },
            "overlap path is not at a boundary"
        );
        parent = pattern[location.sub_index];
    }
    assert_eq!(parent, target, "overlap path does not reach shared token");
}

/// Reuse a boundary decomposition's complement, or collect hierarchical siblings.
///
/// `prefix` selects the right complement; otherwise select the left. Nested
/// right siblings are assembled inner-first to preserve input order.
fn complement(
    graph: &HypergraphRef,
    root: Token,
    shared: Token,
    path: &[ChildLocation],
    prefix: bool,
) -> Pattern {
    validate_path(graph, root, shared, path, prefix);
    let offset = if prefix {
        shared.width().0
    } else {
        root.width().0 - shared.width().0
    };
    let vertex = root.vertex(graph);
    let mut canonical = None;
    for pattern in vertex.child_patterns().values() {
        let mut width = 0;
        for (index, child) in pattern.iter().enumerate().take(pattern.len() - 1)
        {
            width += child.width().0;
            if width == offset {
                assert!(
                    canonical.is_none(),
                    "multiple decompositions share a complement boundary"
                );
                canonical = Some(Pattern::from(if prefix {
                    pattern[index + 1..].to_vec()
                } else {
                    pattern[..index + 1].to_vec()
                }));
            }
        }
    }
    if let Some(pattern) = canonical {
        return pattern;
    }
    let mut parts = Vec::new();
    for location in path {
        let pattern = graph.expect_pattern_at(*location);
        parts.push(if prefix {
            pattern[location.sub_index + 1..].to_vec()
        } else {
            pattern[..location.sub_index].to_vec()
        });
    }
    if prefix {
        parts.reverse();
    }
    let result: Pattern = parts.into_iter().flatten().collect();
    assert_eq!(
        pattern_width(&result).0,
        root.width().0 - shared.width().0,
        "complement width differs"
    );
    result
}

