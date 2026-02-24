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

pub(crate) trait JoinBorders<R: RangeRole<Mode = Join>>:
    TraceBorders<R>
{
    fn get_child_splits(
        &self,
        ctx: &PatternJoinCtx,
    ) -> Option<ModeChildrenOf<R>>;
}

impl JoinBorders<Post<Join>> for BorderInfo {
    fn get_child_splits(
        &self,
        ctx: &PatternJoinCtx,
    ) -> Option<ChildrenOf<Post<Join>>> {
        self.inner_offset.map(|o| {
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
        })
    }
}

impl JoinBorders<Pre<Join>> for BorderInfo {
    fn get_child_splits(
        &self,
        ctx: &PatternJoinCtx,
    ) -> Option<ChildrenOf<Pre<Join>>> {
        self.inner_offset.map(|o| {
            ctx.splits
                .get(&PosKey::new(ctx.pattern[self.sub_index], o))
                .unwrap()
                .left
        })
    }
}

impl JoinBorders<In<Join>> for (BorderInfo, BorderInfo) {
    fn get_child_splits(
        &self,
        ctx: &PatternJoinCtx,
    ) -> Option<ChildrenOf<In<Join>>> {
        tracing::debug!(
            pattern_len = ctx.pattern.len(),
            left_sub_index = self.0.sub_index,
            right_sub_index = self.1.sub_index,
            "JoinBorders<In<Join>>::get_child_splits"
        );
        let (lc, rc) =
            (ctx.pattern[self.0.sub_index], ctx.pattern[self.1.sub_index]);
        match (self.0.inner_offset, self.1.inner_offset) {
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
        }
    }
}
