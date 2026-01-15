use std::borrow::Borrow;

use derive_new::new;
use linked_hash_map::LinkedHashMap;

use crate::{
    join::{
        context::node::{
            context::NodeJoinCtx,
            merge::{RangeMap, PartitionRange},
        },
    },
    split::{
        Split,
        cache::{
            position::PosKey,
            vertex::SplitVertexCache,
        },
        vertex::TokenTracePositions,
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
            // Ranges now use partition indices: i..(i+1) convention from RangeMap
            // Left partition: from start (0) to current offset (i+1)
            // Right partition: from current offset (i+1) to end (len+1)
            let lr = PartitionRange::new(0..(i + 1));
            let rr = PartitionRange::new((i + 1)..(len + 1));
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

        // Use shared merge logic - merge all partitions (0..num_offsets+1)
        super::shared::merge_partitions_in_range(
            self.ctx,
            offsets,
            partitions,
            0..num_offsets + 1,
            num_offsets,
            &mut range_map,
            Some(self.ctx.index),  // Pass node index for pattern updates
        );

        range_map
    }
}

// RangeMap is now imported from the shared merge::RangeMap module
