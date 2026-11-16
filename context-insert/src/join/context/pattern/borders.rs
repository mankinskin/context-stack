use crate::{
    interval::partition::info::{
        border::{
            BorderInfo,
            trace::TraceBorders,
        },
        range::{
            children::InfixChildren,
            role::{
                ChildrenOf,
                In,
                ModeChildrenOf,
                Post,
                Pre,
                RangeRole,
            },
        },
    },
    join::{
        context::pattern::PatternJoinCtx,
        partition::Join,
    },
    split::cache::position::PosKey,
};
use context_trace::Token;

pub(crate) trait JoinBorders<R: RangeRole<Mode = Join>>:
    TraceBorders<R>
{
    /// Return `Err(token)` when the entire interval is already materialized.
    fn get_child_splits(
        &self,
        ctx: &PatternJoinCtx,
    ) -> Result<Option<ModeChildrenOf<R>>, Token>;
}

impl JoinBorders<Post<Join>> for BorderInfo {
    fn get_child_splits(
        &self,
        ctx: &PatternJoinCtx,
    ) -> Result<Option<ChildrenOf<Post<Join>>>, Token> {
        Ok(self.inner_offset.map(|o| {
            let token = ctx.pattern[self.sub_index];
            let key = PosKey::new(token, o);
            tracing::debug!(
                ?token,
                ?key,
                sub_index = self.sub_index,
                pattern_len = ctx.pattern.len(),
                ?o,
                splits_keys = ?ctx.splits.keys().collect::<Vec<_>>(),
                "Post<Join>::get_child_splits lookup"
            );
            ctx.splits
                .get(&key)
                .unwrap_or_else(|| panic!(
                    "Split not found for {:?} in splits map. Pattern: {:?}, sub_index: {}, inner_offset: {:?}",
                    key, ctx.pattern, self.sub_index, o
                ))
                .right
        }))
    }
}

impl JoinBorders<Pre<Join>> for BorderInfo {
    fn get_child_splits(
        &self,
        ctx: &PatternJoinCtx,
    ) -> Result<Option<ChildrenOf<Pre<Join>>>, Token> {
        Ok(self.inner_offset.map(|o| {
            ctx.splits
                .get(&PosKey::new(ctx.pattern[self.sub_index], o))
                .unwrap()
                .left
        }))
    }
}

impl JoinBorders<In<Join>> for (BorderInfo, BorderInfo) {
    fn get_child_splits(
        &self,
        ctx: &PatternJoinCtx,
    ) -> Result<Option<ChildrenOf<In<Join>>>, Token> {
        tracing::debug!(
            pattern_len = ctx.pattern.len(),
            left_sub_index = self.0.sub_index,
            right_sub_index = self.1.sub_index,
            "JoinBorders<In<Join>>::get_child_splits"
        );
        let (lc, rc) =
            (ctx.pattern[self.0.sub_index], ctx.pattern[self.1.sub_index]);
        if self.0.sub_index == self.1.sub_index
            && let (Some(left), Some(right)) =
                (self.0.inner_offset, self.1.inner_offset)
        {
            let (start, end) = (left.get(), right.get());
            assert!(
                start < end && end < lc.width.0,
                "Invalid child interval for {lc:?}: {start}..{end}"
            );
            let token = *ctx
                .intervals
                .get(&(lc, start, end))
                .unwrap_or_else(|| {
                    panic!(
                        "Materialized child interval missing for {lc:?} at {start}..{end}"
                    )
                });
            return Err(token);
        }
        Ok(match (self.0.inner_offset, self.1.inner_offset) {
            (Some(l), Some(r)) => Some(InfixChildren::Both(
                ctx.splits.get(&PosKey::new(lc, l)).unwrap().right,
                ctx.splits.get(&PosKey::new(rc, r)).unwrap().left,
            )),
            (None, Some(r)) => Some(InfixChildren::Right(
                ctx.splits.get(&PosKey::new(rc, r)).unwrap().left,
            )),
            (Some(l), None) => Some(InfixChildren::Left(
                ctx.splits.get(&PosKey::new(lc, l)).unwrap().right,
            )),
            (None, None) => None,
        })
    }
}
