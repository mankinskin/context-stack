use crate::{
    PatternSubDeltas,
    RangeRole,
    interval::partition::{
        Partition,
        ToPartition,
        info::{
            InfoPartition,
            PartitionInfo,
        },
    },
    join::{
        context::{
            node::merge::{
                PartitionRange,
                RangeMap,
                context::MergeCtx,
            },
            pattern::borders::JoinBorders,
        },
        joined::partition::JoinedPartition,
        partition::{
            Join,
            info::JoinPartitionInfo,
        },
    },
};
use context_trace::{
    Token,
    VertexSet,
};
use tracing::debug;

/// Context for merging a single partition.
///
/// This struct encapsulates the partition being merged along with the necessary
/// context for performing the merge operation. It provides a cleaner API than
/// function-based dispatch by holding all partition-specific state together.
///
/// # Type Parameters
/// - `R`: The `RangeRole` (Pre, Post, or In) that determines the partition type
///
/// # Example
/// ```ignore
/// let ctx = MergePartitionCtx::new(
///     Prefix::new(splits),
///     merge_ctx,
///     &range_map,
///     &partition_range,
/// );
/// let (token, delta) = ctx.merge();
/// ```
pub struct MergePartitionCtx<'a, 'b, R: RangeRole<Mode = Join>>
where
    R::Borders: JoinBorders<R>,
{
    /// The partition being merged (contains offset splits)
    pub partition: Partition<R>,
    /// The merge context containing node context and offset cache
    pub merge_ctx: &'a mut MergeCtx<'b>,
    /// Map of already-merged partition ranges to their tokens
    pub range_map: &'a RangeMap,
    /// The range of partition indices being merged
    pub partition_range: &'a PartitionRange,
}

impl<'a, 'b, R: RangeRole<Mode = Join> + 'b> MergePartitionCtx<'a, 'b, R>
where
    R::Borders: JoinBorders<R>,
{
    /// Create a new merge partition context.
    ///
    /// # Arguments
    /// - `partition`: Something that can be converted to a `Partition<R>`
    /// - `merge_ctx`: The merge context with node context and offset cache
    /// - `range_map`: Map of already-merged ranges to tokens
    /// - `partition_range`: The range being merged
    pub fn new<P: ToPartition<R>>(
        partition: P,
        merge_ctx: &'a mut MergeCtx<'b>,
        range_map: &'a RangeMap,
        partition_range: &'a PartitionRange,
    ) -> Self {
        Self {
            partition: partition.to_partition(),
            merge_ctx,
            range_map,
            partition_range,
        }
    }

    /// Get partition info for all patterns.
    ///
    /// Returns `Ok(PartitionInfo)` if the partition needs to be created,
    /// or `Err(Token)` if a token already exists for this exact partition.
    pub fn info_partition(&self) -> Result<PartitionInfo<R>, Token> {
        self.partition.info_partition(&self.merge_ctx.ctx)
    }

    /// Create a JoinedPartition from partition info.
    ///
    /// This handles pattern joining, token creation/lookup, and delta computation.
    pub fn join_partition(
        &mut self,
        info: PartitionInfo<R>,
    ) -> JoinedPartition<R> {
        JoinedPartition::from_partition_info(
            JoinPartitionInfo::new(info),
            &mut self.merge_ctx.ctx,
        )
    }

    /// Add sub-merge patterns to a token if they don't already exist.
    ///
    /// Sub-merge patterns are alternative decompositions of the merged token
    /// that were discovered during the partition merge process.
    pub fn add_sub_merges(&mut self, token: Token) {
        let existing_patterns = self
            .merge_ctx
            .ctx
            .trav
            .expect_vertex_data(token)
            .child_pattern_set();

        let sub_merges: Vec<_> = self
            .range_map
            .range_sub_merges(self.partition_range)
            .into_iter()
            .filter(|p| !existing_patterns.contains(p))
            .collect();

        if !sub_merges.is_empty() {
            debug!(
                num_sub_merges = sub_merges.len(),
                "Adding sub-merge patterns"
            );
            for merge_pattern in sub_merges {
                self.merge_ctx
                    .ctx
                    .trav
                    .add_pattern_with_update(token, merge_pattern);
            }
        }
    }

    /// Merge the partition and return (token, delta).
    ///
    /// This is the main entry point that:
    /// 1. Gets partition info (or returns existing token)
    /// 2. Creates JoinedPartition with delta computation
    /// 3. Adds sub-merge patterns
    /// 4. Returns the token and any pattern deltas
    pub fn merge(mut self) -> (Token, Option<PatternSubDeltas>) {
        debug!(
            range_start = self.partition_range.start(),
            range_end = self.partition_range.end(),
            num_offsets = self.merge_ctx.offsets.len(),
            "MergePartitionCtx::merge: ENTERED"
        );

        match self.info_partition() {
            Ok(info) => {
                let joined = self.join_partition(info);

                debug!(
                    token = %joined.index,
                    ?joined.delta,
                    "JoinPartitionInfo succeeded with delta"
                );

                self.add_sub_merges(joined.index);

                let delta = if joined.delta.is_empty() {
                    None
                } else {
                    Some(joined.delta)
                };
                (joined.index, delta)
            },
            Err(existing) => {
                debug!(
                    ?existing,
                    range_start = self.partition_range.start(),
                    range_end = self.partition_range.end(),
                    "{}: Token already exists - using without modification",
                    R::ROLE_STR
                );
                (existing, None)
            },
        }
    }
}

/// Convenience function to merge a partition.
///
/// This creates a `MergePartitionCtx` and calls `merge()` in one step.
/// For more control over the merge process, use `MergePartitionCtx` directly.
pub fn merge_partition<'a, 'b, R, P>(
    partition: P,
    merge_ctx: &'a mut MergeCtx<'b>,
    range_map: &'a RangeMap,
    partition_range: &'a PartitionRange,
) -> (Token, Option<PatternSubDeltas>)
where
    R: RangeRole<Mode = Join> + 'b,
    R::Borders: JoinBorders<R>,
    P: ToPartition<R>,
{
    MergePartitionCtx::<R>::new(partition, merge_ctx, range_map, partition_range).merge()
}
