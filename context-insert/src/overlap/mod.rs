//! Overlap bundling scaffolding for path-driven structural collapse.
//!
//! This module is the first landing zone for the overlap-complement redesign.
//! The long-term goal is:
//!
//! 1. Accept structural path witnesses from `context-read`
//! 2. Convert those paths into the trace-cache representation required by
//!    `insert_init`
//! 3. Use recursive split/join to construct the structural partitions around
//!    the shared overlap token
//! 4. Bundle the resulting decompositions into a single token
//!
//! This file intentionally starts as a scaffold so the API surface can be
//! introduced in `context-insert` before the full path-to-cache conversion is
//! implemented.

use context_trace::*;

/// Result of extracting one structural partition around a shared overlap token.
///
/// In the `context-read` overlap pipeline, the expected case is `Token(_)`,
/// because only true overlap paths with a non-empty complement should be used.
/// `Empty` exists as a graceful recovery outcome for more general callers and
/// for defensive handling while the API is still being integrated.
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
        }
    }
}

/// Private side marker used internally while the path-based partition helpers
/// are still side-specific in the design.
///
/// This stays module-private for now to avoid freezing the wrong abstraction
/// publicly before the path→trace-cache conversion is proven in tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PartitionSide {
    Left,
    Right,
}

/// Build the structural left partition of the shared overlap token inside the
/// old anchor.
///
/// This is the token represented by all siblings to the **left** of the leaf
/// reached by `path`, preserving the hierarchy described by the path.
///
/// # Current status
///
/// This is a scaffold. The final implementation will:
///
/// 1. Convert `path` into a trace-cache representation
/// 2. Use recursive split/join through `insert_init`
/// 3. Return the resulting partition token or `Empty`
///
/// For now this function exists to establish the API and call sites.
pub(crate) fn left_partition_from_postfix_path(
    _graph: &HypergraphRef,
    _path: &IndexEndPath,
) -> Result<PartitionOutcome, ErrorReason> {
    Err(ErrorReason::NotFound)
}

/// Build the structural right partition of the shared overlap token inside the
/// overlap expansion token.
///
/// This is the token represented by all siblings to the **right** of the leaf
/// reached by `path`, preserving the hierarchy described by the path.
///
/// # Current status
///
/// This is a scaffold. The final implementation will:
///
/// 1. Convert `path` into a trace-cache representation
/// 2. Use recursive split/join through `insert_init`
/// 3. Return the resulting partition token or `Empty`
///
/// The overlap-side path conversion may need small semantic adjustments to
/// ensure the partition is taken relative to the leaf overlap token inside
/// `t2`.
pub(crate) fn right_partition_from_prefix_path(
    _graph: &HypergraphRef,
    _path: &IndexStartPath,
) -> Result<PartitionOutcome, ErrorReason> {
    Err(ErrorReason::NotFound)
}

/// Internal helper that will eventually own the path→trace-cache conversion for
/// one requested side of a structural overlap path.
///
/// This remains private while the semantics are still being validated.
/// Callers should use the higher-level partition helpers or `bundle_overlap`.
fn partition_from_path<P>(
    _graph: &HypergraphRef,
    _path: &P,
    _side: PartitionSide,
) -> Result<PartitionOutcome, ErrorReason> {
    Err(ErrorReason::NotFound)
}

/// Bundle an overlap into a single token.
///
/// This is the intended durable API entry point for `context-read`.
/// Once fully implemented, `context-read` should be able to delegate overlap
/// collapse to this function instead of manually constructing left/right
/// complements.
///
/// The final implementation will:
///
/// 1. Derive the left partition from `anchor_postfix_path`
/// 2. Derive the right partition from `overlap_prefix_path`
/// 3. Construct the required decompositions around the shared overlap token
/// 4. Insert those decompositions and return the bundled token
///
/// # Current status
///
/// This is an API scaffold and currently returns `ErrorReason::NotFound`.
pub fn bundle_overlap(
    graph: &HypergraphRef,
    input: OverlapBundleInput,
) -> Result<Token, ErrorReason> {
    let _ =
        left_partition_from_postfix_path(graph, &input.anchor_postfix_path)?;
    let _ =
        right_partition_from_prefix_path(graph, &input.overlap_prefix_path)?;
    let _ = partition_from_path(
        graph,
        &input.anchor_postfix_path,
        PartitionSide::Left,
    )?;
    let _ = partition_from_path(
        graph,
        &input.overlap_prefix_path,
        PartitionSide::Right,
    )?;

    Err(ErrorReason::NotFound)
}
