use context_trace::{
    Token,
    VertexSet,
};
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
    split::vertex::ToVertexSplits,
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
        target_range: Option<PartitionRange>,
    ) -> (Token, RangeMap) {
        let num_partitions = self.num_partitions();
        // Use provided target range for Root mode, otherwise compute from MergeCtx
        let operating_range = self.operating_partition_range();

        debug!(
            node=?self.ctx.index,
            patterns=?self.ctx.patterns(),
            ?self.offsets,
            num_partitions,
            ?self.mode,
            ?target_range,
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

                let (merged_token, delta) = match partition_type {
                    PartitionType::Full => {
                        debug!(
                            "Merging full partition - adding sub-merge patterns"
                        );
                        let token = self.ctx.index;

                        // Add sub-merge patterns (all 2-way splits around each offset)
                        let existing_patterns = self
                            .ctx
                            .trav
                            .expect_vertex_data(token)
                            .child_pattern_set();

                        let sub_merges: Vec<_> = range_map
                            .range_sub_merges(&partition_range)
                            .into_iter()
                            .filter(|p| !existing_patterns.contains(p))
                            .collect();

                        if !sub_merges.is_empty() {
                            debug!(
                                num_sub_merges = sub_merges.len(),
                                ?sub_merges,
                                "Adding sub-merge patterns to full token"
                            );
                            for merge_pattern in sub_merges {
                                self.ctx.trav.add_pattern_with_update(
                                    token,
                                    merge_pattern,
                                );
                            }
                        }

                        (token, None)
                    },
                    PartitionType::Prefix => {
                        debug!("Merge Prefix partition: ENTERED");
                        let ro_idx = self.prefix_right_offset(end);
                        // Convert to owned VertexSplits before mutable borrow
                        let ro = self
                            .offsets
                            .pos_ctx_by_index(ro_idx)
                            .to_vertex_splits();
                        Prefix::new(ro).merge_partition(
                            self,
                            &range_map,
                            &partition_range,
                        )
                    },
                    PartitionType::Postfix => {
                        debug!("Merge Postfix partition: ENTERED");
                        let lo_idx = self.postfix_left_offset(start);
                        // Convert to owned VertexSplits before mutable borrow
                        let lo = self
                            .offsets
                            .pos_ctx_by_index(lo_idx)
                            .to_vertex_splits();
                        Postfix::new(lo).merge_partition(
                            self,
                            &range_map,
                            &partition_range,
                        )
                    },
                    PartitionType::Infix => {
                        debug!("Merge Infix partition: ENTERED");
                        let (lo_idx, ro_idx) = self.infix_offsets(start, end);
                        // Convert to owned VertexSplits before mutable borrow
                        let lo = self
                            .offsets
                            .pos_ctx_by_index(lo_idx)
                            .to_vertex_splits();
                        let ro = self
                            .offsets
                            .pos_ctx_by_index(ro_idx)
                            .to_vertex_splits();
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

                // Track target token if we've reached the target partition range
                if let Some(target_range) = target_range.as_ref()
                    && &partition_range == target_range
                {
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
        // For intermediary nodes (target_range is None), use the full node token
        let target_token = match target_range {
            Some(ref target_range) => target_token
                .unwrap_or_else(|| panic!(
                    "Target token not found in range_map for range {:?}. Available ranges: {:?}",
                    target_range,
                    range_map.map.keys().collect::<Vec<_>>()
                )),
            None => {
                // For intermediary nodes, the "target" is the full node
                self.ctx.index
            }
        };
        (target_token, range_map)
    }
}
