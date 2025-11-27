//! Matched state type
//!
//! Represents the result of matching a query pattern against the graph.
//! The checkpoint position in the cursor indicates how many query tokens matched.

use crate::{
    cursor::{
        checkpointed::{
            AtCheckpoint,
            Checkpointed,
            HasCandidate,
        },
        Matched,
        PatternCursor,
    },
    state::end::PathCoverage,
};
use context_trace::*;

/// Checkpointed cursor state for MatchResult
///
/// This enum encodes whether the match has an advanced candidate (from parent exploration)
/// or just a checkpoint (no further exploration).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CheckpointedCursor {
    /// At checkpoint - no candidate, only the confirmed match position
    AtCheckpoint(Checkpointed<PatternCursor<Matched>, AtCheckpoint>),
    /// Has candidate - advanced position from parent exploration
    HasCandidate(Checkpointed<PatternCursor<Matched>, HasCandidate>),
}

impl CheckpointedCursor {
    /// Get the checkpoint cursor (always available)
    pub fn checkpoint(&self) -> &PatternCursor<Matched> {
        match self {
            CheckpointedCursor::AtCheckpoint(c) => c.checkpoint(),
            CheckpointedCursor::HasCandidate(c) => c.checkpoint(),
        }
    }

    /// Get the cursor for consecutive searches
    ///
    /// Returns the candidate (advanced position) if available, otherwise the checkpoint.
    /// This allows consecutive searches to continue from where parent exploration left off.
    pub fn cursor(&self) -> &PatternCursor<Matched> {
        match self {
            CheckpointedCursor::AtCheckpoint(c) => c.checkpoint(),
            CheckpointedCursor::HasCandidate(c) => c.candidate(),
        }
    }

    /// Check if this has a candidate (advanced position from parent exploration)
    pub fn has_candidate(&self) -> bool {
        matches!(self, CheckpointedCursor::HasCandidate(_))
    }
}

/// A matched state - query matched at least partially in this root
///
/// The cursor can be either at checkpoint (no candidate) or have a candidate (from parent exploration).
/// Use query_exhausted() to check if the entire query was matched.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchResult {
    /// The path in the graph where the match occurred
    pub path: PathCoverage,
    /// The checkpointed cursor (either AtCheckpoint or HasCandidate)
    pub cursor: CheckpointedCursor,
}
impl GraphRoot for MatchResult {
    fn root_parent(&self) -> Token {
        self.path.root_parent()
    }
}
impl MatchResult {
    /// Get the path
    pub fn path(&self) -> &PathCoverage {
        &self.path
    }

    /// Get the cursor for consecutive searches
    ///
    /// Returns the candidate (advanced position) if available, otherwise the checkpoint.
    /// This allows consecutive searches to continue from where parent exploration left off.
    pub fn cursor(&self) -> &PatternCursor<Matched> {
        self.cursor.cursor()
    }

    /// Check if the query was fully matched
    /// Returns true if the cursor's path has reached the end of the pattern
    /// and there are no more tokens to traverse
    pub fn query_exhausted(&self) -> bool {
        use context_trace::{
            path::accessors::role::End,
            HasPath,
            HasRootChildIndex,
        };
        // Check checkpoint position
        let checkpoint = self.cursor.checkpoint();
        let at_end = checkpoint.path.is_at_pattern_end();
        let path_empty = HasPath::path(checkpoint.path.end_path()).is_empty();
        let end_index =
            HasRootChildIndex::<End>::root_child_index(&checkpoint.path);
        tracing::debug!(
            at_end,
            path_empty,
            end_index,
            end_path_len=%HasPath::path(checkpoint.path.end_path()).len(),
            "query_exhausted check"
        );
        at_end && path_empty
    }

    /// Check if the result is a complete pre-existing token in the graph
    /// Returns true for PathCoverage::EntireRoot (full token match),
    /// false for Range/Prefix/Postfix (intersection paths within tokens)
    pub fn is_full_token(&self) -> bool {
        matches!(self.path, PathCoverage::EntireRoot(_))
    }

    ///// Extract IndexRangePath and cursor for parent state generation
    ///// Returns None for Prefix paths (complex path type)
    //fn to_parent_state(&self) -> Option<(IndexRangePath, PatternCursor)> {
    //    use crate::state::end::{
    //        postfix::PostfixEnd,
    //        range::RangeEnd,
    //    };

    //    match &self.path {
    //        PathCoverage::EntireRoot(p) =>
    //            Some((p.clone(), self.cursor.clone())),
    //        PathCoverage::Range(r) =>
    //            Some((r.path.clone(), self.cursor.clone())),
    //        PathCoverage::Postfix(p) => {
    //            // Convert IndexStartPath to IndexRangePath using From trait
    //            let range_path: IndexRangePath = p.path.clone().into();
    //            Some((range_path, self.cursor.clone()))
    //        },
    //        PathCoverage::Prefix(_) => None, // Defer complex path type
    //    }
    //}

    /// Get start path length for incremental tracing
    pub fn start_len(&self) -> usize {
        self.path().start_len()
    }
}

impl Traceable for &MatchResult {
    fn trace<G: HasGraph>(
        self,
        ctx: &mut TraceCtx<G>,
    ) {
        (&self.path).trace(ctx)
    }
}

impl Traceable for &PathCoverage {
    fn trace<G: HasGraph>(
        self,
        ctx: &mut TraceCtx<G>,
    ) {
        match self {
            PathCoverage::Range(p) => p.trace(ctx),
            PathCoverage::Prefix(p) => p.trace(ctx),
            PathCoverage::Postfix(p) => p.trace(ctx),
            _ => {},
        }
    }
}

impl RootKey for MatchResult {
    fn root_key(&self) -> UpKey {
        self.path().root_key()
    }
}
