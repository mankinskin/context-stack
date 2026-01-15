use crate::{
    interval::partition::{
        Partition,
        ToPartition,
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
    split::{
        cache::position::{
            PosKey,
            SplitPositionCache,
        },
        position_splits,
        trace::SplitTraceState,
        vertex::{
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

#[derive(Debug, Default, Clone, PartialEq, Eq, Deref, DerefMut)]
pub struct SplitVertexCache {
    pub positions: BTreeMap<NonZeroUsize, SplitPositionCache>,
}

impl SplitVertexCache {
    pub fn new(
        pos: NonZeroUsize,
        entry: SplitPositionCache,
    ) -> Self {
        Self {
            positions: BTreeMap::from_iter([(pos, entry)]),
        }
    }
    pub fn augment_node(
        &mut self,
        ctx: NodeTraceCtx,
    ) -> Vec<SplitTraceState> {
        let num_offsets = self.positions.len();
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
    pub fn augment_root(
        &mut self,
        ctx: NodeTraceCtx,
        root_mode: RootMode,
    ) -> Vec<SplitTraceState> {
        eprintln!("AUGMENT_ROOT: root_mode={:?}, existing positions: {:?}", root_mode, self.positions.keys().collect::<Vec<_>>());
        
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
            RootMode::Postfix => {
                eprintln!("  Postfix mode - checking target split at first position");
                if let Some(split_pos) = self.positions.keys().next().copied() {
                    eprintln!("    split_pos={}, cache={:?}", split_pos, self.positions[&split_pos]);
                }
                Self::add_inner_offsets::<Post<Trace>, _>(
                    ctx.clone(),
                    OffsetIndexRange::<Post<Trace>>::get_splits(&(0..), self),
                )
            },
        };
        eprintln!("  After add_inner_offsets: {} splits added", splits.len());
        for (pos, _) in &splits {
            eprintln!("    offset at pos {}", pos);
        }
        self.positions.extend(splits);

        // Then add wrapper offsets for Prefix/Postfix modes
        let wrapper_splits = match root_mode {
            RootMode::Prefix => self.add_wrapper_offsets_prefix(ctx.clone()),
            RootMode::Postfix => self.add_wrapper_offsets_postfix(ctx.clone()),
            RootMode::Infix => BTreeMap::new(), // Infix handles wrappers differently
        };
        eprintln!("  After add_wrapper_offsets: {} splits added", wrapper_splits.len());
        for (pos, _) in &wrapper_splits {
            eprintln!("    offset at pos {}", pos);
        }
        self.positions.extend(wrapper_splits);
        
        eprintln!("  Final positions: {:?}", self.positions.keys().collect::<Vec<_>>());

        next
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
    pub fn inner_offsets<
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
    pub fn add_inner_offsets<
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
    /// and add split positions at wrapper boundaries
    fn add_wrapper_offsets_postfix(
        &self,
        ctx: NodeTraceCtx,
    ) -> BTreeMap<NonZeroUsize, SplitPositionCache> {
        // Get the first (and only) split position for postfix
        let split_pos = self.positions.keys().next().copied();

        eprintln!("ADD_WRAPPER_OFFSETS_POSTFIX: split_pos={:?}", split_pos);

        if let Some(pos) = split_pos {
            let split_cache = &self.positions[&pos];
            let mut wrapper_splits = BTreeMap::new();

            // For each child pattern, find wrapper start position and inner offsets
            for (pid, pattern) in ctx.patterns.iter() {
                if let Some(trace_pos) = split_cache.pattern_splits.get(pid) {
                    eprintln!("  Pattern {:?}: trace_pos={:?}", pid, trace_pos);
                    
                    // The wrapper starts at the beginning of the child that contains the split
                    let child_index = trace_pos.sub_index;
                    let wrapper_child = pattern[child_index];

                    // Calculate the wrapper start offset (start of this child in root)
                    let mut wrapper_start_offset = 0;
                    for i in 0..child_index {
                        wrapper_start_offset += *pattern[i].width;
                    }

                    eprintln!("  wrapper_start_offset={}, child_index={}, wrapper_child={:?}", 
                              wrapper_start_offset, child_index, wrapper_child);

                    // Add wrapper start position if not already present and not at root start
                    if wrapper_start_offset > 0
                        && let Some(wrapper_pos) =
                            NonZeroUsize::new(wrapper_start_offset)
                        && !self.positions.contains_key(&wrapper_pos)
                    {
                        eprintln!("  Adding wrapper offset at pos {}", wrapper_pos);
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
                        let intermediate_offset = wrapper_start_offset + inner_offset.get();
                        eprintln!("  inner_offset={:?}, intermediate_offset={}", inner_offset, intermediate_offset);
                        if let Some(intermediate_pos) = NonZeroUsize::new(intermediate_offset)
                            && !self.positions.contains_key(&intermediate_pos)
                            && !wrapper_splits.contains_key(&intermediate_pos)
                        {
                            eprintln!("  Adding intermediate offset at pos {}", intermediate_pos);
                            wrapper_splits.insert(
                                intermediate_pos,
                                SplitPositionCache::root(position_splits(
                                    ctx.patterns.iter(),
                                    intermediate_pos,
                                )),
                            );
                        }
                    } else {
                        eprintln!("  No inner_offset found in wrapper");
                    }
                }
            }
            
            // CRITICAL: Also check for inner_offset in the TARGET partition (at split_pos)
            // This gives us offsets WITHIN the target partition itself
            eprintln!("  Checking for inner offsets in target partition at pos {}", pos);
            for (pid, trace_pos) in &split_cache.pattern_splits {
                if let Some(inner_offset) = trace_pos.inner_offset {
                    // The inner_offset is relative to the split position
                    let target_inner_offset = pos.get() + inner_offset.get();
                    eprintln!("    Found target inner_offset={:?}, target_inner_offset={}", inner_offset, target_inner_offset);
                    
                    if let Some(target_inner_pos) = NonZeroUsize::new(target_inner_offset)
                        && !self.positions.contains_key(&target_inner_pos)
                        && !wrapper_splits.contains_key(&target_inner_pos)
                    {
                        eprintln!("    Adding target inner offset at pos {}", target_inner_pos);
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

            eprintln!("  Total wrapper_splits: {}", wrapper_splits.len());
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
