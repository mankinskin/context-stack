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
                let part = self
                    .offset_range_partition::<In<Trace>>(start..start + len);
                let (splits, next) = Self::add_inner_offsets(ctx.clone(), part);
                self.positions.extend(splits);
                states.extend(next);
            }
        }
        states
    }
    pub(crate) fn root_augmentation(
        &mut self,
        ctx: NodeTraceCtx,
        root_mode: RootMode,
    ) -> (Vec<SplitTraceState>, PartitionRange, RequiredPartitions) {
        let target_positions = self.positions.keys().cloned().collect_vec();
        debug!(?root_mode, ?target_positions,
            root_patterns = ?ctx.patterns,
            "root_augmentation"
        );

        // First add inner offsets for the target partition
        let (splits, next) = match root_mode {
            RootMode::Infix => Self::add_inner_offsets(
                ctx.clone(),
                OffsetIndexRange::<In<Trace>>::get_splits(&(0..1), self),
            ),
            RootMode::Prefix => Self::add_inner_offsets::<Pre<Trace>, _>(
                ctx.clone(),
                OffsetIndexRange::<Pre<Trace>>::get_splits(&(..0), self),
            ),
            RootMode::Postfix => Self::add_inner_offsets::<Post<Trace>, _>(
                ctx.clone(),
                OffsetIndexRange::<Post<Trace>>::get_splits(&(0..), self),
            ),
        };
        debug!(
            inner_offsets = ?splits.keys().collect_vec(),
            "After add_inner_offsets"
        );
        self.positions.extend(splits);

        // Then add wrapper offsets for Prefix/Postfix modes
        let wrapper_splits = match root_mode {
            RootMode::Prefix => self.add_wrapper_offsets_prefix(ctx.clone()),
            RootMode::Postfix => self.add_wrapper_offsets_postfix(ctx.clone()),
            RootMode::Infix => self.add_wrapper_offsets_infix(ctx.clone()),
        };

        debug!(
            wrapper_offsets = ?wrapper_splits.keys().collect_vec(),
            "After add_wrapper_offsets"
        );
        self.positions.extend(wrapper_splits);

        debug!(final_positions=?self.positions.keys().collect::<Vec<_>>(), "Final positions");

        // Find where the original target positions are in the final positions array
        // toi = "target offset indices"
        let toi = self
            .positions
            .keys()
            .enumerate()
            .filter_map(|(i, pos)| target_positions.contains(pos).then_some(i))
            .collect_vec();

        debug!(?toi, "target offset indices");

        // Calculate target partition range based on root mode and original target positions
        // The target range is defined by where the ORIGINAL target positions are,
        // not by the total number of offsets (which includes wrapper offsets)
        let target_range = PartitionRange::from(match root_mode {
            RootMode::Infix => {
                // Infix: partitions between first and last target offset
                assert_eq!(
                    toi.len(),
                    2,
                    "Infix mode requires exactly 2 target positions"
                );
                // Partition range from after first offset to before last offset
                (toi[0] + 1)..=toi[1]
            },
            RootMode::Prefix => {
                // Prefix: all partitions left of (and including) the last target offset
                assert_eq!(
                    toi.len(),
                    1,
                    "Prefix mode requires exactly 1 target position"
                );
                // Partition range from 0 to the target offset index
                0..=toi[0]
            },
            RootMode::Postfix => {
                // Postfix: all partitions right of (and including) the first target offset
                assert_eq!(
                    toi.len(),
                    1,
                    "Postfix mode requires exactly 1 target position"
                );
                let num_offsets = self.positions.len();
                // Partition range from after target offset to end
                (toi[0] + 1)..=num_offsets
            },
        });

        debug!(?target_range, "calculated target_range");

        // Compute required partitions based on target and wrapper ranges
        // The wrapper range is the full operating range (from first to last offset index)
        let num_offsets = self.positions.len();
        let wrapper_range = PartitionRange::from(match root_mode {
            RootMode::Infix => {
                // Wrapper spans from first to last offset
                // For infix with 5 offsets: partitions 1..=4 (skip prefix 0 and postfix 5)
                1..=(num_offsets - 1)
            },
            RootMode::Prefix => {
                // Wrapper spans from 0 to last wrapper offset
                0..=(num_offsets - 1)
            },
            RootMode::Postfix => {
                // Wrapper spans from first wrapper offset to end
                1..=num_offsets
            },
        });

        debug!(?wrapper_range, "calculated wrapper_range");

        let required =
            self.compute_required_partitions(&target_range, &wrapper_range);
        debug!(required = ?required.iter().collect::<Vec<_>>(), "computed required partitions");

        (next, target_range, required)
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

        // Target is always required
        required.add(target_range.clone());

        // Wrapper is required only if:
        // 1. It differs from target AND
        // 2. There's an unperfect split at the boundary
        if wrapper_range != target_range {
            let target_start = *target_range.start();
            let target_end = *target_range.end();
            let wrapper_start = *wrapper_range.start();
            let wrapper_end = *wrapper_range.end();

            // Check if right boundary split is unperfect (wrapper extends past target on right)
            let right_boundary_unperfect = wrapper_end > target_end
                && !self.is_split_perfect_at_index(target_end);

            // Check if left boundary split is unperfect (wrapper extends before target on left)
            let left_boundary_unperfect = wrapper_start < target_start
                && target_start > 0
                && !self.is_split_perfect_at_index(target_start - 1);

            // Only add wrapper-related partitions if there's an unperfect boundary
            if right_boundary_unperfect || left_boundary_unperfect {
                required.add(wrapper_range.clone());

                // Inner partitions: the prefix of target up to the first pattern-aligned boundary
                // For infix: if target is 1..=3 and wrapper is 1..=4, inner is 1..=2
                // This is the portion of target before the unperfect boundary child
                if right_boundary_unperfect && target_end > target_start {
                    let inner_range =
                        PartitionRange::new(target_start..=(target_end - 1));
                    required.add(inner_range);
                }

                // Also add the suffix of wrapper after target (e.g., 3..=4 for yz)
                // This ensures we can build pattern [aby, z] for abyz
                if right_boundary_unperfect {
                    let suffix_range =
                        PartitionRange::new((target_end + 1)..=wrapper_end);
                    if suffix_range.start() <= suffix_range.end() {
                        required.add(suffix_range);
                    }
                }

                // Handle left boundary extension similarly
                if left_boundary_unperfect && target_start > wrapper_start {
                    let prefix_range =
                        PartitionRange::new(wrapper_start..=(target_start - 1));
                    if prefix_range.start() <= prefix_range.end() {
                        required.add(prefix_range);
                    }
                }
            }
        }

        required
    }

    pub(crate) fn offset_range_partition<K: RangeRole>(
        &self,
        range: K::OffsetRange,
    ) -> Partition<K> {
        range.get_splits(self).to_partition()
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
        // Get the first (and only) split position for postfix
        let split_pos = self.positions.keys().next().copied();

        debug!(?split_pos, "ADD_WRAPPER_OFFSETS_POSTFIX");

        if let Some(pos) = split_pos {
            let split_cache = &self.positions[&pos];
            let mut wrapper_splits = BTreeMap::new();

            // For each child pattern, find wrapper start position and inner offsets
            for (pid, pattern) in ctx.patterns.iter() {
                if let Some(trace_pos) = split_cache.pattern_splits.get(pid) {
                    debug!(?pid, ?trace_pos, "Processing pattern");

                    // Only add wrapper offset if this is an unperfect split
                    // A perfect split (inner_offset = None) doesn't need a wrapper
                    if trace_pos.inner_offset.is_none() {
                        debug!("Perfect split, skipping wrapper offset");
                        continue;
                    }

                    // The wrapper starts at the beginning of the child that contains the split
                    let child_index = trace_pos.sub_index;
                    let wrapper_child = pattern[child_index];

                    // Calculate the wrapper start offset (start of this child in root)
                    let mut wrapper_start_offset = 0;
                    for i in 0..child_index {
                        wrapper_start_offset += *pattern[i].width;
                    }

                    debug!(
                        wrapper_start_offset,
                        child_index,
                        ?wrapper_child,
                        "Wrapper child info"
                    );

                    // Add wrapper start position if not already present and not at root start
                    if wrapper_start_offset > 0
                        && let Some(wrapper_pos) =
                            NonZeroUsize::new(wrapper_start_offset)
                        && !self.positions.contains_key(&wrapper_pos)
                    {
                        debug!(
                            pos = wrapper_pos.get(),
                            "Adding wrapper offset"
                        );
                        wrapper_splits.insert(
                            wrapper_pos,
                            SplitPositionCache::root(position_splits(
                                ctx.patterns.iter(),
                                wrapper_pos,
                            )),
                        );
                    }

                    // If the split has an inner_offset, it means there's a split WITHIN the wrapper child
                    // Add this intermediate offset as well
                    if let Some(inner_offset) = trace_pos.inner_offset {
                        let intermediate_offset =
                            wrapper_start_offset + inner_offset.get();
                        debug!(
                            ?inner_offset,
                            intermediate_offset,
                            "Found inner offset in wrapper"
                        );
                        if let Some(intermediate_pos) =
                            NonZeroUsize::new(intermediate_offset)
                            && !self.positions.contains_key(&intermediate_pos)
                            && !wrapper_splits.contains_key(&intermediate_pos)
                        {
                            debug!(
                                pos = intermediate_pos.get(),
                                "Adding intermediate offset"
                            );
                            wrapper_splits.insert(
                                intermediate_pos,
                                SplitPositionCache::root(position_splits(
                                    ctx.patterns.iter(),
                                    intermediate_pos,
                                )),
                            );
                        }
                    }
                }
            }

            debug!(count = wrapper_splits.len(), "Total wrapper_splits");
            wrapper_splits
        } else {
            BTreeMap::new()
        }
    }

    /// Add wrapper offsets for Prefix mode
    /// For each child pattern, find the wrapper range and add split positions at wrapper boundaries.
    /// Wrapper offsets are only added when there's an unperfect split (inner_offset is Some).
    fn add_wrapper_offsets_prefix(
        &self,
        ctx: NodeTraceCtx,
    ) -> BTreeMap<NonZeroUsize, SplitPositionCache> {
        // Get the first (and only) split position for prefix
        let split_pos = self.positions.keys().next().copied();

        if let Some(pos) = split_pos {
            let split_cache = &self.positions[&pos];
            let mut wrapper_splits = BTreeMap::new();

            // For each child pattern, find wrapper end position
            for (pid, pattern) in ctx.patterns.iter() {
                if let Some(trace_pos) = split_cache.pattern_splits.get(pid) {
                    // Only add wrapper offset if this is an unperfect split
                    // A perfect split (inner_offset = None) doesn't need a wrapper
                    if trace_pos.inner_offset.is_none() {
                        continue;
                    }

                    // The wrapper ends after the child that contains the split
                    let child_index = trace_pos.sub_index;

                    // Calculate the wrapper end offset (end of this child in root)
                    let mut wrapper_end_offset = 0;
                    for i in 0..=child_index {
                        wrapper_end_offset += *pattern[i].width;
                    }

                    // Add wrapper end position if not at root end
                    let root_width: usize =
                        pattern.iter().map(|c| *c.width).sum();
                    if wrapper_end_offset < root_width
                        && let Some(wrapper_pos) =
                            NonZeroUsize::new(wrapper_end_offset)
                        && !self.positions.contains_key(&wrapper_pos)
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
            }

            wrapper_splits
        } else {
            BTreeMap::new()
        }
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

        // Handle LEFT split position - add wrapper END offset (like Prefix)
        if let Some(pos) = left_pos {
            let split_cache = &self.positions[&pos];

            for (pid, pattern) in ctx.patterns.iter() {
                if let Some(trace_pos) = split_cache.pattern_splits.get(pid) {
                    // Only add wrapper offset if this is an unperfect split
                    if trace_pos.inner_offset.is_none() {
                        continue;
                    }

                    // The wrapper ends after the child that contains the split
                    let child_index = trace_pos.sub_index;

                    // Calculate the wrapper end offset (end of this child in root)
                    let mut wrapper_end_offset = 0;
                    for i in 0..=child_index {
                        wrapper_end_offset += *pattern[i].width;
                    }

                    // Add wrapper end position if not at root end
                    let root_width: usize =
                        pattern.iter().map(|c| *c.width).sum();
                    if wrapper_end_offset < root_width
                        && let Some(wrapper_pos) =
                            NonZeroUsize::new(wrapper_end_offset)
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
            }
        }

        // Handle RIGHT split position - add wrapper START offset (like Postfix)
        if let Some(pos) = right_pos {
            let split_cache = &self.positions[&pos];

            for (pid, pattern) in ctx.patterns.iter() {
                if let Some(trace_pos) = split_cache.pattern_splits.get(pid) {
                    // Only add wrapper offset if this is an unperfect split
                    if trace_pos.inner_offset.is_none() {
                        continue;
                    }

                    // The wrapper starts at the beginning of the child that contains the split
                    let child_index = trace_pos.sub_index;

                    // Calculate the wrapper start offset (start of this child in root)
                    let mut wrapper_start_offset = 0;
                    for i in 0..child_index {
                        wrapper_start_offset += *pattern[i].width;
                    }

                    // Add wrapper start position if not already present and not at root start
                    if wrapper_start_offset > 0
                        && let Some(wrapper_pos) =
                            NonZeroUsize::new(wrapper_start_offset)
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

                    // If the split has an inner_offset, add intermediate offset
                    if let Some(inner_offset) = trace_pos.inner_offset {
                        let intermediate_offset =
                            wrapper_start_offset + inner_offset.get();
                        if let Some(intermediate_pos) =
                            NonZeroUsize::new(intermediate_offset)
                            && !self.positions.contains_key(&intermediate_pos)
                            && !wrapper_splits.contains_key(&intermediate_pos)
                        {
                            wrapper_splits.insert(
                                intermediate_pos,
                                SplitPositionCache::root(position_splits(
                                    ctx.patterns.iter(),
                                    intermediate_pos,
                                )),
                            );
                        }
                    }

                    // Also add wrapper END offset for the RIGHT split (needed for outer wrapper like abyz)
                    // This is the end of the child that contains the right split
                    let root_width: usize =
                        pattern.iter().map(|c| *c.width).sum();
                    let mut wrapper_end_offset = 0;
                    for i in 0..=child_index {
                        wrapper_end_offset += *pattern[i].width;
                    }

                    // Add wrapper end position if not at root end
                    if wrapper_end_offset < root_width
                        && let Some(wrapper_pos) =
                            NonZeroUsize::new(wrapper_end_offset)
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
            }
        }

        wrapper_splits
    }
}
