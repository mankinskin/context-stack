use crate::{
    interval::partition::info::{
        PartitionInfo,
        range::role::{
            ModeNodeCtxOf,
            RangeRole,
        },
    },
    join::{
        context::pattern::borders::JoinBorders,
        joined::{
            partition::JoinedPartition,
        },
        partition::Join,
    },
};
use derive_more::derive::{
    Deref,
    DerefMut,
    From,
    Into,
};
use derive_new::new;

pub(crate) mod inner_range;
pub(crate) mod pattern_info;

#[derive(Debug, Deref, DerefMut, Into, From, new)]
pub(crate) struct JoinPartitionInfo<R: RangeRole<Mode = Join>>(PartitionInfo<R>)
where
    R::Borders: JoinBorders<R>;

impl<R: RangeRole<Mode = Join>> JoinPartitionInfo<R>
where
    R::Borders: JoinBorders<R>,
{
    pub(crate) fn into_joined_partition<'a>(
        self,
        ctx: &mut ModeNodeCtxOf<'a, R>,
    ) -> JoinedPartition<R>
    where
        R: 'a,
    {
        JoinedPartition::from_partition_info(self, ctx)
    }
}
