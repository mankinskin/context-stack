//! Positioned composition model.
//!
//! Every retained vertex lists its maximal retained proper child occurrences
//! by atom offset (see the oracle plan in
//! `ngrams/agents/plans/20261118_PLAN_POSITIONED_COMPOSITION_ORACLE.md`).
//! Independent of the row model in `crate::legacy`; shares only the ID and
//! atom types ([`crate::VertexIndex`], [`crate::Token`], [`crate::GraphKind`]).
//!
//! Stores are abstracted by traits (staging plan §3): [`GraphRead`] hands
//! out [`VertexRead`] vertex objects (`access.rs`), [`HasGraph`] /
//! [`HasGraphMut`] abstract how a store is reached (`has_graph.rs`), and the
//! committed store ([`PositionedGraph`], its lock guards and
//! [`PositionedGraphRef`]) implements them (`committed.rs`). A read edits a
//! stage ([`Stage`], [`StageHandle`], `stage.rs`); the overlay stage
//! ([`OverlayHandle`], [`OverlayView`], `overlay.rs`) keeps its vertices
//! next to the read-only committed graph until the commit.
//!
//! Read-only navigation (reader plan Phase 2) is split into separate APIs:
//! - content reading (debug and `test-api` builds):
//!   [`Navigate::read_interval`];
//! - local navigation for search, on vertex objects ([`Navigate`],
//!   `navigate.rs`): `start_at`, `end_at`, `prefix_child`,
//!   `postfix_child`, `prefixes`, `postfixes`, `parent_occurrences`,
//!   `advance_end_path`; atoms through [`GraphRead::atom`];
//! - search paths (role-generic [`RolePath`] with aliases [`StartPath`],
//!   [`EndPath`], [`QueryPath`]; [`RangePath`], `path/`) and trace states / trace cache ([`ParentState`],
//!   [`ChildState`], [`TraceCache`], `trace/`), search plan 2.2.

mod access;
mod check;
mod committed;
pub mod composition;
#[cfg(any(test, feature = "test-api"))]
mod convert;
#[cfg(any(test, feature = "test-api"))]
pub mod diff;
mod edit;
#[cfg(any(test, feature = "test-api"))]
pub mod fixtures;
pub mod graph;
mod has_graph;
pub mod interchange;
pub mod location;
mod navigate;
mod overlay;
pub mod path;
mod stage;
pub mod trace;

#[cfg(feature = "edit")]
#[doc(hidden)]
pub use self::edit::GraphEdit;
#[cfg(debug_assertions)]
pub use self::{
    graph::PositionedLabels,
    overlay::OverlayLabels,
};
#[cfg(any(debug_assertions, test, feature = "test-api"))]
pub use self::location::IntervalView;
#[cfg(any(test, feature = "test-api"))]
pub use self::diff::{
    GraphDiff,
    diff,
};
pub use self::{
    access::{
        CompositionRead,
        Generation,
        GraphRead,
        GraphWrite,
        NodeRef,
        VertexRead,
        VertexState,
    },
    committed::PositionedVertexRef,
    overlay::{
        Lifted,
        OverlayHandle,
        OverlayVertex,
        OverlayView,
        StagedVertex,
        StagingGraph,
    },
    stage::{
        CommitError,
        Stage,
        StageHandle,
        Translation,
    },
    has_graph::{
        GraphAtom,
        HasGraph,
        HasGraphMut,
        IndexOf,
        KindOf,
    },
    navigate::{
        Located,
        Navigate,
        string_repr,
    },
    composition::{
        Composition,
        CompositionError,
        Occurrence,
        maximal,
    },
    edit::{
        Publication,
        VertexEdit,
    },
    graph::{
        PositionedError,
        PositionedGraph,
        PositionedGraphRef,
        PositionedVertex,
        VertexNode,
    },
    location::OccurrenceLocation,
    path::{
        End,
        EndPath,
        HasRolePath,
        PathRole,
        PathRoot,
        QueryPath,
        QueryRoot,
        RangePath,
        RolePath,
        SpanCoverage,
        Start,
        StartPath,
    },
    trace::{
        QueryAnchor,
        TraceCtx,
        cache::{
            TraceCache,
            position::PositionCache,
            vertex::VertexCache,
        },
        child::state::{
            ChildState,
            RootChildState,
        },
        state::{
            IntoParentState,
            StateAdvance,
            parent::ParentState,
        },
        TraceDirection,
        traceable::{
            PostfixCommand,
            PrefixCommand,
            RangeCommand,
            TraceCommand,
            Traceable,
        },
    },
};

#[cfg(test)]
mod tests;
