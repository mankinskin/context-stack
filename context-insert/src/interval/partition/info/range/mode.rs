use std::{
    fmt::Debug,
    hash::Hash,
};

use context_trace::HasToken;

use crate::{
    interval::partition::info::range::{
        ModeRangeInfo,
        TraceRangeInfo,
        role::{
            In,
            ModeOf,
            Post,
            Pre,
            RangeRole,
        },
    },
    split::vertex::{
        node::{
            AsNodeTraceCtx,
            NodeTraceCtx,
        },
        pattern::{
            GetPatternCtx,
            GetPatternTraceCtx,
            HasPatternTraceCtx,
            PatternTraceCtx,
        },
    },
};

#[derive(Debug, Clone, Copy)]
pub(crate) struct Trace;

pub(crate) trait ModeInfo<R: RangeRole<Mode = Self>>:
    Debug + Clone + Copy + ModeChildren<R> + ModeCtx
{
    type PatternInfo: ModeRangeInfo<R>;
}

pub(crate) type PatternInfoOf<R> = <ModeOf<R> as ModeInfo<R>>::PatternInfo;

impl<R: RangeRole<Mode = Self>> ModeInfo<R> for Trace {
    type PatternInfo = TraceRangeInfo<R>;
}

/// Mode context trait for pattern operations.
///
/// Uses GATs (Generic Associated Types) to allow contexts to have lifetimes
/// while the trait itself doesn't require them upfront.
pub(crate) trait ModeCtx {
    type NodeCtx<'a>: AsNodeTraceCtx
        + GetPatternCtx<PatternCtx = Self::PatternResult>
        + GetPatternTraceCtx
        + HasToken
    where
        Self: 'a;
    type PatternResult: HasPatternTraceCtx + Hash + Eq + Clone;
}

impl ModeCtx for Trace {
    type NodeCtx<'a> = NodeTraceCtx;
    type PatternResult = PatternTraceCtx;
}

pub(crate) trait ModeChildren<R: RangeRole> {
    type Result: Clone + Debug;
}

impl<R: RangeRole<Mode = Trace>> ModeChildren<R> for Trace {
    type Result = ();
}
pub(crate) trait PreVisitMode: ModeInfo<Pre<Self>> {}

impl PreVisitMode for Trace {}

pub(crate) trait PostVisitMode: ModeInfo<Post<Self>> {}

impl PostVisitMode for Trace {}

pub(crate) trait InVisitMode:
    ModeInfo<In<Self>> + PreVisitMode + PostVisitMode
{
}

impl InVisitMode for Trace {}
