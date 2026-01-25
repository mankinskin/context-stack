use crate::{
    PatternSubDeltas,
    RangeRole,
    interval::partition::{
        Partition,
        ToPartition,
        info::{
            InfoPartition,
            PartitionInfo,
            range::role::{
                In,
                Post,
                Pre,
            },
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
    split::vertex::ToVertexSplits,
};
use context_trace::{
    Token,
    VertexSet,
};
use tracing::debug;

/// Context for merging a single partition.
///
/// This struct encapsulates the partition being merged along with the necessary
/// context for performing the merge operation. It is constructed from a `MergeCtx`
/// and a `PartitionRange`, automatically extracting the appropriate splits.
///
/// # Type Parameters
/// - `R`: The `RangeRole` (Pre, Post, or In) that determines the partition type
///
/// # Example
/// ```ignore
/// // Create a prefix partition context from MergeCtx
/// let ctx = MergePartitionCtx::<Pre<Join>>::from_prefix(
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
    pub partition_range: PartitionRange,
}

impl<'a, 'b, R: RangeRole<Mode = Join> + 'b> MergePartitionCtx<'a, 'b, R>
where
    R::Borders: JoinBorders<R>,
{
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
    pub fn add_sub_merges(
        &mut self,
        token: Token,
    ) {
        let existing_patterns = self
            .merge_ctx
            .ctx
            .trav
            .expect_vertex_data(token)
            .child_pattern_set();

        let sub_merges: Vec<_> = self
            .range_map
            .range_sub_merges(&self.partition_range)
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

/// Helper struct for building partitions from MergeCtx.
///
/// This struct implements `ToPartition<R>` for each partition role,
/// enabling a uniform interface for constructing `MergePartitionCtx`.
///
/// The builder holds only the data needed to compute the partition,
/// not the full MergeCtx, to avoid borrow conflicts.
#[derive(Clone)]
pub struct MergePartitionBuilder<'a> {
    offsets: &'a crate::SplitVertexCache,
    partition_range: PartitionRange,
    num_partitions: usize,
}

impl<'a> MergePartitionBuilder<'a> {
    pub fn new(
        merge_ctx: &'a MergeCtx<'_>,
        partition_range: PartitionRange,
    ) -> Self {
        Self {
            offsets: &merge_ctx.offsets,
            partition_range,
            num_partitions: merge_ctx.num_partitions(),
        }
    }
}

impl ToPartition<Pre<Join>> for MergePartitionBuilder<'_> {
    fn to_partition(self) -> Partition<Pre<Join>> {
        let partition_end = *self.partition_range.end();
        debug_assert!(
            partition_end < self.num_partitions,
            "Prefix partition end {} must be < num_partitions {}",
            partition_end,
            self.num_partitions
        );
        let ro = self
            .offsets
            .pos_ctx_by_index(partition_end)
            .to_vertex_splits();
        Partition { offsets: ro }
    }
}

impl ToPartition<Post<Join>> for MergePartitionBuilder<'_> {
    fn to_partition(self) -> Partition<Post<Join>> {
        let partition_start = *self.partition_range.start();
        debug_assert!(
            partition_start > 0,
            "Postfix partition start {} must be > 0",
            partition_start
        );
        let lo = self
            .offsets
            .pos_ctx_by_index(partition_start - 1) // offset left of partition
            .to_vertex_splits();
        Partition { offsets: lo }
    }
}

impl ToPartition<In<Join>> for MergePartitionBuilder<'_> {
    fn to_partition(self) -> Partition<In<Join>> {
        let partition_start = *self.partition_range.start();
        let partition_end = *self.partition_range.end();
        debug_assert!(
            partition_start > 0,
            "Infix partition start {} must be > 0",
            partition_start
        );
        debug_assert!(
            partition_end < self.num_partitions,
            "Infix partition end {} must be < num_partitions {}",
            partition_end,
            self.num_partitions
        );
        let lo = self
            .offsets
            .pos_ctx_by_index(partition_start - 1) // offset left of partition
            .to_vertex_splits();
        let ro = self
            .offsets
            .pos_ctx_by_index(partition_end)
            .to_vertex_splits();
        Partition { offsets: (lo, ro) }
    }
}

/// Uniform constructor for MergePartitionCtx using ToPartition trait.
impl<'a, 'b, R: RangeRole<Mode = Join> + 'b> MergePartitionCtx<'a, 'b, R>
where
    R::Borders: JoinBorders<R>,
    for<'c> MergePartitionBuilder<'c>: ToPartition<R>,
{
    /// Create a partition context from MergeCtx and partition range.
    ///
    /// The partition is built automatically based on the role type `R`
    /// using the `ToPartition` implementation for `MergePartitionBuilder`.
    ///
    /// # Type Parameters
    /// - `R`: The partition role (`Pre<Join>`, `Post<Join>`, or `In<Join>`)
    ///
    /// # Example
    /// ```ignore
    /// // Create a prefix partition context
    /// let ctx = MergePartitionCtx::<Pre<Join>>::from_merge_ctx(
    ///     merge_ctx,
    ///     &range_map,
    ///     partition_range,
    /// );
    /// ```
    pub fn from_merge_ctx(
        merge_ctx: &'a mut MergeCtx<'b>,
        range_map: &'a RangeMap,
        partition_range: PartitionRange,
    ) -> Self {
        // Build partition from an immutable borrow that ends before we store merge_ctx
        let partition = {
            let builder =
                MergePartitionBuilder::new(merge_ctx, partition_range.clone());
            builder.to_partition()
        };

        debug!(
            role = R::ROLE_STR,
            ?partition_range,
            "MergePartitionCtx::from_merge_ctx: ENTERED"
        );

        Self {
            partition,
            merge_ctx,
            range_map,
            partition_range,
        }
    }
}
