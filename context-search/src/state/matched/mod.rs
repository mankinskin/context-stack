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
pub struct MatchedEndState {
    /// The path in the graph where the match occurred
    pub path: PathCoverage,
    /// The cursor indicating position in the query pattern
    pub cursor: PatternCursor,
}

impl MatchedEndState {
    /// Get the path
    pub fn path(&self) -> &PathCoverage {
        &self.path
    }

    /// Get the cursor (checkpoint)
    pub fn cursor(&self) -> &PatternCursor {
        &self.cursor
    }

    /// Get the root parent token
    pub fn root_parent(&self) -> Token {
        self.path.root_parent()
    }

    /// Check if the query was fully matched
    /// Returns true if cursor position equals the query length
    pub fn query_exhausted(&self) -> bool {
        use std::borrow::Borrow;
        let query_pattern = self.cursor.path.pattern_root_pattern();
        let query_tokens: &[Token] = query_pattern.borrow();
        let query_length = query_tokens.len();
        let checkpoint_pos = *self.cursor.atom_position.as_ref();
        checkpoint_pos >= query_length
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

impl Traceable for &MatchedEndState {
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

impl RootKey for MatchedEndState {
    fn root_key(&self) -> UpKey {
        self.path().root_key()
    }
}

impl GraphRoot for MatchedEndState {
    fn root_parent(&self) -> Token {
        self.path().root_parent()
    }
}
