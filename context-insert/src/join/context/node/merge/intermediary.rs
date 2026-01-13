use std::borrow::Borrow;

use derive_new::new;
use itertools::Itertools;
use linked_hash_map::LinkedHashMap;

use crate::{
    interval::partition::{
        Infix,
        info::{
            InfoPartition,
            PartitionInfo,
            range::role::In,
        },
    },
    join::{
        context::node::{
            context::NodeJoinCtx,
            merge::RangeMap,
        },
        partition::Join,
    },
    split::{
        Split,
        cache::{
            position::PosKey,
            vertex::SplitVertexCache,
        },
        vertex::{
            PosSplitCtx,
            TokenTracePositions,
        },
    },
};
use context_trace::*;

#[derive(Debug, new)]
pub struct NodeMergeCtx<'a: 'b, 'b> {
    pub ctx: &'b mut NodeJoinCtx<'a>,
}

impl<'a: 'b, 'b: 'c, 'c> NodeMergeCtx<'a, 'b> {
    pub fn merge_node(
        &'c mut self,
        partitions: &Vec<Token>,
    ) -> LinkedHashMap<PosKey, Split> {
        let offsets = self.ctx.vertex_cache().clone();
        assert_eq!(partitions.len(), offsets.len() + 1);

        let merges = self.merge_partitions(&offsets, partitions);

        let len = offsets.len();
        let index = self.ctx.index;
        let mut finals = LinkedHashMap::new();
        for (i, (offset, v)) in offsets.iter().enumerate() {
            let lr = 0..i;
            let rr = i + 1..len;
            let left = *merges.get(&lr).unwrap();
            let right = *merges.get(&rr).unwrap();
            if !lr.is_empty() || !lr.is_empty() {
                if let Some((&pid, _)) = (v.borrow() as &TokenTracePositions)
                    .iter()
                    .find(|(_, s)| s.inner_offset.is_none())
                {
                    self.ctx.trav.replace_pattern(
                        index.to_pattern_location(pid),
                        vec![left, right],
                    );
                } else {
                    self.ctx.trav.add_pattern_with_update(
                        index,
                        Pattern::from(vec![left, right]),
                    );
                }
            }
            finals.insert(PosKey::new(index, *offset), Split::new(left, right));
        }
        finals
    }
    pub fn merge_partitions(
        &mut self,
        offsets: &SplitVertexCache,
        partitions: &Vec<Token>,
    ) -> RangeMap {
        let num_offsets = offsets.positions.len();

        let mut range_map = RangeMap::from(partitions);

        for len in 1..num_offsets {
            for start in 0..num_offsets - len + 1 {
                let range = start..start + len;

                let lo =
                    offsets.iter().map(PosSplitCtx::from).nth(start).unwrap();
                let ro = offsets
                    .iter()
                    .map(PosSplitCtx::from)
                    .nth(start + len)
                    .unwrap();

                // todo: could be read from cache
                let infix = Infix::new(lo, ro);
                let res: Result<PartitionInfo<In<Join>>, _> =
                    infix.info_partition(self.ctx);

                let index = match res {
                    Ok(info) => {
                        let merges =
                            range_map.range_sub_merges(start..start + len);
                        let joined =
                            info.patterns.into_iter().map(|(pid, info)| {
                                Pattern::from(
                                    (info.join_pattern(self.ctx, &pid).borrow()
                                        as &'_ Pattern)
                                        .iter()
                                        .cloned()
                                        .collect_vec(),
                                )
                            });
                        // todo: insert into perfect context
                        let patterns =
                            merges.into_iter().chain(joined).collect_vec();
                        self.ctx.trav.insert_patterns(patterns)
                    },
                    Err(c) => c,
                };
                range_map.insert(range, index);
            }
        }
        range_map
    }
}

// RangeMap is now imported from the shared merge::RangeMap module
