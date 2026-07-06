use crate::{
    interval::partition::{
        Partition,
        ToPartition,
        delta::PatternSubDeltas,
        info::{
            InfoPartition,
            range::{
                mode::Trace,
                role::{
                    In,
                    Post,
                    Pre,
                    RangeRole,
                },
                splits::{
                    OffsetIndexRange,
                    RangeOffsets,
                },
            },
        },
    },
    join::context::node::merge::{
        PartitionRange,
        RequiredPartitions,
    },
    split::{
        cache::position::{
            PosKey,
            SplitPositionCache,
        },
        position_splits,
        trace::SplitTraceState,
        vertex::{
            PosSplitCtx,
            node::NodeTraceCtx,
            output::RootMode,
        },
    },
};
use context_trace::{
    HasVertexIndex,
    Token,
    graph::getters::ErrorReason,
};
use derive_more::derive::{
    Deref,
    DerefMut,
};
use itertools::Itertools;
use std::{
    collections::BTreeMap,
    num::NonZeroUsize,
};
use tracing::debug;

#[derive(Debug, Default, Clone, PartialEq, Eq, Deref, DerefMut)]
pub struct SplitVertexCache {
    pub(crate) positions: BTreeMap<NonZeroUsize, SplitPositionCache>,
}

impl SplitVertexCache {
    pub(crate) fn pos_ctx_by_index(
        &'_ self,
        index: usize,
    ) -> PosSplitCtx<'_> {
        self.positions
            .iter()
            .map(PosSplitCtx::from)
            .nth(index)
            .unwrap_or_else(|| {
                panic!(
                    "Expected offset at index {} in {:#?}",
                    index, self.positions
                )
            })
    }

    /// Check if the split at a given offset index is perfect (no inner_offset).
    ///
    /// A perfect split means the split position aligns exactly with child token
    /// boundaries in all patterns. Returns true if none of the patterns have
    /// an inner_offset at this position.
    pub(crate) fn is_split_perfect_at_index(
        &self,
        index: usize,
    ) -> bool {
        self.positions
            .values()
            .nth(index)
            .map(|cache| {
                cache
                    .pattern_splits
                    .values()
                    .all(|pos| pos.inner_offset.is_none())
            })
            .unwrap_or(true) // If no split at this index, consider it "perfect"
    }

    /// Apply delta adjustments to positions after a merge.
    ///
    /// This handles three categories of positions:
    /// 1. Positions INSIDE the merged region (partition_start..partition_end):
    ///    - These need sub_index adjustment AND inner_offset set
    /// 2. The right boundary position (at partition_end):
    ///    - This needs sub_index adjustment only
    /// 3. Positions AFTER the merged region (> partition_end):
    ///    - These need sub_index adjustment only
    ///
    /// The inner_offsets parameter provides the offset within the merged token
    /// for each position inside the merged region, keyed by enumerate index.
    pub(crate) fn apply_deltas_with_inner_offsets(
        &mut self,
        deltas: &PatternSubDeltas,
        partition_start: usize,
        partition_end: usize,
        inner_offsets: &std::collections::BTreeMap<
            usize,
            std::num::NonZeroUsize,
        >,
    ) {
        for (idx, pos_cache) in self.positions.values_mut().enumerate() {
            if idx >= partition_start && idx < partition_end {
                // Position is INSIDE the merged region
                if let Some(&inner_offset) = inner_offsets.get(&idx) {
                    debug!(
                        idx,
                        ?deltas,
                        ?inner_offset,
                        "Applying delta and inner_offset to inside position"
                    );
                    pos_cache
                        .apply_delta_with_inner_offset(deltas, inner_offset);
                } else {
                    // No inner offset info, just apply delta
                    debug!(
                        idx,
                        ?deltas,
                        "Applying delta to inside position (no inner_offset)"
                    );
                    *pos_cache -= deltas;
                }
            } else if idx >= partition_end {
                // Position is at or after the right boundary
                debug!(idx, ?deltas, "Applying delta to position");
                *pos_cache -= deltas;
            }
        }
    }

    /// Apply delta adjustments to positions AT OR AFTER a given offset index.
    ///
    /// This decrements the `sub_index` for each pattern by the delta amount.
    /// Called after a partition is merged and patterns are replaced,
    /// so that subsequent lookups use correct indices into the modified patterns.
    ///
    /// Offsets with index >= `from_offset_index` are affected. This includes:
    /// - The right boundary of the merged partition (at from_offset_index)
    /// - All positions after the merged partition (> from_offset_index)
    ///
    /// Offsets BEFORE from_offset_index (inside or before the merged region)
    /// should not have their sub_indices adjusted.
    #[allow(dead_code)]
    pub(crate) fn apply_deltas(
        &mut self,
        deltas: &PatternSubDeltas,
        from_offset_index: usize,
    ) {
        for (idx, pos_cache) in self.positions.values_mut().enumerate() {
            if idx >= from_offset_index {
                debug!(idx, ?deltas, "Applying delta to position");
                *pos_cache -= deltas;
            }
        }
    }

    #[allow(dead_code)]
    pub(crate) fn new(
        pos: NonZeroUsize,
        entry: SplitPositionCache,
    ) -> Self {
        Self {
            positions: BTreeMap::from_iter([(pos, entry)]),
        }
    }
    pub(crate) fn node_augmentation(
        &mut self,
        ctx: NodeTraceCtx,
    ) -> Vec<SplitTraceState> {
        let num_offsets = self.positions.len();
        debug!(?num_offsets,
            root_patterns = ?ctx.patterns,
            "node_augmentation"
        );
        let mut states = Vec::new();
        for len in 1..num_offsets {
            for start in 0..num_offsets - len + 1 {
                let Some(part) = self
                    .offset_range_partition::<In<Trace>>(start..start + len)
                else {
                    continue;
                };
                let (splits, next) = Self::add_inner_offsets(ctx.clone(), part);
                self.positions.extend(splits);
                states.extend(next);
            }
        }
        states
    }

    fn target_offset_indices(
        &self,
        target_positions: &[NonZeroUsize],
    ) -> Vec<usize> {
        self.positions
            .keys()
            .enumerate()
            .filter_map(|(index, pos)| target_positions.contains(pos).then_some(index))
            .collect_vec()
    }

    fn target_partition_range(
        root_mode: RootMode,
        target_offset_indices: &[usize],
        num_offsets: usize,
    ) -> PartitionRange {
        PartitionRange::from(match root_mode {
            RootMode::Infix => {
                assert_eq!(
                    target_offset_indices.len(),
                    2,
                    "Infix mode requires exactly 2 target positions"
                );
                (target_offset_indices[0] + 1)..=target_offset_indices[1]
            },
            RootMode::Prefix => {
                assert_eq!(
                    target_offset_indices.len(),
                    1,
                    "Prefix mode requires exactly 1 target position"
                );
                0..=target_offset_indices[0]
            },
            RootMode::Postfix => {
                assert_eq!(
                    target_offset_indices.len(),
                    1,
                    "Postfix mode requires exactly 1 target position"
                );
                (target_offset_indices[0] + 1)..=num_offsets
            },
        })
    }

    fn wrapper_partition_range(
        root_mode: RootMode,
        num_offsets: usize,
    ) -> PartitionRange {
        PartitionRange::from(match root_mode {
            RootMode::Infix => 1..=(num_offsets - 1),
            RootMode::Prefix => 0..=(num_offsets - 1),
            RootMode::Postfix => 1..=num_offsets,
        })
    }

    fn root_mode_inner_splits(
        &self,
        ctx: NodeTraceCtx,
        root_mode: RootMode,
    ) -> Result<
        (
            BTreeMap<NonZeroUsize, SplitPositionCache>,
            Vec<SplitTraceState>,
        ),
        ErrorReason,
    > {
        let missing = || ErrorReason::MissingCacheEntry(ctx.index.vertex_index());

        match root_mode {
            RootMode::Infix => Ok(Self::add_inner_offsets(
                ctx.clone(),
                OffsetIndexRange::<In<Trace>>::get_splits(&(0..1), self)
                    .ok_or_else(missing)?,
            )),
            RootMode::Prefix => Ok(Self::add_inner_offsets::<Pre<Trace>, _>(
                ctx.clone(),
                OffsetIndexRange::<Pre<Trace>>::get_splits(&(..0), self)
                    .ok_or_else(missing)?,
            )),
            RootMode::Postfix => Ok(Self::add_inner_offsets::<Post<Trace>, _>(
                ctx.clone(),
                OffsetIndexRange::<Post<Trace>>::get_splits(&(0..), self)
                    .ok_or_else(missing)?,
            )),
        }
    }

    fn root_mode_wrapper_splits(
        &self,
        ctx: NodeTraceCtx,
        root_mode: RootMode,
    ) -> BTreeMap<NonZeroUsize, SplitPositionCache> {
        match root_mode {
            RootMode::Prefix => self.add_wrapper_offsets_prefix(ctx),
            RootMode::Postfix => self.add_wrapper_offsets_postfix(ctx),
            RootMode::Infix => self.add_wrapper_offsets_infix(ctx),
        }
    }

    pub(crate) fn root_augmentation(
        &mut self,
        ctx: NodeTraceCtx,
        root_mode: RootMode,
    ) -> Result<
        (Vec<SplitTraceState>, PartitionRange, RequiredPartitions),
        ErrorReason,
    > {
        let target_positions = self.positions.keys().cloned().collect_vec();
        debug!(?root_mode, ?target_positions,
            root_patterns = ?ctx.patterns,
            "root_augmentation"
        );

        let (splits, next) = self.root_mode_inner_splits(ctx.clone(), root_mode)?;
        debug!(
            inner_offsets = ?splits.keys().collect_vec(),
            "After add_inner_offsets"
        );
        self.positions.extend(splits);

        let wrapper_splits = self.root_mode_wrapper_splits(ctx.clone(), root_mode);

        debug!(
            wrapper_offsets = ?wrapper_splits.keys().collect_vec(),
            "After add_wrapper_offsets"
        );
        self.positions.extend(wrapper_splits);

        debug!(final_positions=?self.positions.keys().collect::<Vec<_>>(), "Final positions");

        let toi = self.target_offset_indices(&target_positions);

        debug!(?toi, "target offset indices");

        let num_offsets = self.positions.len();
        let target_range = Self::target_partition_range(root_mode, &toi, num_offsets);

        debug!(?target_range, "calculated target_range");

        let wrapper_range = Self::wrapper_partition_range(root_mode, num_offsets);

        debug!(?wrapper_range, "calculated wrapper_range");

        let required =
            self.compute_required_partitions(&target_range, &wrapper_range);
        debug!(required = ?required.iter().collect::<Vec<_>>(), "computed required partitions");

        Ok((next, target_range, required))
    }

    fn boundary_requires_wrapper(
        &self,
        target_range: &PartitionRange,
        wrapper_range: &PartitionRange,
    ) -> (bool, bool) {
        let target_start = *target_range.start();
        let target_end = *target_range.end();
        let wrapper_start = *wrapper_range.start();
        let wrapper_end = *wrapper_range.end();

        let right_boundary_unperfect = wrapper_end > target_end
            && !self.is_split_perfect_at_index(target_end);
        let left_boundary_unperfect = wrapper_start < target_start
            && target_start > 0
            && !self.is_split_perfect_at_index(target_start - 1);

        (left_boundary_unperfect, right_boundary_unperfect)
    }

    fn add_partition_if_valid(
        required: &mut RequiredPartitions,
        range: PartitionRange,
    ) {
        if range.start() <= range.end() {
            required.add(range);
        }
    }

    /// Compute required partitions from target and wrapper ranges.
    ///
    /// Required partitions are:
    /// 1. Target partition (the token being inserted)
    /// 2. Wrapper partition (extends to aligned boundary for unperfect splits)
    /// 3. Inner partitions: the prefix/suffix of target that aligns with wrapper boundaries
    ///
    /// The wrapper is only needed if there's an unperfect split at the boundary
    /// between target and wrapper ranges. A perfect split (no inner_offset)
    /// means the split aligns with child token boundaries and no wrapper is needed.
    fn compute_required_partitions(
        &self,
        target_range: &PartitionRange,
        wrapper_range: &PartitionRange,
    ) -> RequiredPartitions {
        let mut required = RequiredPartitions::new();
        required.add(target_range.clone());
        if wrapper_range != target_range {
            let target_start = *target_range.start();
            let target_end = *target_range.end();
            let wrapper_start = *wrapper_range.start();
            let wrapper_end = *wrapper_range.end();

            let (left_boundary_unperfect, right_boundary_unperfect) =
                self.boundary_requires_wrapper(target_range, wrapper_range);
            if right_boundary_unperfect || left_boundary_unperfect {
                required.add(wrapper_range.clone());
                if right_boundary_unperfect && target_end > target_start {
                    required.add(PartitionRange::new(target_start..=(target_end - 1)));
                }
                if right_boundary_unperfect {
                    Self::add_partition_if_valid(
                        &mut required,
                        PartitionRange::new((target_end + 1)..=wrapper_end),
                    );
                }
                if left_boundary_unperfect && target_start > wrapper_start {
                    Self::add_partition_if_valid(
                        &mut required,
                        PartitionRange::new(wrapper_start..=(target_start - 1)),
                    );
                }
            }
        }

        required
    }

    fn wrapper_start_offset(
        pattern: &[Token],
        child_index: usize,
    ) -> usize {
        pattern
            .iter()
            .take(child_index)
            .map(|child| *child.width)
            .sum()
    }

    fn wrapper_end_offset(
        pattern: &[Token],
        child_index: usize,
    ) -> usize {
        pattern
            .iter()
            .take(child_index + 1)
            .map(|child| *child.width)
            .sum()
    }

    fn root_width(pattern: &[Token]) -> usize {
        pattern.iter().map(|child| *child.width).sum()
    }

    fn insert_wrapper_split(
        &self,
        ctx: &NodeTraceCtx,
        wrapper_splits: &mut BTreeMap<NonZeroUsize, SplitPositionCache>,
        offset: usize,
    ) {
        if let Some(wrapper_pos) = NonZeroUsize::new(offset)
            && !self.positions.contains_key(&wrapper_pos)
            && !wrapper_splits.contains_key(&wrapper_pos)
        {
            wrapper_splits.insert(
                wrapper_pos,
                SplitPositionCache::root(position_splits(
                    ctx.patterns.iter(),
                    wrapper_pos,
                )),
            );
        }
    }

    fn split_cache_at(
        &self,
        pos: Option<NonZeroUsize>,
    ) -> Option<&SplitPositionCache> {
        pos.and_then(|pos| self.positions.get(&pos))
    }

    pub(crate) fn offset_range_partition<K: RangeRole>(
        &self,
        range: K::OffsetRange,
    ) -> Option<Partition<K>> {
        Some(range.get_splits(self)?.to_partition())
    }
    pub(crate) fn inner_offsets<
        R: RangeRole<Mode = Trace>,
        P: ToPartition<R>,
    >(
        ctx: NodeTraceCtx,
        part: P,
    ) -> Vec<NonZeroUsize> {
        part.info_partition(&ctx)
            .map(|bundle|
                //let merges = range_map.range_sub_merges(start..start + len);
                bundle.patterns.into_iter().flat_map(|(_pid, info)|
                    info.inner_range.map(|range| {
                        let splits = range.offsets.as_splits(ctx.clone());
                        Self::inner_offsets(
                            ctx.clone(),
                            splits,
                        )
                    })
                )
                    .flatten()
                    .collect())
            .unwrap_or_default()
    }
    pub(crate) fn add_inner_offsets<
        K: RangeRole<Mode = Trace>,
        P: ToPartition<K>,
    >(
        ctx: NodeTraceCtx,
        part: P,
    ) -> (
        BTreeMap<NonZeroUsize, SplitPositionCache>,
        Vec<SplitTraceState>,
    )
//where K::Mode: ModeChildren::<K>,
    {
        let offsets = Self::inner_offsets(ctx.clone(), part);
        let splits: BTreeMap<_, _> = offsets
            .into_iter()
            .map(|offset| {
                (
                    offset,
                    SplitPositionCache::root(position_splits(
                        ctx.patterns.iter(),
                        offset,
                    )),
                )
            })
            .collect();
        let states = splits
            .iter()
            .flat_map(|(offset, cache)| {
                let key = PosKey::new(ctx.index, *offset);
                cache
                    .pattern_splits
                    .iter()
                    .flat_map(|(pid, pos)| {
                        pos.inner_offset.map(|inner_offset| {
                            let pattern = &ctx.patterns[pid];
                            SplitTraceState {
                                index: pattern[pos.sub_index],
                                offset: inner_offset,
                                prev: key,
                            }
                        })
                    })
                    .collect_vec()
            })
            .collect();
        (splits, states)
    }

    /// Add wrapper offsets for Postfix mode
    /// For each child pattern, find the wrapper range (indices intersected by target partition)
    /// and add split positions at wrapper boundaries.
    /// Wrapper offsets are only added when there's an unperfect split (inner_offset is Some).
    fn add_wrapper_offsets_postfix(
        &self,
        ctx: NodeTraceCtx,
    ) -> BTreeMap<NonZeroUsize, SplitPositionCache> {
        let split_pos = self.positions.keys().next().copied();
        debug!(?split_pos, "ADD_WRAPPER_OFFSETS_POSTFIX");

        let mut wrapper_splits = BTreeMap::new();
        let Some(split_cache) = self.split_cache_at(split_pos) else {
            return wrapper_splits;
        };

        for (pid, pattern) in ctx.patterns.iter() {
            if let Some(trace_pos) = split_cache.pattern_splits.get(pid) {
                debug!(?pid, ?trace_pos, "Processing pattern");
                let Some(inner_offset) = trace_pos.inner_offset else {
                    debug!("Perfect split, skipping wrapper offset");
                    continue;
                };

                let child_index = trace_pos.sub_index;
                let wrapper_start_offset = Self::wrapper_start_offset(pattern, child_index);
                debug!(wrapper_start_offset, child_index, wrapper_child = ?pattern[child_index], "Wrapper child info");

                if wrapper_start_offset > 0 {
                    self.insert_wrapper_split(&ctx, &mut wrapper_splits, wrapper_start_offset);
                }

                let intermediate_offset = wrapper_start_offset + inner_offset.get();
                debug!(?inner_offset, intermediate_offset, "Found inner offset in wrapper");
                self.insert_wrapper_split(&ctx, &mut wrapper_splits, intermediate_offset);
            }
        }

        debug!(count = wrapper_splits.len(), "Total wrapper_splits");
        wrapper_splits
    }

    /// Add wrapper offsets for Prefix mode
    /// For each child pattern, find the wrapper range and add split positions at wrapper boundaries.
    /// Wrapper offsets are only added when there's an unperfect split (inner_offset is Some).
    fn add_wrapper_offsets_prefix(
        &self,
        ctx: NodeTraceCtx,
    ) -> BTreeMap<NonZeroUsize, SplitPositionCache> {
        let split_pos = self.positions.keys().next().copied();

        let mut wrapper_splits = BTreeMap::new();
        let Some(split_cache) = self.split_cache_at(split_pos) else {
            return wrapper_splits;
        };

        for (pid, pattern) in ctx.patterns.iter() {
            if let Some(trace_pos) = split_cache.pattern_splits.get(pid) {
                if trace_pos.inner_offset.is_none() {
                    continue;
                }

                let child_index = trace_pos.sub_index;
                let wrapper_end_offset = Self::wrapper_end_offset(pattern, child_index);
                if wrapper_end_offset < Self::root_width(pattern) {
                    self.insert_wrapper_split(&ctx, &mut wrapper_splits, wrapper_end_offset);
                }
            }
        }

        wrapper_splits
    }

    /// Add wrapper offsets for Infix mode
    /// Infix has TWO split positions (left and right bounds).
    /// - For left split: add wrapper END offset (like Prefix)
    /// - For right split: add wrapper START offset (like Postfix)
    /// Wrapper offsets are only added when there's an unperfect split (inner_offset is Some).
    fn add_wrapper_offsets_infix(
        &self,
        ctx: NodeTraceCtx,
    ) -> BTreeMap<NonZeroUsize, SplitPositionCache> {
        let mut positions = self.positions.keys().copied();
        let left_pos = positions.next();
        let right_pos = positions.next();

        debug!(?left_pos, ?right_pos, "ADD_WRAPPER_OFFSETS_INFIX");

        let mut wrapper_splits = BTreeMap::new();

        if let Some(split_cache) = self.split_cache_at(left_pos) {
            for (pid, pattern) in ctx.patterns.iter() {
                if let Some(trace_pos) = split_cache.pattern_splits.get(pid) {
                    if trace_pos.inner_offset.is_none() {
                        continue;
                    }

                    let wrapper_end_offset =
                        Self::wrapper_end_offset(pattern, trace_pos.sub_index);
                    if wrapper_end_offset < Self::root_width(pattern) {
                        self.insert_wrapper_split(&ctx, &mut wrapper_splits, wrapper_end_offset);
                    }
                }
            }
        }

        if let Some(split_cache) = self.split_cache_at(right_pos) {
            for (pid, pattern) in ctx.patterns.iter() {
                if let Some(trace_pos) = split_cache.pattern_splits.get(pid) {
                    let Some(inner_offset) = trace_pos.inner_offset else {
                        continue;
                    };

                    let child_index = trace_pos.sub_index;
                    let wrapper_start_offset = Self::wrapper_start_offset(pattern, child_index);
                    if wrapper_start_offset > 0 {
                        self.insert_wrapper_split(&ctx, &mut wrapper_splits, wrapper_start_offset);
                    }

                    self.insert_wrapper_split(
                        &ctx,
                        &mut wrapper_splits,
                        wrapper_start_offset + inner_offset.get(),
                    );

                    let wrapper_end_offset = Self::wrapper_end_offset(pattern, child_index);
                    if wrapper_end_offset < Self::root_width(pattern) {
                        self.insert_wrapper_split(&ctx, &mut wrapper_splits, wrapper_end_offset);
                    }
                }
            }
        }

        wrapper_splits
    }
}
