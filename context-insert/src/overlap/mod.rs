//! Overlap bundling for path-driven structural collapse.
//!
//! This module provides the first working implementation of structural overlap
//! bundling for `context-read`.
//!
//! ## Design
//!
//! Given:
//! - an old-anchor postfix path to the shared overlap token `P`
//! - an overlap-side prefix path to the same token `P` inside `t2`
//! - the participating tokens `t1` and `t2`
//!
//! we construct:
//! - the structural left partition of the anchor around `P`
//! - the structural right partition of `t2` around `P`
//!
//! by collecting hierarchical siblings along the supplied paths.
//!
//! This is intentionally implemented as a direct hierarchical sibling
//! collection first. It establishes the semantics needed by the overlap
//! collapse path while keeping the API in `context-insert`, where the
//! longer-term path→trace-cache→split/join implementation belongs.
//!
//! ## Important note
//!
//! The design documents describe a future implementation based on converting
//! paths into `TraceCache` and routing through recursive split/join. This file
//! implements the same *structural semantics* but does so directly by walking
//! the path hierarchy and inserting the collected sibling patterns.
//!
//! That gives `context-read` a durable overlap-bundling entry point now, while
//! leaving room to swap the internals to trace-cache-based split/join later
//! without changing the call site.

use context_trace::{
    path::accessors::has_path::HasRolePath,
    *,
};

/// Result of extracting one structural partition around a shared overlap token.
///
/// In the `context-read` overlap pipeline, the expected case is `Token(_)`,
/// because only true overlap paths with a non-empty complement should be used.
/// `Empty` exists as a graceful recovery outcome for more general callers and
/// for defensive handling.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PartitionOutcome {
    /// A non-empty structural partition was successfully constructed.
    Token(Token),
    /// The requested side of the path contains no siblings.
    Empty,
}

impl PartitionOutcome {
    /// Return the contained token when present.
    pub fn token(self) -> Option<Token> {
        match self {
            Self::Token(token) => Some(token),
            Self::Empty => None,
        }
    }

    /// Borrow the contained token when present.
    pub fn as_token(&self) -> Option<Token> {
        match self {
            Self::Token(token) => Some(*token),
            Self::Empty => None,
        }
    }

    /// `true` when the partition is empty.
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }
}

/// Input bundle describing one overlap collapse operation.
///
/// The shared overlap token is represented structurally by two path witnesses:
///
/// - `anchor_postfix_path`: path from the old anchor root to the shared overlap
///   token `P`
/// - `overlap_prefix_path`: path inside the overlap token `t2` to the same
///   shared token `P`
///
/// `t1` and `t2` are the two decomposition tokens currently used by
/// `context-read`'s overlap collapse path:
///
/// - `t1`: the sequential token matched from the current cursor position
/// - `t2`: the expanded overlap token that contains the shared token `P`
#[derive(Clone, Debug)]
pub struct OverlapBundleInput {
    /// Path from the old anchor root to the selected shared overlap token.
    pub anchor_postfix_path: IndexEndPath,
    /// Path inside the overlap expansion to the selected shared overlap token.
    pub overlap_prefix_path: IndexStartPath,
    /// Sequential expansion token from the current cursor position.
    pub t1: Token,
    /// Overlap expansion token containing the shared overlap token.
    pub t2: Token,
    /// True when the overlap token IS the anchor (repetition pattern).
    /// In this case the standard partition formula double-counts and a
    /// specialized decomposition is used instead.
    pub self_overlap: bool,
}

impl OverlapBundleInput {
    /// Create a new overlap bundle input.
    pub fn new(
        anchor_postfix_path: IndexEndPath,
        overlap_prefix_path: IndexStartPath,
        t1: Token,
        t2: Token,
    ) -> Self {
        Self {
            anchor_postfix_path,
            overlap_prefix_path,
            t1,
            t2,
            self_overlap: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PartitionSide {
    Left,
    Right,
}

/// Bundle an overlap into a single token.
///
/// This is the durable API entry point intended for `context-read`.
///
/// Semantics:
/// - find the structural left partition of the old anchor around the shared
///   overlap token
/// - find the structural right partition of `t2` around the same token
/// - build the two bundled decompositions
/// - insert and return the resulting bundled token
///
/// The current implementation is based on hierarchical sibling collection from
/// the supplied paths.
pub fn bundle_overlap(
    graph: &HypergraphRef,
    input: OverlapBundleInput,
) -> Result<Token, ErrorReason> {
    let anchor = input.anchor_postfix_path.root_parent();

    // Self-overlap: the overlap token IS the anchor (repetition pattern).
    // The standard formula double-counts because left/right partitions
    // are extracted from what is effectively the same token.
    //
    // Instead, build:
    //   primary     = [anchor, t1]
    //   alternative = [left_partition, join(shared, t1)]
    //
    // Example: anchor=[abc,abc]=abcabc, postfix=abc, t1=abc
    //   left = abc, join(abc, abc) = abcabc
    //   primary = [abcabc, abc], alternative = [abc, abcabc]
    if input.self_overlap {
        // In the self-overlap case, the shared token is the postfix
        // of the anchor — the token at the root of the anchor postfix path.
        let shared: Token = GraphRootChild::<End>::graph_root_child(&input.anchor_postfix_path, graph);
        let left = left_partition_from_postfix_path(
            graph,
            &input.anchor_postfix_path,
        )?;

        // Find or create the right extension token [shared, t1].
        // In the common repetition case (t1 == postfix), this is the
        // anchor itself, which already exists in the graph.
        use crate::insert::{ToInsertCtx, outcome::InsertOutcome};
        let right_ext: Token =
            match <HypergraphRef as ToInsertCtx<IndexWithPath>>::insert_next_match(
                graph,
                vec![shared, input.t1],
            ) {
                Ok(o) => {
                    let outcome: InsertOutcome = o;
                    outcome.token()
                },
                Err(_) => graph.insert_pattern(vec![shared, input.t1]),
            };

        let primary_pat = vec![anchor, input.t1];
        let overlap_pat = match left.token() {
            Some(l) => vec![l, right_ext],
            None => vec![right_ext],
        };

        return Ok(graph.insert_patterns(vec![primary_pat, overlap_pat]));
    }

    let shared = shared_overlap_token(graph, &input.overlap_prefix_path);

    let left =
        left_partition_from_postfix_path(graph, &input.anchor_postfix_path)?;
    let right =
        right_partition_from_prefix_path(graph, &input.overlap_prefix_path)?;

    let primary = join_non_empty(
        graph,
        [
            left.as_token(),
            Some(shared),
            right.as_token(),
            Some(input.t1),
        ],
    )
    .ok_or(ErrorReason::NotFound)?;

    let overlap = join_non_empty(
        graph,
        [left.as_token(), Some(input.t2), right.as_token()],
    )
    .ok_or(ErrorReason::NotFound)?;

    Ok(graph.insert_patterns(vec![vec![primary], vec![overlap]]))
}

/// Build the structural left partition of the shared overlap token inside the
/// old anchor.
///
/// This is the token represented by all siblings to the **left** of the leaf
/// reached by `path`, preserving the hierarchy described by the path.
pub(crate) fn left_partition_from_postfix_path(
    graph: &HypergraphRef,
    path: &IndexEndPath,
) -> Result<PartitionOutcome, ErrorReason> {
    partition_from_path(graph, path, PartitionSide::Left)
}

/// Build the structural right partition of the shared overlap token inside the
/// overlap expansion token.
///
/// This is the token represented by all siblings to the **right** of the leaf
/// reached by `path`, preserving the hierarchy described by the path.
pub(crate) fn right_partition_from_prefix_path(
    graph: &HypergraphRef,
    path: &IndexStartPath,
) -> Result<PartitionOutcome, ErrorReason> {
    partition_from_path(graph, path, PartitionSide::Right)
}

fn partition_from_path<R: PathRole>(
    graph: &HypergraphRef,
    path: &RootedRolePath<R, IndexRoot>,
    side: PartitionSide,
) -> Result<PartitionOutcome, ErrorReason> {
    let steps = flatten_path_steps(path);
    if steps.is_empty() {
        return Ok(PartitionOutcome::Empty);
    }

    let mut acc: Option<Token> = None;

    for step in steps.iter().rev() {
        let pattern = graph.expect_pattern_at(step.pattern_location);
        let sibling_slice: &[Token] = match side {
            PartitionSide::Left => &pattern[..step.sub_index],
            PartitionSide::Right => &pattern[step.sub_index + 1..],
        };

        let siblings = sibling_slice.iter().copied();

        let parts: Vec<Token> = match side {
            PartitionSide::Left => siblings.chain(acc.into_iter()).collect(),
            PartitionSide::Right => acc.into_iter().chain(siblings).collect(),
        };

        acc = join_non_empty_vec(graph, parts);
    }

    Ok(match acc {
        Some(token) => PartitionOutcome::Token(token),
        None => PartitionOutcome::Empty,
    })
}

#[derive(Clone, Copy, Debug)]
struct PathStep {
    pattern_location: PatternLocation,
    sub_index: usize,
}

fn flatten_path_steps<R: PathRole>(
    path: &RootedRolePath<R, IndexRoot>
) -> Vec<PathStep> {
    use context_trace::{
        HasChildPath,
        RootedPath,
    };

    let child_path = path.child_path();
    let mut steps = Vec::with_capacity(child_path.len() + 1);

    steps.push(PathStep {
        pattern_location: path.root_pattern_location(),
        sub_index: path.role_path().root_child_index(),
    });

    for loc in child_path.iter().copied() {
        steps.push(PathStep {
            pattern_location: loc.into_pattern_location(),
            sub_index: loc.sub_index,
        });
    }

    steps
}

fn shared_overlap_token(
    graph: &HypergraphRef,
    path: &IndexStartPath,
) -> Token {
    path.role_leaf_token::<Start, _>(graph)
        .or_else(|| path.leaf_token(graph))
        .or_else(|| Some(path.graph_root_child(graph)))
        .expect("overlap path should always identify a shared overlap token")
}

fn join_non_empty<const N: usize>(
    graph: &HypergraphRef,
    parts: [Option<Token>; N],
) -> Option<Token> {
    let collected: Vec<Token> = parts.into_iter().flatten().collect();
    join_non_empty_vec(graph, collected)
}

fn join_non_empty_vec(
    graph: &HypergraphRef,
    parts: Vec<Token>,
) -> Option<Token> {
    match parts.len() {
        0 => None,
        1 => Some(parts[0]),
        _ => Some(graph.insert_pattern(parts)),
    }
}
