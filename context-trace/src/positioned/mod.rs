//! Positioned composition model.
//!
//! Every retained vertex lists its maximal retained proper child occurrences
//! by atom offset (see the oracle plan in
//! `ngrams/agents/plans/20261118_PLAN_POSITIONED_COMPOSITION_ORACLE.md`).
//! Independent of the row model in `crate::legacy`; shares only the ID and
//! atom types ([`crate::VertexIndex`], [`crate::Token`], [`crate::GraphKind`]).

pub mod composition;
pub mod graph;
pub mod interchange;
pub mod location;

pub use self::{
    composition::{
        Composition,
        CompositionError,
        Occurrence,
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
