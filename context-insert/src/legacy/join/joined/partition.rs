use crate::legacy::{
    interval::partition::info::{
        border::perfect::{
            BorderPerfect,
            SinglePerfect,
        },
        range::role::RangeRole,
    },
    join::{
        context::{
            node::context::NodeJoinCtx,
            pattern::borders::JoinBorders,
        },
        joined::patterns::JoinedPatterns,
        partition::{
            Join,
            info::JoinPartitionInfo,
        },
        plan::{
            JoinRef,
            PatternOrigin,
            ProposedPattern,
        },
    },
};
use tracing::debug;

#[derive(Debug)]
pub(crate) struct JoinedPartition<R: RangeRole> {
    pub(crate) index: JoinRef,
    #[allow(dead_code)]
    pub(crate) perfect: R::Perfect,
}

impl<'a, 'c, R: RangeRole<Mode = Join> + 'a> JoinedPartition<R>
where
    R::Borders: JoinBorders<R>,
{
    /// Propose a JoinedPartition from JoinedPatterns.
    ///
    /// If `skip_pattern_replacement` is true, no in-place replacement is
    /// proposed even if there's a perfect match. This is used for edge
    /// partitions in ROOT mode where we only need the token.
    pub(crate) fn from_joined_patterns_with_options(
        pats: JoinedPatterns<R>,
        ctx: &'c mut NodeJoinCtx<'a>,
        skip_pattern_replacement: bool,
    ) -> Self {
        let host = ctx.index;
        let proposed = pats
            .patterns
            .into_iter()
            .map(|(source, refs)| ProposedPattern {
                refs,
                origin: PatternOrigin::Joined { source },
            })
            .collect();
        let index = ctx.plan.propose(host, proposed);

        if !skip_pattern_replacement
            && let SinglePerfect(Some(pid)) = pats.perfect.complete()
        {
            let replace_range = pats.range.unwrap().into();
            ctx.propose_replacement(pid, replace_range, index);
        }

        Self {
            index,
            perfect: pats.perfect,
        }
    }

    pub(crate) fn from_partition_info(
        info: JoinPartitionInfo<R>,
        ctx: &'c mut NodeJoinCtx<'a>,
    ) -> Self {
        Self::from_partition_info_with_options(info, ctx, false)
    }

    pub(crate) fn from_partition_info_with_options(
        info: JoinPartitionInfo<R>,
        ctx: &'c mut NodeJoinCtx<'a>,
        skip_pattern_replacement: bool,
    ) -> Self {
        let pats = JoinedPatterns::from_partition_info(info, ctx);
        debug!("JoinedPatterns: {:#?}", pats);
        Self::from_joined_patterns_with_options(
            pats,
            ctx,
            skip_pattern_replacement,
        )
    }
}
