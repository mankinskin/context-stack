use crate::{
    interval::partition::info::{
        border::{
            PartitionBorder,
            info::InfoBorder,
            perfect::BoolPerfect,
            trace::TraceBorders,
        },
        range::{
            InnerRangeInfo,
            ModeRangeInfo,
            PatternRangeInfo,
            children::RangeChildren,
            role::{
                BordersOf,
                ModeChildrenOf,
                ModePatternCtxOf,
                RangeRole,
            },
        },
    },
    join::{
        context::{
            node::context::NodeJoinCtx,
            pattern::borders::JoinBorders,
        },
        partition::{
            Join,
            info::inner_range::JoinInnerRangeInfo,
        },
        plan::JoinRef,
    },
};
use context_trace::*;

#[derive(Debug, Clone)]
pub(crate) struct JoinPatternInfo<R: RangeRole<Mode = Join>> {
    pub(crate) inner_range: Option<InnerRangeInfo<R>>,
    pub(crate) range: R::PatternRange,
    pub(crate) children: Option<ModeChildrenOf<R>>,
}

impl<R: RangeRole<Mode = Join>> JoinPatternInfo<R>
where
    R::Borders: JoinBorders<R>,
{
    pub(crate) fn join_pattern<'a: 'b, 'b: 'c, 'c>(
        self,
        ctx: &'c mut NodeJoinCtx<'a>,
        pattern_id: &PatternId,
    ) -> Vec<JoinRef>
    where
        R: 'a,
    {
        let inner = self
            .inner_range
            .map(|r| JoinInnerRangeInfo::new(r).plan_pattern_inner(ctx));
        match (inner, self.children) {
            (inner, Some(children)) => children.insert_inner(inner).unwrap(),
            (None, None) => ctx
                .overlay
                .pattern(pattern_id)
                .get(self.range.into())
                .expect("pattern range out of bounds")
                .to_vec(),
            (Some(_), None) => panic!("inner range without tokens"),
        }
    }
}

impl<R: RangeRole<Mode = Join>> ModeRangeInfo<R> for JoinPatternInfo<R>
where
    R::Borders: JoinBorders<R>,
{
    fn info_pattern_range(
        borders: BordersOf<R>,
        ctx: &ModePatternCtxOf<R>,
    ) -> Result<PatternRangeInfo<R>, JoinRef> {
        let perfect = borders.perfect();
        let range = borders.outer_range();
        let children = if perfect.all_perfect() {
            None
        } else {
            borders.get_child_splits(ctx)?
        };
        let inner = borders.inner_info(ctx);
        let pat = ctx.pattern.get(range.clone().into()).unwrap().to_vec();
        let pid = ctx.loc.pattern_id;
        match (pat.len(), children) {
            // Empty range: This can happen when a partition range has been absorbed
            // by a previous merge. Return the token at the boundary position.
            (0, Some(children)) => Err(children.to_token().unwrap()),
            (0, None) => {
                // No children info and empty range - use the token at the start position
                // The start position's sub_index points to the token that absorbed this range
                let sub_index = borders.start_sub_index();
                Err(ctx.pattern[sub_index])
            },
            (1, Some(children)) => Err(children.to_token().unwrap()),
            (1, None) => Err(pat[0]),
            (_, children) => Ok(PatternRangeInfo {
                pattern_id: pid,
                info: JoinPatternInfo {
                    inner_range: inner,
                    range,
                    children,
                },
            }),
        }
    }
}
