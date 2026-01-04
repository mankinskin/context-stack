use std::{
    borrow::Borrow,
    num::NonZeroUsize,
};

use derive_more::{
    Deref,
    DerefMut,
};
use linked_hash_map::LinkedHashMap;

use crate::{
    interval::{
        IntervalGraph,
        partition::{
            Infix,
            Postfix,
            Prefix,
            info::{
                InfoPartition,
                borders::PartitionBorders,
                range::role::{
                    In,
                    Post,
                    Pre,
                },
            },
        },
    },
    join::{
        context::{
            frontier::FrontierSplitIterator,
            node::merge::NodeMergeCtx,
            pattern::PatternJoinCtx,
        },
        joined::partition::JoinedPartition,
        partition::{
            Join,
            JoinPartition,
            info::JoinPartitionInfo,
        },
    },
    split::{
        Split,
        SplitMap,
        cache::{
            position::PosKey,
            vertex::SplitVertexCache,
        },
        position_splits,
        vertex::{
            PosSplitCtx,
            TokenTracePositions,
            VertexSplits,
            node::{
                AsNodeTraceCtx,
                NodeTraceCtx,
            },
            output::RootMode,
            pattern::{
                GetPatternCtx,
                GetPatternTraceCtx,
                PatternTraceCtx,
            },
        },
    },
};
use context_trace::*;

#[derive(Debug)]
pub struct LockedFrontierCtx<'a> {
    pub trav: <HypergraphRef as HasGraphMut>::GuardMut<'a>,
    pub interval: &'a IntervalGraph,
    pub splits: &'a SplitMap,
}
impl<'a> LockedFrontierCtx<'a> {
    pub fn new(ctx: &'a mut FrontierSplitIterator) -> Self {
        Self {
            trav: ctx.trav.graph_mut(),
            interval: &ctx.frontier.interval,
            splits: &ctx.splits,
        }
    }
}
#[derive(Debug, Deref, DerefMut)]
pub struct NodeJoinCtx<'a> {
    #[deref]
    #[deref_mut]
    pub ctx: LockedFrontierCtx<'a>,
    pub index: Token,
}

impl<'a> NodeJoinCtx<'a> {
    pub fn new(
        index: Token,
        ctx: &'a mut FrontierSplitIterator,
    ) -> Self {
        NodeJoinCtx {
            index,
            ctx: LockedFrontierCtx::new(ctx),
        }
    }
}
impl<'a> AsNodeTraceCtx for NodeJoinCtx<'a> {
    fn as_trace_context<'t>(&'t self) -> NodeTraceCtx<'t>
    where
        Self: 't,
        'a: 't,
    {
        NodeTraceCtx::new(self.patterns(), self.borrow().index)
    }
}
impl GetPatternTraceCtx for NodeJoinCtx<'_> {
    fn get_pattern_trace_context<'c>(
        &'c self,
        pattern_id: &PatternId,
    ) -> PatternTraceCtx<'c>
    where
        Self: 'c,
    {
        PatternTraceCtx::new(
            self.index.to_pattern_location(*pattern_id),
            self.as_trace_context().patterns.get(pattern_id).unwrap(),
        )
    }
}
impl GetPatternCtx for NodeJoinCtx<'_> {
    type PatternCtx<'c>
        = PatternJoinCtx<'c>
    where
        Self: 'c;

    fn get_pattern_context<'c>(
        &'c self,
        pattern_id: &PatternId,
    ) -> Self::PatternCtx<'c>
    where
        Self: 'c,
    {
        let ctx = self.get_pattern_trace_context(pattern_id);
        PatternJoinCtx {
            ctx,
            splits: self.splits,
        }
    }
}
impl NodeJoinCtx<'_> {
    pub fn patterns(&self) -> &ChildPatterns {
        self.ctx.trav.expect_child_patterns(self.index)
    }
}

impl NodeJoinCtx<'_> {
    pub fn vertex_cache(&self) -> &SplitVertexCache {
        self.interval.cache.get(&self.index.vertex_index()).unwrap()
    }
    pub fn join_partitions(&mut self) -> LinkedHashMap<PosKey, Split> {
        // insert partitions between all offsets
        let pos_splits = self.vertex_cache().clone();
        let len = pos_splits.len();
        assert!(len > 0);

        let mut iter = pos_splits.iter().map(|(&pos, splits)| VertexSplits {
            pos,
            splits: (splits.borrow() as &TokenTracePositions).clone(),
        });

        let mut prev = iter.next().unwrap();
        let mut partitions = Vec::<Token>::with_capacity(1 + len);
        partitions.push(Prefix::new(&prev).join_partition(self).into());
        for offset in iter {
            partitions
                .push(Infix::new(&prev, &offset).join_partition(self).into());
            prev = offset;
        }
        partitions.push(Postfix::new(prev).join_partition(self).into());
        assert_eq!(
            *self.index.width(),
            partitions.iter().map(|t| *t.width()).sum::<usize>()
        );
        let pos_splits = self.vertex_cache();
        assert_eq!(partitions.len(), pos_splits.len() + 1,);
        NodeMergeCtx::new(self).merge_node(&partitions)
    }
    pub fn join_root_partitions(&mut self) -> Token {
        // Use the new clean implementation
        super::root::join_root_partitions(self)
    }

    // Legacy functions below - to be removed after testing
    #[allow(dead_code)]
    pub fn join_root_partitions_legacy(&mut self) -> Token {
        let root_mode = self.interval.cache.root_mode;
        let offsets = self.vertex_cache().clone();
        let mut offset_iter = offsets.iter().map(PosSplitCtx::from);
        let offset = offset_iter.next().unwrap();

        match root_mode {
            RootMode::Prefix => Prefix::new(offset)
                .join_partition(self)
                .map(|part| self.join_incomplete_prefix(part, offset))
                .unwrap_or_else(|c| c),
            RootMode::Postfix => Postfix::new(offset)
                .join_partition(self)
                .map(|part| self.join_incomplete_postfix(part, offset))
                .unwrap_or_else(|c| c),
            RootMode::Infix => {
                let loffset = offset;
                let roffset = offset_iter.next().unwrap();
                Infix::new(loffset, roffset)
                    .join_partition(self)
                    .map(|part| {
                        self.join_incomplete_infix(part, loffset, roffset)
                    })
                    .unwrap_or_else(|c| c)
            },
        }
    }

    #[allow(dead_code)]
    pub fn join_incomplete_infix<'c>(
        &mut self,
        part: JoinedPartition<In<Join>>,
        loffset: PosSplitCtx<'c>,
        roffset: PosSplitCtx<'c>,
    ) -> Token {
        let loffset = (*loffset.pos, loffset.split.clone());
        let roffset = (*roffset.pos, roffset.split.clone() - part.delta);
        let root_index = self.index;
        if (None, None) == part.perfect.into() {
            // no perfect border
            //        [               ]
            // |     |      |      |     |   |
            let (offset, pre) = match Prefix::new(loffset).join_partition(self)
            {
                Ok(part) =>
                    ((roffset.0, (roffset.1.clone() - part.delta)), part.index),
                Err(ch) => (roffset, ch),
            };
            let post: Token = Postfix::new(offset).join_partition(self).into();
            self.trav.add_pattern_with_update(
                root_index,
                Pattern::from(vec![pre, part.index, post]),
            );
        } else if part.perfect.0 == part.perfect.1 {
            // perfect borders in same pattern
            //       [               ]
            // |     |       |       |      |
            let (ll, rl) = (part.perfect.0.unwrap(), part.perfect.1.unwrap());
            let lpos = loffset.1.pattern_splits[&ll].sub_index();
            let rpos = roffset.1.pattern_splits[&rl].sub_index();
            self.ctx.trav.replace_in_pattern(
                self.index.to_pattern_location(ll),
                lpos..rpos,
                vec![part.index],
            )
        } else {
            // one or both are perfect in different patterns
            let loffset = (loffset.0, &loffset.1);
            let roffset = (roffset.0, &roffset.1);
            if let Some(rp) = part.perfect.1 {
                //           [              ]
                // |     |       |     |    |     |

                let (wrap_offset, li) = {
                    let pre_brds: PartitionBorders<Pre<Join>> =
                        Prefix::new(loffset).partition_borders(self);
                    let rp_brd = &pre_brds.borders[&rp];
                    let li = rp_brd.sub_index;
                    let lc = self.trav.expect_child_at(
                        self.index.to_child_location(SubLocation::new(rp, li)),
                    );
                    let outer_offset = NonZeroUsize::new(
                        rp_brd.start_offset.unwrap().get() + *lc.width(),
                    )
                    .unwrap();
                    (position_splits(self.patterns(), outer_offset), li)
                };
                let ri = roffset.1.pattern_splits[&rp].sub_index();

                //prev_offset.1 = prev_offset.1 - pre.delta;

                let info = Infix::new(&wrap_offset, roffset)
                    .info_partition(self)
                    .unwrap();
                let wrap_patterns =
                    JoinPartitionInfo::from(info).into_joined_patterns(self);
                let wrap_pre = match Infix::new(wrap_offset, loffset)
                    .join_partition(self)
                {
                    Ok(p) => p.index,
                    Err(c) => c,
                };
                let wrapper = self.trav.insert_patterns(
                    std::iter::once(Pattern::from(vec![wrap_pre, part.index]))
                        .chain(wrap_patterns.patterns),
                );
                let loc = self.index.to_pattern_location(rp);
                self.trav.replace_in_pattern(loc, li..ri, vec![wrapper]);

                //let patterns = wrap_patterns.patterns.clone();
                //offset.1 = offset.1 - wrap_patterns.delta;
                //let wrapper = ctx.graph.insert_patterns(
                //    std::iter::once(vec![pre.index, part.index])
                //        .chain(patterns),
                //);

                //let ri = offset.1.pattern_splits[&rp].sub_index;
                //let loc = index.to_pattern_location(rp);
                //ctx.graph.replace_in_pattern(
                //    loc,
                //    0..ri,
                //    [wrapper],
                //);
            }
            if let Some(lp) = part.perfect.0 {
                //       [                 ]
                // |     |       |      |      |   |

                // find wrapping offsets
                let (wrap_offset, ri) = {
                    let post_brds: PartitionBorders<Post<Join>> =
                        Postfix::new(roffset).partition_borders(self);
                    let lp_brd = &post_brds.borders[&lp];
                    let ri = lp_brd.sub_index;
                    let rc = self.trav.expect_child_at(
                        self.index.to_child_location(SubLocation::new(lp, ri)),
                    );
                    let outer_offset = NonZeroUsize::new(
                        lp_brd.start_offset.unwrap().get() + *rc.width(),
                    )
                    .unwrap();
                    (position_splits(self.patterns(), outer_offset), ri)
                };

                let li = loffset.1.pattern_splits[&lp].sub_index();

                let info = Infix::new(loffset, &wrap_offset)
                    .info_partition(self)
                    .unwrap();
                let wrap_patterns =
                    JoinPartitionInfo::from(info).into_joined_patterns(self);
                let wrap_post = match Infix::new(roffset, wrap_offset)
                    .join_partition(self)
                {
                    Ok(p) => p.index,
                    Err(c) => c,
                };

                let wrapper = self.trav.insert_patterns(
                    std::iter::once(Pattern::from(vec![part.index, wrap_post]))
                        .chain(wrap_patterns.patterns),
                );
                let loc = self.index.to_pattern_location(lp);
                self.trav.replace_in_pattern(loc, li..ri + 1, vec![wrapper]);
            }
        }
        part.index
    }

    #[allow(dead_code)]
    pub fn join_incomplete_postfix<'c>(
        &mut self,
        part: JoinedPartition<Post<Join>>,
        offset: PosSplitCtx<'c>,
    ) -> Token {
        let offset_copy = (*offset.pos, offset.split.clone());
        let offset_ref = (offset_copy.0, &offset_copy.1);

        // Get borders in all patterns
        let post_brds: PartitionBorders<Post<Join>> =
            Postfix::new(offset_ref).partition_borders(self);

        // Step 1: Join inner partitions and create working patterns
        // Collect patterns first to avoid borrow checker issues
        let working_patterns: HashMap<PatternId, Pattern> =
            self.patterns().clone();

        for (pid, pattern) in working_patterns.iter() {
            let mut working_pattern = Vec::new();

            // determine inner range for postfix
            let border = post_brds
                .borders
                .get(pid)
                .expect("Pattern must have border");
            let li = border.sub_index;
            let ri = pattern.len();
            let inner_range = li + 1..ri;

            // Check if there are multiple children after the border that form inner partition
            if inner_range.len() >= 2 {
                // Join consecutive children [li+1 .. ri] as inner partition
                let mut inner_children = Vec::new();
                for i in inner_range {
                    inner_children.push(pattern[i]);
                }

                let inner_token =
                    self.trav.insert_pattern(Pattern::from(inner_children));
                working_pattern.push(inner_token);
            }
        }

        // Step 2: Build wrapper for each pattern using working patterns
        for (pid, border) in post_brds.borders.iter() {
            let pattern = &self.patterns()[pid];
            let working_pattern = &working_patterns[pid];

            // Determine wrapper range
            let li = border.sub_index;
            let ri = pattern.len();

            // Get the complement (left part before target)
            let border_child = pattern[li];
            let wrap_pre = if let Some(inner_offset) = border.inner_offset {
                self.ctx
                    .splits
                    .get(&PosKey::new(border_child, inner_offset))
                    .unwrap()
                    .left
            } else {
                border_child
            };

            // Build wrapper patterns
            let mut wrapper_patterns = Vec::new();

            // Primary pattern: [wrap_pre, target]
            wrapper_patterns.push(Pattern::from(vec![wrap_pre, part.index]));

            // Secondary pattern from working pattern subrange
            // Map original pattern indices to working pattern indices
            // (working pattern may be shorter due to joined inner partitions)
            let working_li = li; // For now, assume same index (may need adjustment)
            let working_ri = working_pattern.len();

            if working_li + 1 < working_ri {
                let mut alt_pattern = Vec::new();

                // First child: right half if split
                if let Some(inner_offset) = border.inner_offset {
                    let right_half = self
                        .ctx
                        .splits
                        .get(&PosKey::new(border_child, inner_offset))
                        .unwrap()
                        .right;
                    alt_pattern.push(right_half);
                } else {
                    alt_pattern.push(border_child);
                }

                // Remaining children from working pattern (with inner partitions joined)
                for i in (working_li + 1)..working_ri {
                    alt_pattern.push(working_pattern[i]);
                }

                if alt_pattern.len() > 1 {
                    wrapper_patterns.push(Pattern::from(alt_pattern));
                }
            }

            let wrapper = self.trav.insert_patterns(wrapper_patterns);

            let loc = self.index.to_pattern_location(*pid);
            self.trav.replace_in_pattern(loc, li..ri, vec![wrapper]);
        }

        part.index
    }

    #[allow(dead_code)]
    pub fn join_incomplete_prefix<'c>(
        &mut self,
        part: JoinedPartition<Pre<Join>>,
        offset: PosSplitCtx<'c>,
    ) -> Token {
        let offset_copy = (*offset.pos, offset.split.clone());
        let offset_ref = (offset_copy.0, &offset_copy.1);

        // Get borders for all patterns that contain the prefix
        let pre_brds: PartitionBorders<Pre<Join>> =
            Prefix::new(offset_ref).partition_borders(self);

        // For root prefix, create wrappers for each pattern
        for (pid, border) in pre_brds.borders.iter() {
            // Get the original child pattern from root
            let pattern = &self.patterns()[pid];

            // Determine wrapper range: from start of pattern to border child (inclusive)
            let li = 0; // Start of pattern
            let ri = border.sub_index + 1; // After border child

            // Build wrapper pattern from original root child pattern
            // The wrapper contains: [target, complement]
            // where complement is the part from target end to wrapper end

            // Get the complement (right part of wrapper after target)
            let border_child = pattern[border.sub_index];
            let wrap_post = if let Some(inner_offset) = border.inner_offset {
                // Child is split - get right half from split cache
                self.ctx
                    .splits
                    .get(&PosKey::new(border_child, inner_offset))
                    .unwrap()
                    .right
            } else {
                // Border at child boundary - use entire child
                border_child
            };

            // Build wrapper child patterns from original pattern
            let mut wrapper_patterns = Vec::new();

            // Primary pattern: [target, wrap_post]
            wrapper_patterns.push(Pattern::from(vec![part.index, wrap_post]));

            // Additional patterns from the original root pattern structure
            // For prefix, build pattern from start+middle children
            if ri > 1 {
                // There are more children between wrapper start and end
                let mut alt_pattern = Vec::new();

                // Add children before border child
                for i in 0..(ri - 1) {
                    alt_pattern.push(pattern[i]);
                }

                // Last child: if split, use left half; otherwise use entire child
                if let Some(inner_offset) = border.inner_offset {
                    let left_half = self
                        .ctx
                        .splits
                        .get(&PosKey::new(border_child, inner_offset))
                        .unwrap()
                        .left;
                    alt_pattern.push(left_half);
                } else {
                    alt_pattern.push(border_child);
                }

                if alt_pattern.len() > 1
                    || (alt_pattern.len() == 1 && alt_pattern[0] != part.index)
                {
                    wrapper_patterns.push(Pattern::from(alt_pattern));
                }
            }

            // Create wrapper vertex with all patterns
            let wrapper = self.trav.insert_patterns(wrapper_patterns);

            // Replace the wrapper range in root pattern
            let loc = self.index.to_pattern_location(*pid);
            self.trav.replace_in_pattern(loc, li..ri, vec![wrapper]);
        }

        part.index
    }
}
