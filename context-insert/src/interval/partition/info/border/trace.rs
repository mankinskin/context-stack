use crate::{
    interval::partition::info::{
        border::info::InfoBorder,
        range::{
            InnerRangeInfo,
            role::{
                ModePatternCtxOf,
                RangeRole,
            },
        },
    },
    split::vertex::pattern::HasPatternTraceCtx,
};

pub(crate) trait TraceBorders<R: RangeRole>: InfoBorder<R> {
    fn inner_info(
        &self,
        ctx: &ModePatternCtxOf<R>,
    ) -> Option<InnerRangeInfo<R>>;
}

impl<R: RangeRole> TraceBorders<R> for R::Borders {
    fn inner_info(
        &self,
        ctx: &ModePatternCtxOf<R>,
    ) -> Option<InnerRangeInfo<R>> {
        let pctx = ctx.pattern_trace_context();
        self.inner_range_offsets(&pctx.pattern).map(move |offsets| {
            InnerRangeInfo {
                range: self.inner_range(),
                offsets,
            }
        })
    }
}
