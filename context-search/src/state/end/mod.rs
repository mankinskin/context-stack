use context_trace::{
    path::{
        accessors::has_path::HasRolePath,
        RolePathUtils,
    },
    RootedPath,
    *,
};

use crate::{
    compare::parent::ParentCompareState,
    cursor::PatternCursor,
};

pub(crate) mod postfix;
pub(crate) mod prefix;
pub(crate) mod range;

use postfix::PostfixEnd;
use prefix::PrefixEnd;
use range::RangeEnd;

///// Represents the state of matching during search.
///// Distinguishes between "haven't found anything yet" (query state)
///// and "found something" (located in graph).
//#[derive(Clone, Debug, PartialEq, Eq)]
//pub(crate) enum MatchState {
//    /// Initial state: searching for the query pattern, no graph location yet
//    Query(PatternRangePath),
//    /// Found state: matched something and located it in the graph
//    Located(MatchResult),
//}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PathCoverage {
    Range(RangeEnd),
    Postfix(PostfixEnd),
    Prefix(PrefixEnd),
    EntireRoot(IndexRangePath),
}
impl GraphRoot for PathCoverage {
    fn root_parent(&self) -> Token {
        match self {
            PathCoverage::Range(p) => p.path.root_parent(),
            PathCoverage::Postfix(p) => p.path.root_parent(),
            PathCoverage::Prefix(p) => p.path.root_parent(),
            PathCoverage::EntireRoot(c) => c.root_parent(),
        }
    }
}

impl RootedPath for PathCoverage {
    type Root = IndexRoot;
    fn path_root(&self) -> Self::Root {
        match self {
            PathCoverage::Range(p) => p.path.path_root(),
            PathCoverage::Postfix(p) => p.path.path_root(),
            PathCoverage::Prefix(p) => p.path.path_root(),
            PathCoverage::EntireRoot(c) => c.path_root(),
        }
    }
}

impl RootKey for PathCoverage {
    fn root_key(&self) -> UpKey {
        UpKey::new(
            self.root_parent(),
            match self {
                PathCoverage::Range(s) => s.root_pos.into(),
                PathCoverage::Postfix(p) => p.root_pos.into(),
                PathCoverage::Prefix(_) => 0.into(),
                PathCoverage::EntireRoot(_) => 0.into(),
            },
        )
    }
}
impl PathCoverage {
    pub(crate) fn from_range_path<G: HasGraph>(
        mut path: IndexRangePath<
            ChildLocation,
            PositionAnnotated<ChildLocation>,
        >,
        root_pos: AtomPosition,
        target: DownKey,
        end_pos: AtomPosition,
        trav: &G,
    ) -> Self {
        if !path.start_path().is_empty() || !path.end_path().is_empty() {
            // Simplify both paths
            tracing::trace!(
                "from_range_path BEFORE simplify: start_path.len={}, end_path.len={}",
                path.start_path().len(),
                path.end_path().len()
            );
            path.start_path_mut().simplify(trav);
            path.end_path_mut().simplify(trav);
            tracing::trace!(
                "from_range_path AFTER simplify: start_path.len={}, end_path.len={}",
                path.start_path().len(),
                path.end_path().len()
            );
        }

        // Convert to plain path (strip position annotations) after simplification
        let path = path.into_plain();

        let start_at_border = path.is_at_border::<_, Start>(trav.graph());
        let start_path_empty = path.start_path().is_empty();
        let end_at_border = path.is_at_border::<_, End>(trav.graph());
        let end_path_empty = path.end_path().is_empty();

        tracing::trace!("from_range_path: start_at_border={}, start_path_empty={}, end_at_border={}, end_path_empty={}", 
            start_at_border, start_path_empty, end_at_border, end_path_empty);

        match (
            start_at_border,
            start_path_empty,
            end_at_border,
            end_path_empty,
        ) {
            (true, true, true, true) => PathCoverage::EntireRoot(path),
            (true, true, false, _) | (true, true, true, false) =>
                PathCoverage::Prefix(PrefixEnd {
                    path: path.into(),
                    target,
                    end_pos,
                }),
            (false, _, true, true) | (true, false, true, true) => {
                let path: IndexStartPath = path.into();
                tracing::trace!(
                    "Creating PostfixEnd with root_pos={}",
                    usize::from(root_pos)
                );
                PathCoverage::Postfix(PostfixEnd { path, root_pos })
            },
            _ => {
                tracing::trace!(
                    "Creating RangeEnd: root_pos={}, end_pos={}",
                    usize::from(root_pos),
                    usize::from(end_pos)
                );
                PathCoverage::Range(RangeEnd {
                    path,
                    root_pos,
                    target,
                    end_pos,
                })
            },
        }
    }
    pub(crate) fn from_start_path<G: HasGraph>(
        mut path: IndexStartPath,
        root_pos: AtomPosition,
        trav: &G,
    ) -> Self {
        path.role_path_mut().simplify(trav);
        match (
            path.is_at_border::<_, Start>(trav.graph()),
            path.raw_child_path().is_empty(),
        ) {
            (true, true) => PathCoverage::EntireRoot(path.into()),
            _ => PathCoverage::Postfix(PostfixEnd { path, root_pos }),
        }
    }

    /// Get the start path length for incremental tracing
    pub(crate) fn start_len(&self) -> usize {
        match self {
            PathCoverage::Range(p) => p.path.start_path().len(),
            PathCoverage::Postfix(p) => p.path.start_path().len(),
            PathCoverage::Prefix(_) | PathCoverage::EntireRoot(_) => 0,
        }
    }

    /// Get the start path if it exists (safe version that returns Option)
    pub(crate) fn try_start_path(&self) -> Option<&StartPath> {
        match self {
            PathCoverage::Range(p) => Some(p.path.start_path()),
            PathCoverage::Postfix(p) => Some(p.path.start_path()),
            PathCoverage::Prefix(_) | PathCoverage::EntireRoot(_) => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum EndReason {
    QueryExhausted,
    Mismatch,
    ChildExhausted,
}

impl std::fmt::Display for EndReason {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter,
    ) -> std::fmt::Result {
        match self {
            EndReason::QueryExhausted => write!(f, "QueryExhausted"),
            EndReason::Mismatch => write!(f, "Mismatch"),
            EndReason::ChildExhausted => write!(f, "ChildExhausted"),
        }
    }
}

// End types:
// - top down match-mismatch
// - top down match-query end
// - bottom up-no matching parents

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EndState {
    pub(crate) reason: EndReason,
    pub(crate) path: PathCoverage,
    pub(crate) cursor: PatternCursor,
}

impl Traceable for &EndState {
    fn trace<G: HasGraph>(
        self,
        ctx: &mut TraceCtx<G>,
    ) {
        match &self.path {
            PathCoverage::Range(p) => p.trace(ctx),
            PathCoverage::Prefix(p) => p.trace(ctx),
            PathCoverage::Postfix(p) => p.trace(ctx),
            _ => {},
        }
    }
}

impl EndState {
    //pub(crate) fn init_fold(cursor: PatternCursor) -> MatchState {
    //    // Initially, we have a query pattern that hasn't been located in the graph yet
    //    MatchState::Query(cursor.path)
    //}
    pub(crate) fn with_reason<G: HasGraph>(
        trav: G,
        reason: EndReason,
        parent: ParentCompareState,
    ) -> Self {
        let root_pos = *context_trace::path::accessors::path_accessor::StatePosition::root_pos(&parent.parent_state);
        Self {
            reason,
            path: PathCoverage::from_start_path(
                parent.parent_state.path,
                root_pos,
                &trav,
            ),
            cursor: parent.cursor,
        }
    }

    pub(crate) fn mismatch<G: HasGraph>(
        trav: G,
        parent: ParentCompareState,
    ) -> Self {
        Self::with_reason(trav, EndReason::Mismatch, parent)
    }
}

impl RootKey for EndState {
    fn root_key(&self) -> UpKey {
        UpKey::new(
            match &self.path {
                PathCoverage::Range(s) => s.path.root_parent(),
                PathCoverage::Postfix(p) => p.path.root_parent(),
                PathCoverage::Prefix(p) => p.path.root_parent(),
                PathCoverage::EntireRoot(c) => c.root_parent(),
            },
            match &self.path {
                PathCoverage::Range(s) => s.root_pos.into(),
                PathCoverage::Postfix(p) => p.root_pos.into(),
                PathCoverage::Prefix(_) => 0.into(),
                PathCoverage::EntireRoot(_) => 0.into(),
            },
        )
    }
}

impl_root! { GraphRoot for EndState, self =>
    match &self.path {
        PathCoverage::EntireRoot(c) => c.root_parent(),
        PathCoverage::Range(p) => p.path.root_parent(),
        PathCoverage::Postfix(p) => p.path.root_parent(),
        PathCoverage::Prefix(p) => p.path.root_parent(),
    }
}
