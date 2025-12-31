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
                let (splits, next) = Self::add_inner_offsets(ctx, part);
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
        // First add inner offsets for the target partition
        let (splits, next) = match root_mode {
            RootMode::Infix => Self::add_inner_offsets(
                ctx,
                OffsetIndexRange::<In<Trace>>::get_splits(&(0..1), self),
            ),
            RootMode::Prefix => Self::add_inner_offsets::<Pre<Trace>, _>(
                ctx,
                OffsetIndexRange::<Pre<Trace>>::get_splits(&(0..0), self),
            ),
            RootMode::Postfix => Self::add_inner_offsets::<Post<Trace>, _>(
                ctx,
                OffsetIndexRange::<Post<Trace>>::get_splits(&(0..), self),
            ),
        };
        self.positions.extend(splits);
        
        // Then add wrapper offsets for Prefix/Postfix modes
        let wrapper_splits = match root_mode {
            RootMode::Prefix => self.add_wrapper_offsets_prefix(ctx),
            RootMode::Postfix => self.add_wrapper_offsets_postfix(ctx),
            RootMode::Infix => BTreeMap::new(), // Infix handles wrappers differently
        };
        self.positions.extend(wrapper_splits);
        
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
        range: K::Range,
    ) -> Partition<K> {
        range.get_splits(self).to_partition()
    }
    pub fn inner_offsets<
        'a: 't,
        't,
        R: RangeRole<Mode = Trace>,
        P: ToPartition<R>,
    >(
        ctx: NodeTraceCtx<'a>,
        part: P,
    ) -> Vec<NonZeroUsize> {
        part.info_partition(&ctx)
            .map(|bundle|
                //let merges = range_map.range_sub_merges(start..start + len);
                bundle.patterns.into_iter().flat_map(|(_pid, info)|
                    info.inner_range.map(|range| {
                        let splits = range.offsets.as_splits(ctx);
                        Self::inner_offsets(
                            ctx,
                            splits,
                        )
                    })
                )
                    .flatten()
                    .collect())
            .unwrap_or_default()
    }
    pub fn add_inner_offsets<
        'a: 't,
        't,
        K: RangeRole<Mode = Trace>,
        P: ToPartition<K>,
    >(
        ctx: NodeTraceCtx<'a>,
        part: P,
    ) -> (
        BTreeMap<NonZeroUsize, SplitPositionCache>,
        Vec<SplitTraceState>,
    )
//where K::Mode: ModeChildren::<K>,
    {
        let offsets = Self::inner_offsets(ctx, part);
        let splits: BTreeMap<_, _> = offsets
            .into_iter()
            .map(|offset| {
                (
                    offset,
                    SplitPositionCache::root(position_splits(
                        ctx.patterns,
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
        
        if let Some(pos) = split_pos {
            let split_cache = &self.positions[&pos];
            let mut wrapper_splits = BTreeMap::new();
            
            // For each child pattern, find wrapper start position
            for (pid, pattern) in ctx.patterns.iter() {
                if let Some(trace_pos) = split_cache.pattern_splits.get(pid) {
                    // The wrapper starts at the beginning of the child that contains the split
                    let child_index = trace_pos.sub_index;
                    
                    // Calculate the wrapper start offset (start of this child in root)
                    let mut wrapper_start_offset = 0;
                    for i in 0..child_index {
                        wrapper_start_offset += *pattern[i].width;
                    }
                    
                    // Add wrapper start position if not already present and not at root start
                    if wrapper_start_offset > 0 {
                        if let Some(wrapper_pos) = NonZeroUsize::new(wrapper_start_offset) {
                            if !self.positions.contains_key(&wrapper_pos) {
                                wrapper_splits.insert(
                                    wrapper_pos,
                                    SplitPositionCache::root(position_splits(
                                        ctx.patterns,
                                        wrapper_pos,
                                    )),
                                );
                            }
                        }
                    }
                }
            }
            
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
                    let root_width: usize = pattern.iter().map(|c| *c.width).sum();
                    if wrapper_end_offset < root_width {
                        if let Some(wrapper_pos) = NonZeroUsize::new(wrapper_end_offset) {
                            if !self.positions.contains_key(&wrapper_pos) {
                                wrapper_splits.insert(
                                    wrapper_pos,
                                    SplitPositionCache::root(position_splits(
                                        ctx.patterns,
                                        wrapper_pos,
                                    )),
                                );
                            }
                        }
                    }
                }
            }
            
            wrapper_splits
        } else {
            BTreeMap::new()
        }
    }
}
