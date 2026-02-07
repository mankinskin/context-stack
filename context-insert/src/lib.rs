#![feature(slice_index_methods)]

pub mod insert;
pub mod interval;
pub(crate) mod join;
pub mod split;

#[cfg(test)]
pub(crate) mod tests;

// Auto-generated pub(crate) use statements
pub use crate::{
    insert::{
        ToInsertCtx,
        context::InsertCtx,
        result::InsertResult,
    },
    interval::{
        IntervalGraph,
        init::InitInterval,
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
