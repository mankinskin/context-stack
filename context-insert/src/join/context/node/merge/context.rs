use std::mem::offset_of;

use context_trace::Token;
use derive_more::{
    Deref,
    DerefMut,
};
use tracing::debug;

use crate::{
    RootMode,
    SplitVertexCache,
    interval::partition::{
        Infix,
        Postfix,
        Prefix,
    },
    join::context::node::{
        context::NodeJoinCtx,
        merge::{
            PartitionRange,
            RangeMap,
            partition::MergePartition,
        },
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
    /// Total number of partitions (num_offsets + 1)
    pub fn num_partitions(&self) -> usize {
        self.offsets.len() + 1
    }

    /// The target partition range based on merge mode.
    pub fn target_partition_range(&self) -> PartitionRange {
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

    /// Determine the partition type for a given partition range.
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

    /// Get the right offset index for a prefix partition.
    /// For partition `0..=p`, the right offset is at index `p`.
    pub fn prefix_right_offset(
        &self,
        partition_end: usize,
    ) -> usize {
        debug_assert!(
            partition_end < self.num_partitions(),
            "Prefix partition end {} must be < num_partitions {}",
            partition_end,
            self.num_partitions()
        );
        partition_end
    }

    /// Get the left offset index for a postfix partition.
    /// For partition `p..=num_offsets`, the left offset is at index `p - 1`.
    pub fn postfix_left_offset(
        &self,
        partition_start: usize,
    ) -> usize {
        debug_assert!(
            partition_start > 0,
            "Postfix partition start {} must be > 0",
            partition_start
        );
        partition_start - 1
    }

    /// Get both offset indices for an infix partition.
    /// For partition `a..=b`, left offset is at `a - 1`, right offset is at `b`.
    pub fn infix_offsets(
        &self,
        partition_start: usize,
        partition_end: usize,
    ) -> (usize, usize) {
        debug_assert!(
            partition_start > 0,
            "Infix partition start {} must be > 0",
            partition_start
        );
        debug_assert!(
            partition_end < self.num_partitions(),
            "Infix partition end {} must be < num_partitions {}",
            partition_end,
            self.num_partitions()
        );
        (partition_start - 1, partition_end)
    }

    pub fn merge_sub_partitions(
        &mut self,
        target_range_override: Option<PartitionRange>,
    ) -> (Token, RangeMap) {
        let num_partitions = self.num_partitions();
        // Use provided target range for Root mode, otherwise compute from MergeCtx
        let target_partition_range = target_range_override
            .unwrap_or_else(|| self.target_partition_range());

        debug!(
            node=?self.ctx.index,
            patterns=?self.ctx.patterns(),
            ?self.offsets,
            num_partitions,
            ?self.mode,
            ?target_partition_range,
            "merge_partitions_in_range: ENTERED"
        );

        let mut target_token: Option<Token> = None;
        let mut range_map = RangeMap::default();

        // Iterate over ALL partition ranges by length, then by starting position
        for len in 1..=num_partitions {
            debug!(
                "
    ==============================================================
    merging partitions of length {}
    ==============================================================",
                len
            );
            for start in 0..=(num_partitions - len) {
                let end = start + len - 1; // end is inclusive (partition index)
                let partition_range = PartitionRange::new(start..=end);

                debug!(
                    node=?self.ctx.index,
                    ?partition_range,
                    num_partitions,
                    ?self.mode,
                    "Merging partition range"
                );

                let partition_type = self.partition_type(&partition_range);
                debug!(?partition_type, "Detected partition type");

                // Use normal partition-based merging for Full mode and len=1 within target
                let (merged_token, delta) = match partition_type {
                    PartitionType::Full => {
                        debug!("Merging full existing token - skipping");
                        (self.ctx.index, None)
                    },
                    PartitionType::Prefix => {
                        debug!("Merge Prefix partition: ENTERED");
                        let ro_idx = self.prefix_right_offset(end);
                        let ro = self.offsets.pos_ctx_by_index(ro_idx);
                        Prefix::new(ro).merge_partition(
                            self,
                            &range_map,
                            &partition_range,
                        )
                    },
                    PartitionType::Postfix => {
                        debug!("Merge Postfix partition: ENTERED");
                        let lo_idx = self.postfix_left_offset(start);
                        let lo = self.offsets.pos_ctx_by_index(lo_idx);
                        Postfix::new(lo).merge_partition(
                            self,
                            &range_map,
                            &partition_range,
                        )
                    },
                    PartitionType::Infix => {
                        debug!("Merge Infix partition: ENTERED");
                        let (lo_idx, ro_idx) = self.infix_offsets(start, end);
                        let lo = self.offsets.pos_ctx_by_index(lo_idx);
                        let ro = self.offsets.pos_ctx_by_index(ro_idx);
                        Infix::new(lo, ro).merge_partition(
                            self,
                            &range_map,
                            &partition_range,
                        )
                    },
                };

                // Apply delta to offsets after prefix merges (patterns were modified)
                if let Some(ref deltas) = delta
                    && !deltas.is_empty()
                {
                    debug!(?deltas, "Applying deltas to offset cache");
                    self.offsets.apply_deltas(deltas);
                }

                if partition_range == target_partition_range {
                    debug!(
                        ?partition_range,
                        "merge_partitions_in_range: reached target partition range"
                    );
                    assert_eq!(target_token, None, "Target token already set");
                    target_token = Some(merged_token);
                }

                debug!(
                    ?partition_range,
                    ?merged_token,
                    "RangeMap INSERT: inserting token for range"
                );
                range_map.insert(partition_range.clone(), merged_token);
            }
        }

        // Extract target token from range_map
        let target_token = target_token
            .unwrap_or_else(|| panic!(
                "Target token not found in range_map for range {:?}. Available ranges: {:?}",
                target_partition_range,
                range_map.map.keys().collect::<Vec<_>>()
            ));
        (target_token, range_map)
    }
}
