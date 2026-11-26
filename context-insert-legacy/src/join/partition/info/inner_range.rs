use crate::{
    interval::partition::info::range::{
        InnerRangeInfo,
        role::RangeRole,
        splits::RangeOffsets,
    },
    join::{
        context::{
            node::context::NodeJoinCtx,
            pattern::borders::JoinBorders,
        },
        partition::{
            Join,
            JoinPartition,
        },
        plan::JoinRef,
    },
};
use derive_more::derive::{
    Deref,
    DerefMut,
    From,
    Into,
};
use derive_new::new;

#[derive(Debug, Clone, Deref, DerefMut, Into, From, new)]
pub(crate) struct JoinInnerRangeInfo<R: RangeRole<Mode = Join>>(
    InnerRangeInfo<R>,
)
where
    R::Borders: JoinBorders<R>;

impl<R: RangeRole<Mode = Join>> JoinInnerRangeInfo<R>
where
    R::Borders: JoinBorders<R>,
{
    pub(crate) fn plan_pattern_inner<'a: 'b, 'b: 'c, 'c>(
        &self,
        ctx: &'c mut NodeJoinCtx<'a>,
    ) -> JoinRef
    where
        Self: 'a,
    {
        let splits = self.offsets.as_splits_of(ctx.overlay.iter());
        match splits.join_partition(ctx) {
            Ok(inner) => inner.index,
            Err(p) => p,
        }
    }
}
