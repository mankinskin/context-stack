use crate::{
    RangeRole,
    interval::partition::{
        Partition,
        ToPartition,
        info::{
            InfoPartition,
            PartitionInfo,
            border::perfect::BorderPerfect,
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
        plan::{
            JoinRef,
            NodeOverlay,
            PatternOrigin,
            ProposedPattern,
            SubMergeKind,
        },
    },
    split::{
        trace_back_pos,
        vertex::{
            ToVertexSplits,
            VertexSplits,
        },
    },
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
/// let token = ctx.merge();
/// ```
pub(crate) struct MergePartitionCtx<'a, 'b, R: RangeRole<Mode = Join>>
where
    R::Borders: JoinBorders<R>,
{
    /// The partition being merged (contains offset splits)
    pub(crate) partition: Partition<R>,
    /// The merge context containing node context and offset cache
    pub(crate) merge_ctx: &'a mut MergeCtx<'b>,
    /// Map of already-merged partition ranges to their tokens
    pub(crate) range_map: &'a RangeMap,
    /// The range of partition indices being merged
    pub(crate) partition_range: PartitionRange,
}

/// Result of a partition merge operation.
pub(crate) struct MergeResult {
    /// The merged token
    pub(crate) token: JoinRef,
    /// Whether an in-place replacement was proposed (perfect pattern match)
    pub(crate) had_pattern_replacement: bool,
}

impl<'a, 'b, R: RangeRole<Mode = Join> + 'b> MergePartitionCtx<'a, 'b, R>
where
    R::Borders: JoinBorders<R>,
{
    /// Get partition info for all patterns.
    ///
    /// Returns `Ok(PartitionInfo)` if the partition needs to be created,
    /// or `Err(JoinRef)` if a token already exists for this exact partition.
    pub(crate) fn info_partition(&self) -> Result<PartitionInfo<R>, JoinRef> {
        self.partition.info_partition(&self.merge_ctx.ctx)
    }

    /// Create a JoinedPartition from partition info.
    ///
    /// This handles pattern joining and candidate proposal.
    /// If `skip_pattern_replacement` is true, no replacement is proposed.
    pub(crate) fn join_partition_with_options(
        &mut self,
        info: PartitionInfo<R>,
        skip_pattern_replacement: bool,
    ) -> JoinedPartition<R> {
        JoinedPartition::from_partition_info_with_options(
            JoinPartitionInfo::new(info),
            &mut self.merge_ctx.ctx,
            skip_pattern_replacement,
        )
    }

    /// Create a JoinedPartition from partition info (allows pattern replacement).
    #[allow(dead_code)]
    pub(crate) fn join_partition(
        &mut self,
        info: PartitionInfo<R>,
    ) -> JoinedPartition<R> {
        self.join_partition_with_options(info, false)
    }

    /// Propose sub-merge patterns for a token if it doesn't have them yet.
    ///
    /// Sub-merge patterns are alternative decompositions of the merged token
    /// that were discovered during the partition merge process.
    pub(crate) fn add_sub_merges(
        &mut self,
        token: JoinRef,
    ) {
        let sub_merges = self.range_map.range_sub_merges(&self.partition_range);
        let origin = PatternOrigin::SubMerge(SubMergeKind::Partition);
        let plan = &mut *self.merge_ctx.ctx.ctx.plan;
        match token {
            JoinRef::Candidate(id) => {
                let existing = plan.candidate_pattern_set(id);
                for refs in sub_merges {
                    if !existing.contains(&refs) {
                        plan.add_candidate_pattern(
                            id,
                            ProposedPattern { refs, origin },
                        );
                    }
                }
            },
            JoinRef::Existing(target) => {
                let trav = self.merge_ctx.ctx.ctx.trav;
                let mut existing: Vec<Vec<JoinRef>> = trav
                    .expect_child_patterns(target)
                    .into_values()
                    .map(|p| p.iter().map(|t| JoinRef::Existing(*t)).collect())
                    .chain(plan.added_patterns(target).cloned())
                    .collect();
                for refs in sub_merges {
                    if !existing.contains(&refs) {
                        existing.push(refs.clone());
                        plan.push_edit(crate::join::plan::PlanEdit::AddPattern {
                            target,
                            pattern: ProposedPattern { refs, origin },
                        });
                    }
                }
            },
        }
    }

    /// Internal merge implementation with configurable pattern replacement.
    fn merge_internal(
        mut self,
        skip_pattern_replacement: bool,
    ) -> MergeResult {
        debug!(
            range_start = self.partition_range.start(),
            range_end = self.partition_range.end(),
            num_offsets = self.merge_ctx.offsets.len(),
            skip_pattern_replacement,
            "MergePartitionCtx::merge: ENTERED"
        );

        match self.info_partition() {
            Ok(info) => {
                // Check if this will be a perfect match BEFORE calling join
                let will_have_perfect = info.perfect.complete().0.is_some();

                let joined = self.join_partition_with_options(
                    info,
                    skip_pattern_replacement,
                );

                debug!(
                    token = ?joined.index,
                    had_pattern_replacement = will_have_perfect && !skip_pattern_replacement,
                    "JoinPartitionInfo succeeded"
                );

                if !skip_pattern_replacement {
                    self.add_sub_merges(joined.index);
                }

                MergeResult {
                    token: joined.index,
                    had_pattern_replacement: will_have_perfect
                        && !skip_pattern_replacement,
                }
            },
            Err(existing) => {
                debug!(
                    ?existing,
                    range_start = self.partition_range.start(),
                    range_end = self.partition_range.end(),
                    "{}: Token already exists - using without modification",
                    R::ROLE_STR
                );
                MergeResult {
                    token: existing,
                    had_pattern_replacement: false,
                }
            },
        }
    }

    /// Merge the partition and return the merged reference.
    ///
    /// This is the main entry point that:
    /// 1. Gets partition info (or returns existing token)
    /// 2. Proposes a JoinedPartition
    /// 3. Proposes sub-merge patterns
    #[allow(dead_code)]
    pub(crate) fn merge(self) -> JoinRef {
        self.merge_internal(false).token
    }

    /// Merge the partition and return full result info.
    ///
    /// This includes whether a pattern replacement occurred.
    pub(crate) fn merge_with_info(self) -> MergeResult {
        self.merge_internal(false)
    }

    /// Merge the partition without modifying patterns.
    ///
    /// This is used for edge partitions in ROOT mode where we only need
    /// the token, not pattern modifications. This avoids corrupting the
    /// root pattern when merging edge partitions.
    pub(crate) fn merge_token_only(self) -> JoinRef {
        self.merge_internal(true).token
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
pub(crate) struct MergePartitionBuilder<'a> {
    offsets: &'a crate::SplitVertexCache,
    overlay: &'a NodeOverlay,
    partition_range: PartitionRange,
    num_partitions: usize,
}

impl<'a> MergePartitionBuilder<'a> {
    pub(crate) fn new(
        merge_ctx: &'a MergeCtx<'_>,
        partition_range: PartitionRange,
    ) -> Self {
        Self {
            offsets: &merge_ctx.offsets,
            overlay: &merge_ctx.ctx.overlay,
            partition_range,
            num_partitions: merge_ctx.num_partitions(),
        }
    }

    /// Split positions of the offset at `index`, located in the node's
    /// current overlay patterns from the stable atom offset.
    fn splits_at(
        &self,
        index: usize,
    ) -> VertexSplits {
        let VertexSplits { pos, splits } =
            self.offsets.pos_ctx_by_index(index).to_vertex_splits();
        let splits = splits
            .into_keys()
            .map(|pid| {
                let trace = trace_back_pos(self.overlay.pattern(&pid), pos)
                    .expect("offset must lie inside the node");
                (pid, trace)
            })
            .collect();
        VertexSplits { pos, splits }
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
        let ro = self.splits_at(partition_end);
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
        let lo = self.splits_at(partition_start - 1); // offset left of partition
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
        let lo = self.splits_at(partition_start - 1); // offset left of partition
        let ro = self.splits_at(partition_end);
        Partition { offsets: (lo, ro) }
    }
}

/// Uniform constructor for MergePartitionCtx using ToPartition trait.
impl<'a, 'b, R: RangeRole<Mode = Join> + 'b> MergePartitionCtx<'a, 'b, R>
where
    R::Borders: JoinBorders<R>,
    for<'c> MergePartitionBuilder<'c>: ToPartition<R>,
{
    pub(crate) fn from_merge_ctx(
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
