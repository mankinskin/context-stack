use std::fmt::Debug;

use mode::{
    PatternInfoOf,
    Trace,
};
use role::{
    BordersOf,
    ModePatternCtxOf,
    RangeRole,
};

use context_trace::*;

use crate::{
    interval::partition::info::border::{
        info::InfoBorder,
        trace::TraceBorders,
    },
    split::vertex::pattern::HasPatternTraceCtx,
};

pub(crate) mod role;

pub(crate) mod children;
pub(crate) mod mode;
pub(crate) mod splits;

#[derive(Debug)]
pub(crate) struct PatternRangeInfo<R: RangeRole> {
    pub(crate) pattern_id: PatternId,
    pub(crate) info: PatternInfoOf<R>,
}

impl<R: RangeRole> From<PatternRangeInfo<R>> for (PatternId, PatternInfoOf<R>) {
    fn from(val: PatternRangeInfo<R>) -> Self {
        (val.pattern_id, val.info)
    }
}

pub(crate) trait ModeRangeInfo<R: RangeRole>: Debug {
    fn info_pattern_range(
        borders: BordersOf<R>,
        ctx: &ModePatternCtxOf<R>,
    ) -> Result<PatternRangeInfo<R>, Token>;
}

impl<R: RangeRole<Mode = Trace>> ModeRangeInfo<R> for TraceRangeInfo<R> {
    fn info_pattern_range(
        borders: BordersOf<R>,
        ctx: &ModePatternCtxOf<R>,
    ) -> Result<PatternRangeInfo<R>, Token> {
        let range = borders.outer_range();
        let inner = borders.inner_info(ctx);
        let pctx = ctx.pattern_trace_context();
        let pat = pctx.pattern.get(range.clone().into()).unwrap().to_vec();
        let pid = pctx.loc.pattern_id();
        if pat.len() != 1 {
            Ok(PatternRangeInfo {
                pattern_id: pid,
                info: TraceRangeInfo { inner_range: inner },
            })
        } else {
            Err(pat[0])
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct InnerRangeInfo<R: RangeRole> {
    pub(crate) range: R::PatternRange,
    pub(crate) offsets: R::Offsets,
}
impl<R: RangeRole> InnerRangeInfo<R> {
    pub(crate) fn delta(&self) -> usize {
        self.range.clone().into().len().saturating_sub(1)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TraceRangeInfo<R: RangeRole<Mode = Trace>> {
    pub(crate) inner_range: Option<InnerRangeInfo<R>>,
}
