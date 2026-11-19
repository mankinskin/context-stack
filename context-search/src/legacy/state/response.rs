use context_trace::legacy::{
    graph::visualization::{
        GraphOpEvent,
        Transition,
    },
    *,
};

use crate::legacy::{
    cursor::{
        Matched,
        PatternCursor,
    },
    state::{
        end::PathCoverage,
        matched::MatchResult,
    },
};

#[derive(Debug, Clone, Eq)]
pub struct Response {
    pub cache: TraceCache,
    pub end: MatchResult,
    /// Collected graph-op events emitted during the search.
    /// Available for test assertions and post-hoc inspection.
    pub events: Vec<GraphOpEvent>,
}

/// PartialEq compares only `cache` and `end`, ignoring `events`.
/// This lets existing tests compare structural results without
/// enumerating the full event trace.  Use `response.events` or
/// `response.transitions()` to assert on events separately.
impl PartialEq for Response {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.cache == other.cache && self.end == other.end
    }
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
    pub fn is_entire_root(&self) -> bool {
        self.end.is_entire_root()
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
                "{}: Expected EntireRoot path. Got PathCoverage::{}:\nend: {:#?},\n##### TRACE CACHE #####\n{:#?}##### END #####",
                msg, self.end.path.as_ref(), self.end, self.cache
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

    /// Get the confirmed match position, excluding any unconfirmed
    /// candidate advance.
    pub fn checkpoint_position(&self) -> AtomPosition {
        self.end.cursor.checkpoint().atom_position
    }

    /// Locations from a whole-root match to the start of the query.
    ///
    /// Starts with the matched root decomposition and its start entry,
    /// followed by the recorded start path. Trailing frames at the start
    /// border (first children) were simplified away by the search, so the
    /// remaining occurrence lies at offset 0 of the last returned child.
    /// Returns `None` unless the match covers its entire root.
    pub fn entire_root_start_path(&self) -> Option<Vec<ChildLocation>> {
        match &self.end.path {
            PathCoverage::EntireRoot(path) => {
                let root = path.root_pattern_location();
                let mut locations = vec![ChildLocation::new(
                    root.parent,
                    root.pattern_id,
                    path.role_root_child_index::<Start>(),
                )];
                locations.extend(path.start_path().path().iter().copied());
                Some(locations)
            },
            _ => None,
        }
    }

    /// Get the collected transitions (event kinds) in emission order.
    ///
    /// Useful for test assertions:
    /// ```ignore
    /// use context_trace::legacy::graph::visualization::Transition;
    /// let transitions = response.transitions();
    /// assert!(matches!(transitions[0], Transition::StartNode { .. }));
    /// ```
    pub fn transitions(&self) -> Vec<&Transition> {
        self.events.iter().map(|e| &e.transition).collect()
    }

    /// Get the transitions as owned clones.
    pub fn transitions_owned(&self) -> Vec<Transition> {
        self.events.iter().map(|e| e.transition.clone()).collect()
    }
}

impl TargetKey for Response {
    fn target_key(&self) -> DirectedKey {
        DirectedKey::up(self.end.root_parent(), *self.end.root_parent().width())
    }
}
