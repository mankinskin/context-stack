//! Search response (legacy `state/response.rs`): the only hand-off to
//! insertion and to the next adjacent search. No visualization events.

use std::ops::Range;

#[cfg(any(test, feature = "test-api"))]
use context_trace::positioned::OccurrenceLocation;
use context_trace::{
    positioned::{
        QueryAnchor,
        TraceCache,
    },
    AtomPosition,
    CommittedIndex,
    Token,
    VertexId,
};

use crate::{
    cursor::{
        Matched,
        QueryCursor,
    },
    state::matched::{
        MatchResult,
        PathCoverage,
    },
};

/// Result of a positioned search: the trace cache of the final match and
/// the match itself.
///
/// Query positions are absolute in the query (see
/// [`state::matched`](crate::state::matched)): the match start in the root
/// (`span().start`) is the query position [`Self::query_start`].
///
/// [`Self::generation`] is the graph generation the search ran under (insert
/// plan rev 3, R6/D9); insertion rejects a response of another generation.
///
/// `I` is the index type of the searched store (staging plan §3.2); the
/// match itself is still committed until search becomes store-generic (S2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Response<I: VertexId = CommittedIndex> {
    pub cache: TraceCache<I>,
    pub end: MatchResult,
    pub(crate) generation: u64,
}

impl<I: VertexId> Response<I> {
    /// Graph generation under which the search ran
    /// ([`PositionedGraph::generation`](context_trace::positioned::PositionedGraph::generation)).
    pub fn generation(&self) -> u64 {
        self.generation
    }
    /// Root of the final match.
    pub fn root_token(&self) -> Token {
        self.end.root_parent()
    }
    pub fn path(&self) -> &PathCoverage {
        &self.end.path
    }
    /// The whole query matched.
    pub fn query_exhausted(&self) -> bool {
        self.end.query_exhausted()
    }
    pub fn is_entire_root(&self) -> bool {
        self.end.is_entire_root()
    }
    /// The checkpoint: the last matched query leaf. A continuation search
    /// ([`Searchable`](crate::search::searchable::Searchable)) starts right
    /// after it. (Legacy returned the advanced candidate; it is
    /// `end.cursor()` here.)
    pub fn query_cursor(&self) -> &QueryCursor<Matched> {
        self.end.checkpoint()
    }
    /// Query position of the candidate if present, else of the checkpoint.
    pub fn cursor_position(&self) -> AtomPosition {
        self.end.cursor().atom_position()
    }
    /// Query position of the match end.
    pub fn checkpoint_position(&self) -> AtomPosition {
        self.end.checkpoint().atom_position()
    }
    /// Query position of the match start (legacy assumed 0).
    pub fn query_start(&self) -> AtomPosition {
        self.end.path.query_start()
    }
    /// Matched atoms in absolute offsets of the root.
    pub fn span(&self) -> Range<AtomPosition> {
        self.end.path.span()
    }
    /// Root offsets ↔ query positions of the match.
    pub fn anchor(&self) -> QueryAnchor {
        self.end.path.anchor()
    }
    /// For an `EntireRoot` match, the start path top-down (root entry
    /// first); empty if only an atom first query token matched (the root is
    /// that atom). `None` for other coverages.
    #[cfg(any(test, feature = "test-api"))]
    pub fn entire_root_start_path(&self) -> Option<Vec<OccurrenceLocation>> {
        match &self.end.path {
            PathCoverage::EntireRoot(end) => Some(
                end.path
                    .iter()
                    .flat_map(|range| range.path.start.top_down().copied())
                    .collect(),
            ),
            _ => None,
        }
    }
    /// Panics unless the whole query matched a whole root.
    #[track_caller]
    pub fn expect_complete(
        self,
        msg: &str,
    ) -> Self {
        assert!(
            self.query_exhausted() && self.is_entire_root(),
            "{msg}: expected a complete EntireRoot match, got {:?}",
            self.end
        );
        self
    }
}
