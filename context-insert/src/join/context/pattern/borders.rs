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
        plan::{
            JoinRef,
            RefPosKey,
        },
    },
};
use context_trace::Wide;

pub(crate) trait JoinBorders<R: RangeRole<Mode = Join>>:
    TraceBorders<R>
{
    /// Return `Err(token)` when the entire interval is already materialized.
    fn get_child_splits(
        &self,
        ctx: &PatternJoinCtx,
    ) -> Result<Option<ModeChildrenOf<R>>, JoinRef>;
}

fn expect_split(
    ctx: &PatternJoinCtx,
    key: RefPosKey,
) -> &crate::split::Split<JoinRef> {
    ctx.splits.get(&key).unwrap_or_else(|| {
        panic!(
            "Split not found for {:?} in splits map. Pattern: {:?}",
            key, ctx.pattern
        )
    })
}

impl JoinBorders<Post<Join>> for BorderInfo {
    fn get_child_splits(
        &self,
        ctx: &PatternJoinCtx,
    ) -> Result<Option<ChildrenOf<Post<Join>>>, JoinRef> {
        Ok(self.inner_offset.map(|o| {
            let child = ctx.pattern[self.sub_index];
            tracing::debug!(
                ?child,
                sub_index = self.sub_index,
                pattern_len = ctx.pattern.len(),
                ?o,
                "Post<Join>::get_child_splits lookup"
            );
            expect_split(ctx, RefPosKey::new(child, o)).right
        }))
    }
}

impl JoinBorders<Pre<Join>> for BorderInfo {
    fn get_child_splits(
        &self,
        ctx: &PatternJoinCtx,
    ) -> Result<Option<ChildrenOf<Pre<Join>>>, JoinRef> {
        Ok(self.inner_offset.map(|o| {
            expect_split(ctx, RefPosKey::new(ctx.pattern[self.sub_index], o))
                .left
        }))
    }
}

impl JoinBorders<In<Join>> for (BorderInfo, BorderInfo) {
    fn get_child_splits(
        &self,
        ctx: &PatternJoinCtx,
    ) -> Result<Option<ChildrenOf<In<Join>>>, JoinRef> {
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
                start < end && end < *lc.width(),
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
                expect_split(ctx, RefPosKey::new(lc, l)).right,
                expect_split(ctx, RefPosKey::new(rc, r)).left,
            )),
            (None, Some(r)) => Some(InfixChildren::Right(
                expect_split(ctx, RefPosKey::new(rc, r)).left,
            )),
            (Some(l), None) => Some(InfixChildren::Left(
                expect_split(ctx, RefPosKey::new(lc, l)).right,
            )),
            (None, None) => None,
        })
    }
}
