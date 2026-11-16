//! Factored closure of a canonical root-postfix/expansion-prefix overlap.
//!
//! For scope `X = L + P` and expansion `E = P + T`, construct the binary
//! closure `C = [X,T] = [L,E]`. The complements `L` and `T` are canonical
//! tokens represented inside their smallest scopes: `L` at offset 0 of `X`
//! and `T` at offset `|P|` of `E`. Existing represented occurrences are
//! reused; otherwise they are materialized by scope-restricted insertion,
//! which rewrites the scope's decompositions so the occurrence is connected.
//! Preparation is read-only; applying the plan may mutate the graph but leaves
//! accumulation updates to the sink. Neither phase provides rollback.

use crate::pipeline::root::{
    RootManager,
    Span,
};
use context_insert::insert::scoped::{
    find_represented,
    insert_scope_postfix_width,
    insert_scope_prefix_width,
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
        /// `X`: entire last child for a tail edit, otherwise the accumulated root.
        scope: Token,
        /// Integrate by replacing the root's tail rather than adopting a new root.
        tail: bool,
        /// `P`: the shared overlap token.
        shared: Token,
        /// `E = P + T`: the accepted expansion.
        expansion: Token,
        /// Connected postfix path `X -> P` at preparation time.
        scope_path: Vec<ChildLocation>,
        /// Connected prefix path `E -> P` at preparation time.
        expansion_path: Vec<ChildLocation>,
        /// `|L| = |X| - |P|`.
        left_width: usize,
        /// `|T| = |E| - |P|`.
        right_width: usize,
        /// Canonical token with the closure content, if already represented.
        existing: Option<Token>,
    },
}

impl ClosurePlan {
    /// Materialize the prepared closure and return the sink operation.
    ///
    /// Resolves `L` inside `X`, then `T` inside `E`, re-derives the overlap
    /// paths (insertion may rewrite decompositions), and adds the missing
    /// decompositions `[X,T]` and `[L,E]` to an existing canonical result or
    /// allocates the closure token. Direct tail expansion does not modify the
    /// graph here.
    ///
    /// # Panics
    ///
    /// Panics if a canonical complement exists but is not represented inside
    /// its scope, if scoped insertion fails, or if the overlap paths or
    /// complement occurrences are disconnected afterwards.
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
                shared,
                expansion,
                scope_path,
                expansion_path,
                left_width,
                right_width,
                existing,
            } => {
                validate_path(graph, scope, shared, &scope_path, false);
                validate_path(graph, expansion, shared, &expansion_path, true);
                let left = complement(graph, scope, 0, left_width);
                let offset = shared.width().0;
                let right = complement(graph, expansion, offset, right_width);
                // Insertion can rewrite the decompositions along both paths.
                let scope_path = boundary_path(graph, scope, shared, false)
                    .expect("scope must still represent the shared postfix");
                validate_path(graph, scope, shared, &scope_path, false);
                let expansion_path = boundary_path(
                    graph, expansion, shared, true,
                )
                .expect("expansion must still represent the shared prefix");
                validate_path(graph, expansion, shared, &expansion_path, true);
                assert_eq!(
                    find_represented(graph, scope, 0, left_width),
                    Some(left),
                    "left complement is not connected inside the scope"
                );
                assert_eq!(
                    find_represented(graph, expansion, offset, right_width),
                    Some(right),
                    "right complement is not connected inside the expansion"
                );
                let patterns = [
                    Pattern::from(vec![scope, right]),
                    Pattern::from(vec![left, expansion]),
                ];
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
/// Panics on missing or disconnected boundary paths, empty complements, or
/// if an existing closure token shares a planned boundary across distinct
/// decompositions.
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
    let expansion_path = boundary_path(graph, expansion, postfix, true)
        .expect("expansion must represent the overlap prefix");
    let left_width = scope.width().0 - postfix.width().0;
    let right_width = expansion.width().0 - postfix.width().0;
    assert!(
        left_width > 0 && right_width > 0,
        "closure requires proper complements"
    );
    // Content lookup `[X] + atoms(T)`; read-only.
    let mut content = vec![scope];
    content.extend_from_slice(&atoms(graph, expansion)[postfix.width().0..]);
    let existing = find_complete(graph, &Pattern::from(content));
    if let Some(token) = existing {
        let planned = [
            std::collections::BTreeSet::from([scope.width().0]),
            std::collections::BTreeSet::from([left_width]),
        ];
        for current in graph.expect_child_patterns(token).values() {
            let current = boundaries(current);
            for proposed in &planned {
                assert!(
                    &current == proposed || current.is_disjoint(proposed),
                    "closure would duplicate an existing boundary"
                );
            }
        }
    }
    ClosurePlan::Closed {
        root,
        scope,
        tail: editable,
        shared: postfix,
        expansion,
        scope_path: path.to_vec(),
        expansion_path,
        left_width,
        right_width,
        existing,
    }
}

/// Resolve the canonical complement of `width` atoms at `offset` in `scope`.
///
/// Reuses a connected occurrence, otherwise materializes it by scoped
/// insertion. Complements are always prefixes or postfixes of `scope`.
///
/// # Panics
///
/// Panics if the content already has a canonical token that is not
/// represented inside `scope`, or if scoped insertion fails.
fn complement(
    graph: &HypergraphRef,
    scope: Token,
    offset: usize,
    width: usize,
) -> Token {
    if let Some(token) = find_represented(graph, scope, offset, width) {
        return token;
    }
    let content = &atoms(graph, scope)[offset..offset + width];
    if let Some(token) = find_complete(graph, &Pattern::from(content.to_vec()))
    {
        panic!(
            "canonical complement {} is not represented in {}",
            graph.index_string(token),
            graph.index_string(scope)
        );
    }
    let inserted = if offset == 0 {
        insert_scope_prefix_width(graph, scope, width)
    } else {
        assert_eq!(offset + width, scope.width().0, "complement is an infix");
        insert_scope_postfix_width(graph, scope, width)
    };
    inserted.unwrap_or_else(|error| {
        panic!("scoped complement insertion failed: {:?}", error)
    })
}

/// Expand a token to its atoms through any decomposition.
fn atoms(
    graph: &HypergraphRef,
    token: Token,
) -> Vec<Token> {
    let patterns = graph.expect_child_patterns(token);
    match patterns.values().next() {
        None => vec![token],
        Some(pattern) => pattern
            .iter()
            .flat_map(|child| atoms(graph, *child))
            .collect(),
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
