//! Row-model insertion (legacy).
//!
//! Extracted from `context_insert::legacy`. Frozen: no new code here, only fixes needed
//! to keep the legacy tests green. New code lives in `context-insert` and must
//! not depend on this crate.

pub mod insert;
pub mod interval;
pub(crate) mod join;
pub mod overlap;
pub mod split;
pub(crate) mod visualization;

#[cfg(test)]
pub(crate) mod tests;

// Auto-generated pub(crate) use statements
pub use crate::{
    insert::{
        InsertOutcome,
        Materialization,
        Publication,
        RangeReplacement,
        ToInsertCtx,
        context::InsertCtx,
        result::InsertResult,
    },
    interval::{
        IntervalGraph,
        init::InitInterval,
    },
    overlap::{
        ClosurePolicy,
        Overlap,
    },
    split::{
        TraceBack,
        TraceFront,
        TraceSide,
        cache::{
            SplitCache,
            position::{
                PosKey,
                SplitPositionCache,
            },
            vertex::SplitVertexCache,
        },
        trace::states::SplitStates,
        vertex::output::RootMode,
    },
};

pub(crate) use crate::{
    interval::partition::{
        delta::PatternSubDeltas,
        info::range::{
            mode::InVisitMode,
            role::{
                BooleanPerfectOf,
                In,
                OffsetsOf,
                Post,
                Pre,
                RangeRole,
            },
        },
    },
    split::{
        TokenTracePos,
        cleaned_position_splits,
        trace::SplitTraceCtx,
        vertex::{
            ToVertexSplitPos,
            TokenTracePositions,
            VertexSplits,
            output::{
                CompleteLocations,
                InnerNode,
            },
            position::{
                HasInnerOffset,
                Offset,
                SubSplitLocation,
            },
        },
    },
};
