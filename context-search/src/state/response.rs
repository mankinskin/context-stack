use context_trace::*;

use crate::{
    cursor::{
        Matched,
        PatternCursor,
    },
    state::{
        end::PathCoverage,
        matched::MatchResult,
    },
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Response {
    pub cache: TraceCache,
    pub end: MatchResult,
}
//impl From<EndState> for Response {
//    fn from(state: EndState) -> Self {
//        if let PathEnum::Complete(c) = &state.kind {
//            Response::Complete(*c) // cursor.path
//        } else {
//            Response::Incomplete(Box::new(state))
//        }
//    }
//}

impl Response {
    //pub(crate) fn new(
    //    cache: TraceCache,
    //    end: MatchResult,
    //) -> Self {
    //    Self { cache, end }
    //}

    /// Check if the query was fully matched
    pub fn query_exhausted(&self) -> bool {
        self.end.query_exhausted()
    }

    /// Check if the result is a complete pre-existing token in the graph
    /// Returns true for PathCoverage::EntireRoot, false for Range/Prefix/Postfix
    pub fn is_full_token(&self) -> bool {
        self.end.is_full_token()
    }

    /// Unwrap a complete response with a custom error message
    #[track_caller]
    pub(crate) fn expect_entire_root(
        self,
        msg: &str,
    ) -> IndexRangePath {
        match self.end.path {
            PathCoverage::EntireRoot(path) => path,
            _ => panic!(
                "{}: Complete response has non-EntireRoot path: {:#?}",
                msg, self
            ),
        }
    }
    /// Unwrap a complete response with a custom error message
    #[track_caller]
    pub fn expect_complete(
        self,
        msg: &str,
    ) -> IndexRangePath {
        if !self.end.query_exhausted() {
            panic!("{}", msg);
        }
        self.expect_entire_root(msg)
    }

    /// Get the query pattern cursor from the response
    pub fn query_cursor(&self) -> &PatternCursor<Matched> {
        self.end.cursor()
    }

    /// Get the root token from the located path
    pub fn root_token(&self) -> Token {
        self.end.root_parent()
    }

    /// Get the cursor atom position
    /// Returns the candidate position if available, otherwise the checkpoint position.
    /// This is useful for consecutive searches.
    pub fn cursor_position(&self) -> AtomPosition {
        self.end.cursor().atom_position
    }
}

impl TargetKey for Response {
    fn target_key(&self) -> DirectedKey {
        DirectedKey::up(self.end.root_parent(), *self.end.root_parent().width())
    }
}
