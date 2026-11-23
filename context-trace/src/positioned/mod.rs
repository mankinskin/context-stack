//! Positioned composition model.
//!
//! Every retained vertex lists its maximal retained proper child occurrences
//! by atom offset (see the oracle plan in
//! `ngrams/agents/plans/20261118_PLAN_POSITIONED_COMPOSITION_ORACLE.md`).
//! Independent of the row model in `crate::legacy`; shares only the ID and
//! atom types ([`crate::VertexIndex`], [`crate::Token`], [`crate::GraphKind`]).
//!
//! Read-only navigation (reader plan Phase 2) is split into separate APIs:
//! - content reading: [`PositionedGraph::read_interval`] (`read.rs`);
//! - occurrence enumeration in an interval:
//!   [`PositionedGraph::occurrences_in`] (`enumerate.rs`);
//! - local navigation for search: [`PositionedGraph::start_at`],
//!   [`PositionedGraph::end_at`], [`PositionedGraph::prefix_child`],
//!   [`PositionedGraph::parent_occurrences`],
//!   [`PositionedGraph::atom_token`], [`PositionedGraph::advance_end_path`]
//!   (`navigate.rs`);
//! - search paths (role-generic [`RolePath`] with aliases [`StartPath`],
//!   [`EndPath`], [`QueryPath`]; [`RangePath`], `path/`) and trace states / trace cache ([`ParentState`],
//!   [`ChildState`], [`TraceCache`], `trace/`), search plan 2.2;
//! - boundary search (prefixes/postfixes with occurrence paths) lives in
//!   `context_search::boundary`.

pub mod composition;
mod convert;
pub mod diff;
mod edit;
mod enumerate;
#[cfg(any(test, feature = "test-api"))]
pub mod fixtures;
pub mod graph;
pub mod interchange;
pub mod location;
mod navigate;
pub mod path;
mod read;
pub mod trace;

#[cfg(feature = "edit")]
#[doc(hidden)]
pub use self::edit::GraphEdit;
#[cfg(debug_assertions)]
pub use self::graph::PositionedLabels;
pub use self::{
    composition::{
        Composition,
        CompositionError,
        Occurrence,
        maximal,
    },
    diff::{
        GraphDiff,
        diff,
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
    location::{
        IntervalView,
        OccurrenceLocation,
        OccurrencePath,
    },
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
