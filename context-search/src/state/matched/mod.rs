//! Matched state type
//!
//! Represents the result of matching a query pattern against the graph.
//! The checkpoint position in the cursor indicates how many query tokens matched.

use crate::{
    cursor::PatternCursor,
    state::end::PathCoverage,
};
use context_trace::*;

/// A matched state - query matched at least partially in this root
///
/// The cursor's atom_position indicates how far into the query pattern we matched.
/// Use query_exhausted() to check if the entire query was matched.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchResult {
    /// The path in the graph where the match occurred
    pub path: PathCoverage,
    /// The cursor indicating position in the query pattern
    pub cursor: PatternCursor,
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

    /// Get the cursor (checkpoint)
    pub fn cursor(&self) -> &PatternCursor {
        &self.cursor
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
        let at_end = self.cursor.path.is_at_pattern_end();
        let path_empty = HasPath::path(self.cursor.path.end_path()).is_empty();
        let end_index =
            HasRootChildIndex::<End>::root_child_index(&self.cursor.path);
        tracing::debug!(
            at_end,
            path_empty,
            end_index,
            end_path_len=%HasPath::path(self.cursor.path.end_path()).len(),
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
