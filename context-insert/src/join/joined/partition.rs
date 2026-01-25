use crate::{
    interval::partition::{
        delta::PatternSubDeltas,
        info::{
            border::perfect::{
                BorderPerfect,
                SinglePerfect,
            },
            range::role::RangeRole,
        },
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
    },
};
use context_trace::*;
use std::borrow::Borrow;
use tracing::debug;

#[derive(Debug)]
pub struct JoinedPartition<R: RangeRole> {
    pub index: Token,
    pub perfect: R::Perfect,
    pub delta: PatternSubDeltas,
}

impl<'a, 'c, R: RangeRole<Mode = Join> + 'a> JoinedPartition<R>
where
    R::Borders: JoinBorders<R>,
{
    pub fn from_joined_patterns(
        pats: JoinedPatterns<R>,
        ctx: &'c mut NodeJoinCtx<'a>,
    ) -> Self {
        // collect infos about partition in each pattern
        let index = ctx.trav.insert_patterns(pats.patterns);
        
        // Compute actual delta based on replacement
        // When we replace a range of N elements with 1 token, delta = N - 1
        let mut delta = pats.delta;
        
        // Replace pattern if range is perfect in a pattern
        if let SinglePerfect(Some(pid)) = pats.perfect.complete() {
            let loc = ctx.index.to_pattern_location(pid);
            let replace_range = pats.range.as_ref().unwrap();
            
            // Compute the actual delta from the replacement
            // Replace range length - 1 (since we're replacing N tokens with 1)
            let replacement_delta = replace_range.len().saturating_sub(1);
            if replacement_delta > 0 {
                // Update the delta for this pattern
                delta.inner.insert(pid, replacement_delta);
            }
            
            ctx.trav.replace_in_pattern(loc, replace_range.clone(), index);
        }
        
        Self {
            index,
            perfect: pats.perfect,
            delta,
        }
    }
    pub fn from_partition_info(
        info: JoinPartitionInfo<R>,
        ctx: &'c mut NodeJoinCtx<'a>,
    ) -> Self {
        // collect infos about partition in each pattern
        let pats = JoinedPatterns::from_partition_info(info, ctx);
        debug!("JoinedPatterns: {:#?}", pats);
        Self::from_joined_patterns(pats, ctx)
    }
}

impl<K: RangeRole> Borrow<Token> for JoinedPartition<K> {
    fn borrow(&self) -> &Token {
        &self.index
    }
}

impl<K: RangeRole> Borrow<Token> for &JoinedPartition<K> {
    fn borrow(&self) -> &Token {
        &self.index
    }
}
