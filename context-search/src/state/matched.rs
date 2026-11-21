//! Match results (legacy `state/matched` and `state/end`): the matched
//! range classified by coverage, and the checkpointed query cursor.
//!
//! **Query positions.** All positions are query positions (Decision 4 a);
//! within a search they are absolute in the query, so chained searches
//! share one coordinate system. A match maps root offsets to query
//! positions through its anchor: `span.start` ↔ [`PathCoverage::query_start`]
//! (the query position of the match start; legacy assumed 0). The trace
//! cache follows the conventions of `context_trace::positioned::trace`:
//! up keys carry the query position at the token end; down keys the query
//! position at the token start, clamped to the match start (with
//! overlapping occurrences an end-path token may start before the match;
//! its true offset is the sum of the edge labels plus `span.start`).

use std::ops::Range;

use context_trace::{
    positioned::{
        ChildState,
        End,
        EndPath,
        PositionedGraph,
        PostfixCommand,
        PrefixCommand,
        QueryAnchor,
        RangeCommand,
        RangePath,
        SpanCoverage,
        StartPath,
        TraceCtx,
        Traceable,
    },
    AtomPosition,
    DownKey,
    DownPosition,
    GraphKind,
    Token,
    UpPosition,
};

use crate::cursor::{
    AtCheckpoint,
    Checkpointed,
    HasCandidate,
    Matched,
    QueryCursor,
};

/// The query cursor of a result: the checkpoint (last matched query leaf)
/// and, after an inconclusive root, the candidate advanced past it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CheckpointedCursor {
    AtCheckpoint(Checkpointed<QueryCursor<Matched>, AtCheckpoint>),
    HasCandidate(Checkpointed<QueryCursor<Matched>, HasCandidate>),
}

impl CheckpointedCursor {
    /// The last matched query leaf; `atom_position` is the match end.
    pub fn checkpoint(&self) -> &QueryCursor<Matched> {
        match self {
            Self::AtCheckpoint(c) => c.checkpoint(),
            Self::HasCandidate(c) => c.checkpoint(),
        }
    }
    /// The candidate if present, else the checkpoint (legacy `cursor()`).
    pub fn cursor(&self) -> &QueryCursor<Matched> {
        match self {
            Self::AtCheckpoint(c) => c.checkpoint(),
            Self::HasCandidate(c) => c.candidate(),
        }
    }
}

/// A matched range in its root with its query positions (legacy
/// `RangeEnd`/`PrefixEnd`/`PostfixEnd` fields).
///
/// - `path`: canonical paths (2.4) and `span` in absolute root offsets;
/// - `entry_pos`: query position at the end of the root entry child;
/// - `exit_pos`: down position of the root exit child;
/// - `target`: down key of the end leaf;
/// - `end_pos`: query position at the match end.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchedRange {
    pub path: RangePath,
    pub entry_pos: UpPosition,
    pub exit_pos: DownPosition,
    pub target: DownKey,
    pub end_pos: AtomPosition,
}

impl MatchedRange {
    /// Query position of the match start (`span.start`).
    pub fn query_start(&self) -> AtomPosition {
        AtomPosition::from(*self.end_pos - (*self.path.span.end - *self.path.span.start))
    }
    /// Root offsets ↔ query positions of this match.
    pub fn anchor(&self) -> QueryAnchor {
        QueryAnchor {
            root: self.path.span.start,
            query: self.query_start(),
        }
    }
}

/// A whole root matched. `path` is `None` iff nothing beyond the first
/// query token matched and that token is an atom (legacy `EntireRoot` of
/// `q0`); an unmatched composite `q0` carries its canonical paths.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntireRootEnd {
    pub root: Token,
    pub query_start: AtomPosition,
    pub path: Option<MatchedRange>,
}

/// How the match covers its root (legacy `PathCoverage`), classified by
/// [`SpanCoverage`] from the span.
#[derive(Clone, Debug, PartialEq, Eq, strum::AsRefStr)]
pub enum PathCoverage {
    Range(MatchedRange),
    Postfix(MatchedRange),
    Prefix(MatchedRange),
    EntireRoot(EntireRootEnd),
}

impl PathCoverage {
    /// Classify the matched child state. The end path is replaced by the
    /// canonical one (2.4: descent to the largest token ending at the span
    /// end), so it depends only on root and span, not on the leaves the
    /// comparison went through; the start path is the climbed chain.
    pub(crate) fn from_child_state<G: GraphKind>(
        graph: &PositionedGraph<G>,
        mut state: ChildState,
        end_pos: AtomPosition,
    ) -> Self {
        let anchor = state.anchor();
        state.path.canonicalize::<End, G>(graph);
        state.exit_pos = anchor.down(state.path.end.root_child_offset());
        let target = DownKey::new(
            state.path.end.leaf(),
            anchor.down(state.path.end.leaf_offset()),
        );
        let ChildState {
            entry_pos,
            exit_pos,
            path,
            ..
        } = state;
        let range = MatchedRange {
            path,
            entry_pos,
            exit_pos,
            target,
            end_pos,
        };
        debug_assert_eq!(range.anchor(), anchor, "query positions agree");
        match range.path.coverage() {
            SpanCoverage::EntireRoot => Self::EntireRoot(EntireRootEnd {
                root: range.path.root(),
                query_start: range.query_start(),
                path: Some(range),
            }),
            SpanCoverage::Prefix => Self::Prefix(range),
            SpanCoverage::Postfix => Self::Postfix(range),
            SpanCoverage::Range => Self::Range(range),
        }
    }
    /// The first query token at `query_start` matched alone (legacy
    /// `EntireRoot` of `q0`). An atom has no paths. A composite token gets
    /// the canonical paths of the span `[0, width)` in itself (2.4: the
    /// prefix child, i.e. `start_at(token, 0)`, and `end_at(token, width)`),
    /// as the equivalent atom query matching the whole token returns them;
    /// both are functions of root and span, no content is read.
    pub(crate) fn entire_token<G: GraphKind>(
        graph: &PositionedGraph<G>,
        token: Token,
        query_start: AtomPosition,
    ) -> Self {
        let Some(prefix) = graph.prefix_child(token) else {
            return Self::EntireRoot(EntireRootEnd {
                root: token,
                query_start,
                path: None,
            });
        };
        let width = AtomPosition::from(token.width);
        let start = StartPath::new(token, prefix.offset, prefix.child);
        let end = EndPath::end_at(graph, token, width).expect("composite token has an end");
        let entry_pos = UpPosition(query_start + prefix.child.width.0);
        let state = ChildState {
            entry_pos,
            exit_pos: DownPosition(query_start),
            start_pos: query_start,
            path: RangePath::new(AtomPosition::default()..width, start, end),
        };
        let path = Self::from_child_state(graph, state, query_start + token.width.0);
        debug_assert!(matches!(path, Self::EntireRoot(_)));
        path
    }
    pub fn root(&self) -> Token {
        match self {
            Self::EntireRoot(end) => end.root,
            Self::Range(r) | Self::Postfix(r) | Self::Prefix(r) => r.path.root(),
        }
    }
    /// The matched range with paths; `None` for an unmatched first token.
    pub fn range(&self) -> Option<&MatchedRange> {
        match self {
            Self::EntireRoot(end) => end.path.as_ref(),
            Self::Range(r) | Self::Postfix(r) | Self::Prefix(r) => Some(r),
        }
    }
    /// Matched atoms in absolute offsets of the root.
    pub fn span(&self) -> Range<AtomPosition> {
        match self.range() {
            Some(range) => range.path.span.clone(),
            None => AtomPosition::default()..AtomPosition::from(self.root().width),
        }
    }
    /// Query position of the match start.
    pub fn query_start(&self) -> AtomPosition {
        match self {
            Self::EntireRoot(end) => end.query_start,
            Self::Range(r) | Self::Postfix(r) | Self::Prefix(r) => r.query_start(),
        }
    }
    /// Root offsets ↔ query positions of this match.
    pub fn anchor(&self) -> QueryAnchor {
        QueryAnchor {
            root: self.span().start,
            query: self.query_start(),
        }
    }
    pub fn coverage(&self) -> SpanCoverage {
        match self {
            Self::Range(_) => SpanCoverage::Range,
            Self::Postfix(_) => SpanCoverage::Postfix,
            Self::Prefix(_) => SpanCoverage::Prefix,
            Self::EntireRoot(_) => SpanCoverage::EntireRoot,
        }
    }
    pub fn start_path(&self) -> Option<&StartPath> {
        self.range().map(|r| &r.path.start)
    }
    pub fn end_path(&self) -> Option<&EndPath> {
        self.range().map(|r| &r.path.end)
    }
}

impl Traceable for &PathCoverage {
    /// As legacy: up edges along the start path for `Postfix`, down edges
    /// along the end path for `Prefix`, both for `Range`, nothing for
    /// `EntireRoot`.
    fn trace<G: GraphKind>(
        self,
        ctx: &mut TraceCtx<'_, G>,
    ) {
        match self {
            PathCoverage::Range(r) => RangeCommand {
                path: r.path.clone(),
                entry_pos: r.entry_pos,
            }
            .trace(ctx),
            PathCoverage::Postfix(r) => PostfixCommand {
                path: r.path.start.clone(),
                entry_pos: r.entry_pos,
            }
            .trace(ctx),
            PathCoverage::Prefix(r) => PrefixCommand {
                path: r.path.end.clone(),
                exit_pos: r.exit_pos,
            }
            .trace(ctx),
            PathCoverage::EntireRoot(_) => {},
        }
    }
}

/// A match in one root: the classified path and the query cursor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchResult {
    pub path: PathCoverage,
    pub cursor: CheckpointedCursor,
}

impl MatchResult {
    pub(crate) fn new(
        path: PathCoverage,
        cursor: CheckpointedCursor,
    ) -> Self {
        let result = Self { path, cursor };
        debug_assert_eq!(
            *result.cursor.checkpoint().atom_position(),
            {
                let span = result.path.span();
                *result.path.query_start() + (*span.end - *span.start)
            },
            "the checkpoint is at the match end"
        );
        result
    }
    pub fn path(&self) -> &PathCoverage {
        &self.path
    }
    pub fn root_parent(&self) -> Token {
        self.path.root()
    }
    /// The candidate if present, else the checkpoint (legacy `cursor()`).
    pub fn cursor(&self) -> &QueryCursor<Matched> {
        self.cursor.cursor()
    }
    pub fn checkpoint(&self) -> &QueryCursor<Matched> {
        self.cursor.checkpoint()
    }
    /// The whole query matched.
    pub fn query_exhausted(&self) -> bool {
        self.cursor.checkpoint().path().at_end()
    }
    pub fn is_entire_root(&self) -> bool {
        matches!(self.path, PathCoverage::EntireRoot(_))
    }
}

impl Traceable for &MatchResult {
    fn trace<G: GraphKind>(
        self,
        ctx: &mut TraceCtx<'_, G>,
    ) {
        (&self.path).trace(ctx)
    }
}
