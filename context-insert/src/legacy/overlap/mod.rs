//! Structural closure of an accepted overlap.
//!
//! ```text
//! anchor A:       L P
//! expansion E:      P T
//! closure C:      L P T = [A, T] = [L, E]
//! ```
//!
//! [`Overlap`] carries the structural evidence of one accepted overlap: the
//! closure bound `X` (the selected anchor scope), its retained postfix
//! frames down to the shared token `P`, the accepted expansion `E`, and the
//! prefix frames of `E` reaching the same `P`. [`Overlap::close`] integrates
//! `E` with `X` and returns their union token; root ownership and
//! installation stay with the caller.
//!
//! For frames `P = S0 < S1 < ... < Sk = X`, closure unwinds from `P`
//! outward:
//!
//! ```text
//! C0 = E
//! Si = Li + S(i-1)
//! Ci = [Si, T] = [Li, C(i-1)]
//! ```
//!
//! - `T` is resolved once from the expansion frames' right siblings and
//!   reused by every step.
//! - The closure steps `Si` are the parents of the supplied anchor frames.
//!   `Li` is resolved from the left siblings along the frames from `Si` to
//!   `S(i-1)`; a wrapper introduced by an earlier complement resolution
//!   lengthens those frames but is not a closure step.
//! - Complements come from the scoped split/join core
//!   ([`scope_complement`]); a single sibling is used directly. Every
//!   resolution returns its [`Publication`], through which the frames still
//!   to be closed are translated, so no frame is rediscovered by search or
//!   depth-first reconnection.
//! - An existing union is reused: first through a known decomposition of
//!   `Si` or `C(i-1)`, then by one structured content lookup of `[Si, T]`.
//!   Its decompositions are extended with the missing closure patterns.
//!   The two closure decompositions are added directly: join cannot plan a
//!   new union vertex, and reconciling `Si`'s other decompositions with it
//!   is deferred to affected-host completion.
//!
//! [`ClosurePolicy::SingleStep`] closes `X` with `E` in one step using the
//! whole frame path, as required when the expansion is the scope itself.
//! If `X == P`, the result is `E` and no complement is resolved.

use context_search::legacy::{
    ErrorState,
    Find,
};
use context_trace::legacy::*;

use crate::legacy::insert::{
    publication::Publication,
    scoped::{
        Boundary,
        scope_complement,
    },
};

/// Which retained anchor frames become closure steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClosurePolicy {
    /// Close every frame from the shared token outward.
    Nested,
    /// Close only the scope with the expansion, in one step.
    SingleStep,
}

/// One accepted overlap with the connections to its shared occurrence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Overlap {
    /// Closure bound `X`: the anchor scope integrated with the expansion.
    pub scope: Token,
    /// Frames from `scope` to the shared token through last children.
    /// Empty when the scope is the shared token.
    pub anchor_path: Vec<ChildLocation>,
    /// Accepted expansion `E`, beginning with the shared token.
    pub expansion: Token,
    /// Frames from `expansion` to the shared token through first children.
    pub expansion_path: Vec<ChildLocation>,
}

impl Overlap {
    /// The shared token `P` reached by the anchor frames.
    ///
    /// # Panics
    ///
    /// Panics if the anchor frames are disconnected.
    pub fn shared(
        &self,
        graph: &Hypergraph,
    ) -> Token {
        boundary_target(graph, self.scope, &self.anchor_path, false)
    }

    /// Close the overlap and return the union of `scope` and `expansion`.
    ///
    /// # Panics
    ///
    /// Panics if the expansion does not extend the shared token, if the
    /// frames are disconnected or do not reach the same token, if a
    /// translated frame cannot be resolved, or if an existing union would
    /// receive a duplicate boundary. Mutations are not rolled back.
    pub fn close(
        self,
        graph: &HypergraphRef,
        policy: ClosurePolicy,
    ) -> Result<Token, ErrorState> {
        let shared = self.shared(graph);
        assert!(
            self.expansion.width() > shared.width(),
            "overlap must consume input"
        );
        if self.anchor_path.is_empty() {
            return Ok(self.expansion);
        }
        assert_eq!(
            boundary_target(graph, self.expansion, &self.expansion_path, true),
            shared,
            "expansion frames do not reach the shared token"
        );
        let (right, publication) = scope_complement(
            graph,
            self.expansion,
            &self.expansion_path,
            Boundary::Postfix,
        )?;
        // Closure steps are the supplied frame parents, innermost last.
        // Wrappers introduced later by complement resolution only lengthen
        // the frames between two steps; they are not closure steps.
        let mut steps: Vec<Token> = match policy {
            ClosurePolicy::Nested =>
                self.anchor_path.iter().map(|l| l.parent).collect(),
            ClosurePolicy::SingleStep => vec![self.scope],
        };
        let mut frames =
            publication.translate(graph, &self.anchor_path, shared, false);
        let mut closed = self.expansion;
        while let Some(outer) = steps.pop() {
            let start = frames
                .iter()
                .rposition(|location| location.parent == outer)
                .expect("closure step must remain on its translated frames");
            let (left, publication) = scope_complement(
                graph,
                outer,
                &frames[start..],
                Boundary::Prefix,
            )?;
            frames = translate_outer(graph, &publication, &frames[..start], outer);
            closed = close_step(graph, outer, left, closed, right);
        }
        Ok(closed)
    }
}

/// Frames from `root` to its prefix `target` along the widest first child.
///
/// A prefix occurs once, at offset 0, so every first-child path identifies
/// the same occurrence; only the decomposition chosen per level differs.
/// Used to complete evidence whose trailing border frames were simplified
/// away, and by callers without search evidence. Mirrors the widest-postfix
/// traversal that selects overlap anchors. Returns `None` if `target` is not
/// reached along the widest first children.
pub fn prefix_frames(
    graph: &Hypergraph,
    root: Token,
    target: Token,
) -> Option<Vec<ChildLocation>> {
    let mut token = root;
    let mut path = Vec::new();
    while token != target {
        let (location, child) = graph
            .expect_child_patterns(token)
            .iter()
            .map(|(&id, pattern)| (ChildLocation::new(token, id, 0), pattern[0]))
            .max_by_key(|(_, child)| child.width())?;
        if child.width() < target.width() {
            return None;
        }
        path.push(location);
        token = child;
    }
    Some(path)
}

/// Translate the frames still to be closed, which end at `outer`.
fn translate_outer(
    graph: &Hypergraph,
    publication: &Publication,
    frames: &[ChildLocation],
    outer: Token,
) -> Vec<ChildLocation> {
    if publication.is_unchanged() {
        frames.to_vec()
    } else {
        publication.translate(graph, frames, outer, false)
    }
}

/// Close one frame: `C = [outer, right] = [left, closed]`.
fn close_step(
    graph: &HypergraphRef,
    outer: Token,
    left: Token,
    closed: Token,
    right: Token,
) -> Token {
    let patterns = [
        Pattern::from(vec![outer, right]),
        Pattern::from(vec![left, closed]),
    ];
    let Some(token) = existing_union(graph, &patterns) else {
        return graph.insert_patterns(patterns);
    };
    let planned = [
        std::collections::BTreeSet::from([outer.width().0]),
        std::collections::BTreeSet::from([left.width().0]),
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

/// Find an existing token for the union of a closure step.
///
/// Known decompositions are checked through the parents of their first
/// child; otherwise the structured pattern `[outer, right]` is looked up
/// once.
fn existing_union(
    graph: &HypergraphRef,
    patterns: &[Pattern; 2],
) -> Option<Token> {
    patterns
        .iter()
        .find_map(|pattern| parent_with_pattern(graph, pattern))
        .or_else(|| find_complete(graph, &patterns[0]))
}

fn parent_with_pattern(
    graph: &Hypergraph,
    pattern: &Pattern,
) -> Option<Token> {
    let vertex = graph.expect_vertex_data(pattern[0]);
    vertex.parents().iter().find_map(|(&index, parent)| {
        let token = Token::new(index, parent.width());
        graph
            .expect_child_patterns(token)
            .values()
            .any(|p| p == pattern)
            .then_some(token)
    })
}

/// Look up an entire token matching a nonempty pattern without
/// materialization.
///
/// Returns `None` for absent or partial matches; unexpected search errors
/// panic.
pub fn find_complete(
    graph: &HypergraphRef,
    pattern: &Pattern,
) -> Option<Token> {
    if pattern.len() == 1 {
        return Some(pattern[0]);
    }
    let width: usize = pattern.iter().map(|t| t.width().0).sum();
    match graph.find_ancestor(pattern.to_vec()) {
        Ok(response)
            if response.query_exhausted()
                && response.is_entire_root()
                && response.root_token().width().0 == width =>
            Some(response.root_token()),
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
