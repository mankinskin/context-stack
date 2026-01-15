use std::{
    fmt::Debug,
    hash::Hash,
};

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
pub struct Trace;

pub trait ModeInfo<R: RangeRole<Mode = Self>>:
    Debug + Clone + Copy + ModeChildren<R> + ModeCtx
{
    type PatternInfo: ModeRangeInfo<R>;
}

pub type PatternInfoOf<R> = <ModeOf<R> as ModeInfo<R>>::PatternInfo;

impl<R: RangeRole<Mode = Self>> ModeInfo<R> for Trace {
    type PatternInfo = TraceRangeInfo<R>;
}

/// Mode context trait for pattern operations.
/// 
/// Uses GATs (Generic Associated Types) to allow contexts to have lifetimes
/// while the trait itself doesn't require them upfront.
pub trait ModeCtx {
    type NodeCtx<'a>: AsNodeTraceCtx
        + GetPatternCtx<PatternCtx = Self::PatternResult>
        + GetPatternTraceCtx
    where
        Self: 'a;
    type PatternResult: HasPatternTraceCtx + Hash + Eq + Clone;
}

impl ModeCtx for Trace {
    type NodeCtx<'a> = NodeTraceCtx;
    type PatternResult = PatternTraceCtx;
}

pub trait ModeChildren<R: RangeRole> {
    type Result: Clone + Debug;
}

impl<R: RangeRole<Mode = Trace>> ModeChildren<R> for Trace {
    type Result = ();
}
pub trait PreVisitMode: ModeInfo<Pre<Self>> {}

impl PreVisitMode for Trace {}

pub trait PostVisitMode: ModeInfo<Post<Self>> {}

impl PostVisitMode for Trace {}

pub trait InVisitMode:
    ModeInfo<In<Self>> + PreVisitMode + PostVisitMode
{
}

impl InVisitMode for Trace {}
