use derive_more::{
    Deref,
    DerefMut,
};

use crate::{
    RootMode,
    SplitVertexCache,
    join::context::node::{
        context::NodeJoinCtx,
        merge::PartitionRange,
    },
};

/// The type of a partition range based on its bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartitionType {
    Prefix,
    Postfix,
    Full,
    Infix,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum MergeMode {
    Full,
    Root(RootMode),
}
#[allow(dead_code)]
impl MergeMode {
    pub fn is_prefix(&self) -> bool {
        matches!(self, MergeMode::Root(RootMode::Prefix))
    }
    pub fn is_postfix(&self) -> bool {
        matches!(self, MergeMode::Root(RootMode::Postfix))
    }
    pub fn is_full(&self) -> bool {
        matches!(self, MergeMode::Full)
    }
    pub fn is_infix(&self) -> bool {
        matches!(self, MergeMode::Root(RootMode::Infix))
    }
    pub fn has_prefix(&self) -> bool {
        matches!(self, MergeMode::Full | MergeMode::Root(RootMode::Prefix))
    }
    pub fn has_postfix(&self) -> bool {
        matches!(self, MergeMode::Full | MergeMode::Root(RootMode::Postfix))
    }
}

#[derive(Debug, Deref, DerefMut)]
pub struct MergeCtx<'a> {
    #[deref]
    #[deref_mut]
    pub ctx: NodeJoinCtx<'a>,
    pub offsets: SplitVertexCache,
    pub mode: MergeMode,
}

impl<'a> MergeCtx<'a> {
    pub fn new(
        ctx: NodeJoinCtx<'a>,
        mode: MergeMode,
    ) -> Self {
        let offsets = ctx.vertex_cache().clone();
        Self {
            ctx,
            offsets, // clone the cache for processing and modification
            // in the future this may be a mutable reference into IntervalGraph.SplitCache, if we want to reuse the interval cache after insertion
            mode,
        }
    }

    /// Add a split to the shared splits map.
    /// This makes splits for merged tokens available to subsequent pattern operations.
    pub fn add_split(
        &mut self,
        key: crate::split::cache::position::PosKey,
        split: crate::split::Split,
    ) {
        self.ctx.splits.insert(key, split);
    }
}

impl<'a> MergeCtx<'a> {
    /// Total number of partitions (num_offsets + 1)
    pub fn num_partitions(&self) -> usize {
        self.offsets.len() + 1
    }

    /// The operating partition range based on merge mode.
    pub fn operating_partition_range(&self) -> PartitionRange {
        let num_offsets = self.offsets.len();
        PartitionRange::from(match self.mode {
            MergeMode::Full => 0..=num_offsets,
            MergeMode::Root(root_mode) => match root_mode {
                RootMode::Prefix => 0..=num_offsets.saturating_sub(1),
                RootMode::Postfix => 1..=num_offsets,
                RootMode::Infix => 1..=num_offsets.saturating_sub(1),
            },
        })
    }

    /// Determine the partition type for a given partition range relative to the full node.
    pub fn partition_type(
        &self,
        range: &PartitionRange,
    ) -> PartitionType {
        let starts_at_zero = *range.start() == 0;
        let ends_at_last = *range.end() == self.offsets.len();

        match (starts_at_zero, ends_at_last) {
            (true, true) => PartitionType::Full,
            (true, false) => PartitionType::Prefix,
            (false, true) => PartitionType::Postfix,
            (false, false) => PartitionType::Infix,
        }
    }

    /// Merge all sub-partitions and return the target token with range map.
    ///
    /// This uses `PartitionMergeIter` to iterate over all partitions in the
    /// operating range, merging them from smallest to largest.
    ///
    /// # Arguments
    /// - `target_range`: For root merges, the range containing the target token.
    ///   For intermediary merges, pass `None`.
    pub fn merge_sub_partitions(
        &mut self,
        target_range: Option<PartitionRange>,
    ) -> super::iter::MergeIterResult {
        use super::PartitionMergeIter;

        let mut iter = PartitionMergeIter::new(self, target_range);
        iter.merge_all();
        iter.finalize()
    }
}
