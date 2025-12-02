use context_trace::{
    path::{
        accessors::has_path::HasRolePath,
        RolePathUtils,
    },
    trace::cache::key::directed::{
        down::DownPosition,
        up::UpPosition,
    },
    RootedPath,
    *,
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
                PathCoverage::Range(s) => s.entry_pos,
                PathCoverage::Postfix(p) => p.entry_pos,
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
        root_pos: UpPosition,
        exit_pos: DownPosition,
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
                    exit_pos,
                    end_pos,
                }),
            (false, _, true, true) | (true, false, true, true) => {
                let path: IndexStartPath = path.into();
                tracing::trace!(
                    "Creating PostfixEnd with exit_pos={}",
                    usize::from(exit_pos.0)
                );
                PathCoverage::Postfix(PostfixEnd {
                    path,
                    entry_pos: root_pos,
                })
            },
            _ => {
                tracing::trace!(
                    "Creating RangeEnd: root_pos={}, end_pos={}",
                    usize::from(root_pos.0),
                    usize::from(end_pos)
                );
                PathCoverage::Range(RangeEnd {
                    path,
                    exit_pos,
                    entry_pos: root_pos,
                    target,
                    end_pos,
                })
            },
        }
    }
    #[allow(dead_code)]
    pub(crate) fn from_start_path<G: HasGraph>(
        mut path: IndexStartPath,
        root_pos: UpPosition,
        trav: &G,
    ) -> Self {
        path.role_path_mut().simplify(trav);
        match (
            path.is_at_border::<_, Start>(trav.graph()),
            path.raw_child_path().is_empty(),
        ) {
            (true, true) => PathCoverage::EntireRoot(path.into()),
            _ => PathCoverage::Postfix(PostfixEnd {
                path,
                entry_pos: root_pos,
            }),
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

    ///// Get the start path if it exists (safe version that returns Option)
    //pub(crate) fn try_start_path(&self) -> Option<&StartPath> {
    //    match self {
    //        PathCoverage::Range(p) => Some(p.path.start_path()),
    //        PathCoverage::Postfix(p) => Some(p.path.start_path()),
    //        PathCoverage::Prefix(_) | PathCoverage::EntireRoot(_) => None,
    //    }
    //}
}

impl std::fmt::Display for PathCoverage {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter,
    ) -> std::fmt::Result {
        let coverage = match self {
            PathCoverage::Range(_) => "Range",
            PathCoverage::Postfix(_) => "Postfix",
            PathCoverage::Prefix(_) => "Prefix",
            PathCoverage::EntireRoot(_) => "EntireRoot",
        };
        write!(f, "PathCoverage::{}(", coverage)?;
        write!(f, ")")
    }
}
