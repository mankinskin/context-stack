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
//! - boundary search (prefixes/postfixes with occurrence paths) lives in
//!   `context_search::boundary`.

pub mod composition;
mod convert;
pub mod diff;
#[cfg(any(test, feature = "test-api"))]
pub mod fixtures;
pub mod graph;
pub mod interchange;
pub mod location;
mod read;

pub use self::{
    composition::{
        Composition,
        CompositionError,
        Occurrence,
    },
    diff::{
        GraphDiff,
        diff,
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
};

#[cfg(test)]
mod tests;
