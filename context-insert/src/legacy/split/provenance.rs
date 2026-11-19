//! Operation-local provenance of split requirements.
//!
//! Recorded during split collection, read-only w.r.t. the graph. Records why
//! a split position was required and through which parent decompositions a
//! child was reached. Purely informational: it does not influence splitting
//! or joining.

use std::num::NonZeroUsize;

use context_trace::legacy::*;

use crate::legacy::split::cache::position::PosKey;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct IncidenceId(pub(crate) usize);

/// Why a split position exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RequirementOrigin {
    /// Boundary of the insertion target in the root.
    Target,
    /// Boundary propagated from a parent position, through `via` when the
    /// parent decomposition location is known.
    Propagated {
        from: PosKey,
        via: Option<IncidenceId>,
    },
    /// Offset added to complete inner ranges between existing positions.
    InnerAugmentation,
    /// Root wrapper boundary added for unperfect target splits.
    Wrapper,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Requirement {
    pub(crate) key: PosKey,
    pub(crate) origin: RequirementOrigin,
}

/// One parent decomposition location through which a child boundary was
/// requested: `child` is at `sub_index` of `pattern_id` in `parent`, starting
/// at parent atom offset `child_start`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct IncidencePath {
    pub(crate) parent: Token,
    pub(crate) pattern_id: PatternId,
    pub(crate) sub_index: usize,
    pub(crate) child_start: usize,
}

/// Structural incidence: a boundary requested at child-local `local_offset`
/// of `child`. Every parent decomposition requesting it is recorded as one
/// of its `paths`, so alternative decompositions do not duplicate it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Incidence {
    pub(crate) child: Token,
    pub(crate) local_offset: NonZeroUsize,
    pub(crate) paths: Vec<IncidencePath>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct SplitProvenance {
    pub(crate) requirements: Vec<Requirement>,
    pub(crate) incidences: Vec<Incidence>,
    index: HashMap<(Token, NonZeroUsize), IncidenceId>,
}

impl SplitProvenance {
    pub(crate) fn require(
        &mut self,
        key: PosKey,
        origin: RequirementOrigin,
    ) {
        let req = Requirement { key, origin };
        if !self.requirements.contains(&req) {
            self.requirements.push(req);
        }
    }

    pub(crate) fn record_incidence(
        &mut self,
        child: Token,
        local_offset: NonZeroUsize,
        path: IncidencePath,
    ) -> IncidenceId {
        let id = *self.index.entry((child, local_offset)).or_insert_with(|| {
            self.incidences.push(Incidence {
                child,
                local_offset,
                paths: Vec::new(),
            });
            IncidenceId(self.incidences.len() - 1)
        });
        let paths = &mut self.incidences[id.0].paths;
        if !paths.contains(&path) {
            paths.push(path);
        }
        id
    }

    #[allow(dead_code)]
    pub(crate) fn incidences_of(
        &self,
        child: Token,
    ) -> impl Iterator<Item = &Incidence> {
        self.incidences.iter().filter(move |i| i.child == child)
    }

    #[allow(dead_code)]
    pub(crate) fn requirements_at(
        &self,
        key: PosKey,
    ) -> impl Iterator<Item = &Requirement> {
        self.requirements.iter().filter(move |r| r.key == key)
    }
}
