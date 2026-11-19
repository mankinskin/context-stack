use crate::{
    interval::partition::info::{
        PartitionInfo,
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
        partition::{
            Join,
            info::{
                JoinPartitionInfo,
                pattern_info::JoinPatternInfo,
            },
        },
        plan::JoinRef,
    },
};
use context_trace::legacy::*;

/// Joined decompositions of a partition, one per original node pattern.
#[derive(Debug)]
pub(crate) struct JoinedPatterns<R: RangeRole> {
    pub(crate) patterns: Vec<(PatternId, Vec<JoinRef>)>,
    pub(crate) perfect: R::Perfect,
    pub(crate) range: Option<R::PatternRange>,
}

impl<'a, R: RangeRole<Mode = Join> + 'a> JoinedPatterns<R>
where
    R::Borders: JoinBorders<R>,
{
    pub(crate) fn from_partition_info<'b>(
        info: JoinPartitionInfo<R>,
        ctx: &'b mut NodeJoinCtx<'a>,
    ) -> Self {
        // index inner range
        // cases:
        // - (token, inner, token)
        // - (token, inner),
        // - (inner, token),
        // - (token, token),
        // - token: not possible, handled earlier
        let range = if let SinglePerfect(Some(pid)) = info.perfect.complete() {
            Some(info.patterns[&pid].range.clone())
        } else {
            None
        };
        let perfect = info.perfect.clone();
        let patterns = PartitionInfo::from(info)
            .patterns
            .into_iter()
            .map(|(pid, pinfo): (PatternId, JoinPatternInfo<_>)| {
                (pid, pinfo.join_pattern(ctx, &pid))
            })
            .collect();
        Self {
            patterns,
            perfect,
            range,
        }
    }
}
