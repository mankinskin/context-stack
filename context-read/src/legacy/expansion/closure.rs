//! Root integration of an accepted overlap closure.
//!
//! Read selects the closure bound `X` and the matching sink operation; the
//! graph-only closure itself is executed by [`context_insert::legacy::Overlap`]:
//!
//! - Editable root: `X` is its entire last child; commit replaces that tail.
//! - Otherwise: `X` is the root; commit adopts the whole-root closure.
//!
//! The bound is chosen after materialization, which can add decompositions
//! or parents to the root. A self-expansion (`E == X`) is closed in a single
//! step; every other overlap closes each retained anchor frame from the
//! shared token outward. Preparation is read-only; applying the plan may
//! mutate the graph but leaves accumulation updates to the sink. Neither
//! phase provides rollback.

use crate::legacy::pipeline::root::{
    RootManager,
    Span,
};
use context_insert::legacy::{
    ClosurePolicy,
    Overlap,
};
use context_trace::legacy::*;

/// Structural evidence of an accepted root expansion.
///
/// Locations are valid for the graph after materializing the expansion.
#[derive(Debug)]
pub(crate) struct Accepted {
    pub(crate) root: Token,
    /// Retained postfix frames from `root` to the shared token.
    pub(crate) anchor_path: Vec<ChildLocation>,
    pub(crate) expansion: Token,
    /// Prefix frames from `expansion` to the shared token.
    pub(crate) expansion_path: Vec<ChildLocation>,
}

/// Validated closure input and its intended root integration operation.
pub(crate) struct ClosurePlan {
    integration: Integration,
    overlap: Overlap,
    policy: ClosurePolicy,
}

enum Integration {
    Tail { root: Token, old: Token },
    Root { previous: Token },
}

impl Integration {
    fn into_span(
        self,
        token: Token,
    ) -> Span {
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
    /// Close the overlap and return the sink operation.
    ///
    /// Direct tail expansion (`X == P`) does not modify the graph.
    ///
    /// # Panics
    ///
    /// Panics if the structural closure fails (see [`Overlap::close`]).
    pub(crate) fn apply(
        self,
        graph: &HypergraphRef,
    ) -> Span {
        let token = self
            .overlap
            .close(graph, self.policy)
            .unwrap_or_else(|error| panic!("overlap closure failed: {:?}", error));
        self.integration.into_span(token)
    }
}

/// Select the closure bound and sink operation for an accepted expansion.
///
/// # Panics
///
/// Panics on disconnected anchor frames or a non-consuming expansion.
pub(crate) fn prepare(
    graph: &HypergraphRef,
    sink: &RootManager,
    accepted: Accepted,
) -> ClosurePlan {
    let Accepted {
        root,
        anchor_path,
        expansion,
        expansion_path,
    } = accepted;
    let shared = boundary_target(graph, root, &anchor_path, false);
    assert!(
        expansion.width() > shared.width(),
        "overlap must consume input"
    );
    let editable = sink.is_editable(root)
        && expansion.vertex_index() != root.vertex_index();
    let (scope, anchor_path) = if editable {
        let tail = anchor_path.first().expect("postfix path must start at root");
        assert_eq!(tail.parent, root, "postfix path must start at root");
        let scope = graph.expect_pattern_at(*tail)[tail.sub_index];
        (scope, anchor_path[1..].to_vec())
    } else {
        (root, anchor_path)
    };
    let policy = if expansion.vertex_index() == scope.vertex_index() {
        ClosurePolicy::SingleStep
    } else {
        ClosurePolicy::Nested
    };
    let integration = if editable {
        Integration::Tail { root, old: scope }
    } else {
        Integration::Root { previous: root }
    };
    ClosurePlan {
        integration,
        overlap: Overlap {
            scope,
            anchor_path,
            expansion,
            expansion_path,
        },
        policy,
    }
}

/// Derive accepted-overlap evidence as expansion does: anchor frames from
/// the widest-postfix traversal, expansion frames from the widest prefix.
#[cfg(test)]
pub(crate) fn accept(
    graph: &HypergraphRef,
    root: Token,
    shared: Token,
    expansion: Token,
) -> Accepted {
    let mut anchor_path = Vec::new();
    for (location, postfix) in root.postfix_iter(graph.clone()) {
        anchor_path.push(location);
        if postfix == shared {
            return Accepted {
                root,
                anchor_path,
                expansion,
                expansion_path: context_insert::legacy::overlap::prefix_frames(
                    graph, expansion, shared,
                )
                .expect("expansion must begin with the shared token"),
            };
        }
    }
    panic!("shared token is not on the root's widest-postfix chain")
}
