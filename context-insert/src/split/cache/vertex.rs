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
    join::context::node::merge::PartitionRange,
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
    iter::FromIterator,
    num::NonZeroUsize,
};
use tracing::debug;

#[derive(Debug, Default, Clone, PartialEq, Eq, Deref, DerefMut)]
pub struct SplitVertexCache {
    pub positions: BTreeMap<NonZeroUsize, SplitPositionCache>,
}

impl SplitVertexCache {
    pub fn pos_ctx_by_index(
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

    /// Apply delta adjustments to positions AFTER a given offset index.
    ///
    /// This decrements the `sub_index` for each pattern by the delta amount.
    /// Called after a partition is merged and patterns are replaced,
    /// so that subsequent lookups use correct indices into the modified patterns.
    ///
    /// Only offsets with index > `after_offset_index` are affected. Offsets at or
    /// before the merged partition should not have their sub_indices adjusted.
    pub fn apply_deltas(
        &mut self,
        deltas: &PatternSubDeltas,
        after_offset_index: usize,
    ) {
        for (idx, pos_cache) in self.positions.values_mut().enumerate() {
            if idx > after_offset_index {
                *pos_cache -= deltas;
            }
        }
    }

    pub fn new(
        pos: NonZeroUsize,
        entry: SplitPositionCache,
    ) -> Self {
        Self {
            positions: BTreeMap::from_iter([(pos, entry)]),
        }
    }
    pub fn node_augmentation(
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
    pub fn root_augmentation(
        &mut self,
        ctx: NodeTraceCtx,
        root_mode: RootMode,
    ) -> (Vec<SplitTraceState>, PartitionRange) {
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
            RootMode::Infix => unimplemented!(), // TODO: Add wrapper offsets for Infix
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
        (next, target_range)
    }
    pub fn pos_mut(
        &mut self,
        pos: NonZeroUsize,
    ) -> &mut SplitPositionCache {
        self.positions.entry(pos).or_default()
    }
    pub fn offset_range_partition<K: RangeRole>(
        &self,
        range: K::OffsetRange,
    ) -> Partition<K> {
        range.get_splits(self).to_partition()
    }
    pub fn inner_offsets<R: RangeRole<Mode = Trace>, P: ToPartition<R>>(
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
    pub fn add_inner_offsets<K: RangeRole<Mode = Trace>, P: ToPartition<K>>(
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
    /// and add split positions at wrapper boundaries
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
                    } else {
                        debug!("No inner_offset found in wrapper");
                    }
                }
            }

            // CRITICAL: Also check for inner_offset in the TARGET partition (at split_pos)
            // This gives us offsets WITHIN the target partition itself
            debug!(
                pos = pos.get(),
                "Checking for inner offsets in target partition"
            );
            for trace_pos in split_cache.pattern_splits.values() {
                if let Some(inner_offset) = trace_pos.inner_offset {
                    // The inner_offset is relative to the split position
                    let target_inner_offset = pos.get() + inner_offset.get();
                    debug!(
                        ?inner_offset,
                        target_inner_offset, "Found target inner_offset"
                    );

                    if let Some(target_inner_pos) =
                        NonZeroUsize::new(target_inner_offset)
                        && !self.positions.contains_key(&target_inner_pos)
                        && !wrapper_splits.contains_key(&target_inner_pos)
                    {
                        debug!(
                            pos = target_inner_pos.get(),
                            "Adding target inner offset"
                        );
                        wrapper_splits.insert(
                            target_inner_pos,
                            SplitPositionCache::root(position_splits(
                                ctx.patterns.iter(),
                                target_inner_pos,
                            )),
                        );
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
    /// For each child pattern, find the wrapper range and add split positions at wrapper boundaries
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
}
