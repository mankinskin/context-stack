//! Factored closure of a canonical root-postfix/expansion-prefix overlap.
//!
//! For scope `X = L + P` and expansion `E = P + T`, close the overlap
//! bottom-up along the widest-postfix chain `P = S0 < S1 < ... < Sk = X`:
//! `C0 = E` and `Ci = [Si,T] = [Li,C(i-1)]` with `Si = Li + S(i-1)`, so every
//! existing suffix token of `X` wider than `P` is closed with the expansion.
//! The complements `Li` and `T` are canonical tokens represented inside their
//! smallest scopes: `Li` at offset 0 of `Si` and `T` at offset `|P|` of `E`.
//! `T` is resolved once and reused by every step. Existing represented
//! occurrences are reused; otherwise they are materialized by scope-restricted
//! insertion, which rewrites the scope's decompositions so the occurrence is
//! connected.
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
pub(crate) struct ClosurePlan {
    integration: Integration,
    /// `P`: the shared overlap token.
    shared: Token,
    /// `E = P + T`: the accepted expansion.
    expansion: Token,
    /// Ascending postfix tokens `S1 < ... < Sk = X`; empty when `X = P`.
    chain: Vec<Token>,
}

enum Integration {
    Tail { root: Token, old: Token },
    Root { previous: Token },
}

impl Integration {
    fn into_span(self, token: Token) -> Span {
        match self {
            Self::Tail { root, old } => Span::ReplaceTail {
                previous: root,
                old,
                token,
            },
            Self::Root { previous } => Span::ReplaceRoot { previous, token },
        }
    }
}

impl ClosurePlan {
    /// Materialize the prepared closure and return the sink operation.
    ///
    /// Resolves `T` inside `E` once, then closes each chain token `Si`
    /// bottom-up with the previous closure, adding the decompositions
    /// `[Si,T]` and `[Li,C(i-1)]` to an existing canonical result or
    /// allocating it. Direct tail expansion does not modify the graph here.
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
        let token = close_nested(graph, self.shared, self.expansion, self.chain);
        self.integration.into_span(token)
    }
}

fn close_nested(
    graph: &HypergraphRef,
    shared: Token,
    expansion: Token,
    chain: Vec<Token>,
) -> Token {
    // An entire-tail expansion needs no complements or graph mutations.
    if chain.is_empty() {
        return expansion;
    }
    let offset = shared.width().0;
    let right_width = expansion.width().0 - offset;
    let right = complement(graph, expansion, offset, right_width);
    assert_eq!(
        find_represented(graph, expansion, offset, right_width),
        Some(right),
        "right complement is not connected inside the expansion"
    );
    let (mut closed, mut inner) = (expansion, shared);
    for outer in chain {
        closed = close_step(graph, outer, inner, closed, right);
        inner = outer;
    }
    closed
}

/// Close one chain step: `C = [outer,right] = [left,closed]`.
///
/// `inner` is the next smaller chain token, a postfix of `outer` and the
/// prefix of `closed`; `left = outer - inner` is resolved inside `outer`.
fn close_step(
    graph: &HypergraphRef,
    outer: Token,
    inner: Token,
    closed: Token,
    right: Token,
) -> Token {
    let left_width = outer.width().0 - inner.width().0;
    let left = complement(graph, outer, 0, left_width);
    // Insertion can rewrite the decompositions along both paths.
    let outer_path = boundary_path(graph, outer, inner, false)
        .expect("chain token must still represent its inner postfix");
    validate_path(graph, outer, inner, &outer_path, false);
    let closed_path = boundary_path(graph, closed, inner, true)
        .expect("closure must still represent the shared prefix");
    validate_path(graph, closed, inner, &closed_path, true);
    assert_eq!(
        find_represented(graph, outer, 0, left_width),
        Some(left),
        "left complement is not connected inside the scope"
    );
    // Content lookup `[outer] + atoms(right)`; read-only.
    let mut content = vec![outer];
    content.extend(atoms(graph, right));
    let existing = find_complete(graph, &Pattern::from(content));
    let patterns = [
        Pattern::from(vec![outer, right]),
        Pattern::from(vec![left, closed]),
    ];
    match existing {
        Some(token) => {
            let planned = [
                std::collections::BTreeSet::from([outer.width().0]),
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
    }
}

/// Prepare closure using current ownership and connected overlap paths.
///
/// Call after materializing `expansion` and reconnecting `root_path`.
/// Editable roots close around their entire last child; non-editable roots
/// and self-expansions close around the root. Self-expansions close the root
/// in one step; otherwise the widest-postfix chain of the scope down to the
/// shared token is closed bottom-up. This function does not mutate.
///
/// # Panics
///
/// Panics on missing or disconnected boundary paths, or
/// if the shared token is not on the scope's widest-postfix chain.
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
    validate_path(graph, scope, postfix, path, false);
    let chain = if expansion.vertex_index() == scope.vertex_index() {
        vec![scope]
    } else {
        postfix_chain(graph, scope, postfix)
    };
    if !chain.is_empty() {
        boundary_path(graph, expansion, postfix, true)
            .expect("expansion must represent the overlap prefix");
    }
    let integration = if editable {
        Integration::Tail { root, old: scope }
    } else {
        Integration::Root { previous: root }
    };
    ClosurePlan {
        integration,
        shared: postfix,
        expansion,
        chain,
    }
}

/// Collect the widest-postfix chain of `scope` strictly wider than `shared`.
///
/// Returns tokens in ascending width ending with `scope`, or an empty chain
/// when `scope == shared`. Boundary witnesses may skip intermediate postfixes,
/// so the widest terminal child is followed.
fn postfix_chain(
    graph: &HypergraphRef,
    scope: Token,
    shared: Token,
) -> Vec<Token> {
    if scope == shared {
        return Vec::new();
    }
    let mut chain = vec![scope];
    for (_, postfix) in scope.postfix_iter(graph.clone()) {
        if postfix.width() <= shared.width() {
            assert_eq!(
                postfix, shared,
                "shared token is not on the scope's widest-postfix chain"
            );
            chain.reverse();
            return chain;
        }
        chain.push(postfix);
    }
    panic!("shared token is not on the scope's widest-postfix chain")
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
    assert_eq!(
        boundary_target(graph, root, path, prefix),
        target,
        "overlap path does not reach shared token"
    );
}
