use context_trace::*;

use crate::state::{
    end::PathCoverage,
    matched::MatchedEndState,
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Response {
    pub cache: TraceCache,
    pub end: MatchedEndState,
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
    //    end: MatchedEndState,
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

    /// Unwrap a complete response, panicking if incomplete
    pub fn unwrap_complete(self) -> IndexRangePath {
        self.expect_complete("Called unwrap_complete on incomplete Response")
    }

    /// Unwrap a complete response with a custom error message
    pub fn expect_complete(
        self,
        msg: &str,
    ) -> IndexRangePath {
        if !self.end.query_exhausted() {
            panic!("{}", msg);
        }
        match self.end.path {
            PathCoverage::EntireRoot(path) => path,
            _ => panic!("{}: Complete response has non-EntireRoot path", msg),
        }
    }

    /// Try to get the complete path if the response is complete
    pub fn as_complete(&self) -> Option<&IndexRangePath> {
        if !self.end.query_exhausted() {
            return None;
        }
        match &self.end.path {
            PathCoverage::EntireRoot(path) => Some(path),
            _ => None,
        }
    }

    /// Get the query pattern cursor from the response
    pub fn query_cursor(&self) -> &crate::cursor::PatternCursor {
        self.end.cursor()
    }

    /// Get the query pattern path from the response
    pub fn query_pattern(&self) -> &PatternRangePath {
        &self.end.cursor().path
    }

    /// Get the root token from the located path
    pub fn root_token(&self) -> Token {
        self.end.root_parent()
    }

    /// Get the cursor atom position
    pub fn cursor_position(&self) -> AtomPosition {
        self.end.cursor().atom_position
    }
}

impl TargetKey for Response {
    fn target_key(&self) -> DirectedKey {
        DirectedKey::up(self.end.root_parent(), *self.end.root_parent().width())
    }
}
