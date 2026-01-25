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
    interval::partition::info::range::role::{
        In,
        Post,
        Pre,
    },
    join::{
        context::node::{
            context::NodeJoinCtx,
            merge::{
                MergePartitionCtx,
                PartitionRange,
                RangeMap,
            },
        },
        partition::Join,
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

    pub fn merge_sub_partitions(
        &mut self,
        target_range: Option<PartitionRange>,
    ) -> (Token, RangeMap) {
        let num_partitions = self.num_partitions();
        // Use provided target range for Root mode, otherwise compute from MergeCtx
        let _operating_range = self.operating_partition_range();

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
                        let token = self.ctx.index;

                        // For intermediary nodes (Full mode), add sub-merge patterns
                        // to represent all 2-way decompositions around each offset.
                        // For Root mode, we don't add sub-merge patterns because
                        // the root already has its patterns and we're just extracting
                        // a target partition.
                        if matches!(self.mode, MergeMode::Full) {
                            debug!(
                                "Merging full partition - adding sub-merge patterns (intermediary node)"
                            );

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
                        } else {
                            debug!(
                                "Merging full partition - skipping sub-merge patterns (root mode)"
                            );
                        }

                        (token, None)
                    },
                    PartitionType::Prefix =>
                        MergePartitionCtx::<Pre<Join>>::from_merge_ctx(
                            self,
                            &range_map,
                            partition_range.clone(),
                        )
                        .merge(),
                    PartitionType::Postfix =>
                        MergePartitionCtx::<Post<Join>>::from_merge_ctx(
                            self,
                            &range_map,
                            partition_range.clone(),
                        )
                        .merge(),
                    PartitionType::Infix =>
                        MergePartitionCtx::<In<Join>>::from_merge_ctx(
                            self,
                            &range_map,
                            partition_range.clone(),
                        )
                        .merge(),
                };

                // Apply delta to offsets AFTER the merged partition (patterns were modified)
                // Only offsets that come after the partition's end should have sub_indices adjusted
                if let Some(ref deltas) = delta
                    && !deltas.is_empty()
                {
                    debug!(
                        ?deltas,
                        partition_end = end,
                        "Applying deltas to offset cache (after index {})",
                        end
                    );
                    self.offsets.apply_deltas(deltas, end);
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

                // Compute splits for newly merged tokens (partitions with len > 0,
                // i.e., covering more than one partition index)
                // The range_map now has all sub-ranges we need to compute splits
                // Add them to shared splits so subsequent pattern operations can access them
                if !partition_range.is_empty() {
                    let computed_splits = range_map
                        .compute_splits_for_merged_token(
                            merged_token,
                            &partition_range,
                            self.ctx.splits,
                        );
                    debug!(
                        ?merged_token,
                        ?partition_range,
                        num_splits = computed_splits.len(),
                        "Computed splits for merged token"
                    );
                    for (key, split) in computed_splits {
                        self.add_split(key, split);
                    }
                }
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
