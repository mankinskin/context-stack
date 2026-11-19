use context_trace::legacy::Wide;
use linked_hash_map::LinkedHashMap;
use tracing::info;

use crate::{
    join::{
        context::node::merge::{
            PartitionRange,
            context::MergeCtx,
        },
        plan::{
            HostInterval,
            JoinRef,
            RefPosKey,
        },
    },
    split::Split,
};

impl<'a> MergeCtx<'a> {
    /// Merge an intermediary node, returning splits for each offset.
    ///
    /// This uses the same merge algorithm as root nodes - `merge_sub_partitions`
    /// proposes all candidates and pattern edits. This function just extracts
    /// the Split results from the range_map for use by parent nodes.
    ///
    /// Splits for merged tokens are added to the shared splits map during merging,
    /// so they're available for subsequent pattern operations.
    pub(crate) fn merge_node(
        &mut self
    ) -> LinkedHashMap<RefPosKey, Split<JoinRef>> {
        // Merge all sub-partitions - this proposes all pattern creation
        // and adds splits for merged tokens to the shared map
        let result = self.merge_sub_partitions(None);

        // Extract splits for each offset from the range_map
        let len = self.offsets.len();
        let index = self.ctx.index;

        // Collect offsets first to avoid borrow issues
        let offsets: Vec<_> = self
            .offsets
            .iter()
            .enumerate()
            .map(|(i, (offset, _))| (i, *offset))
            .collect();

        let mut split_result = LinkedHashMap::new();
        let merges = result.range_map;

        for (range, &token) in &merges.map {
            let start = if *range.start() == 0 {
                0
            } else {
                offsets[*range.start() - 1].1.get()
            };
            let end = if *range.end() == len {
                *index.width
            } else {
                offsets[*range.end()].1.get()
            };
            assert_eq!(
                *token.width(),
                end - start,
                "Materialized interval width mismatch for {index:?} at {start}..{end}"
            );
            self.ctx
                .intervals
                .insert((JoinRef::Existing(index), start, end), token);
            self.ctx.ctx.plan.record_interval(
                HostInterval {
                    host: index,
                    start,
                    end,
                },
                token,
            );
        }

        for (i, offset) in offsets {
            // Left partition: from start (0) to current offset position (i)
            // Right partition: from after current offset (i+1) to end (len)
            let lr = PartitionRange::new(0..=i);
            let rr = PartitionRange::new((i + 1)..=len);
            let left = *merges.get(&lr).unwrap();
            let right = *merges.get(&rr).unwrap();
            let key = RefPosKey::new(JoinRef::Existing(index), offset);
            let split = Split::new(left, right);
            // Add to shared map so parent nodes can access
            self.add_split(key, split.clone());
            // Also collect for return
            split_result.insert(key, split);
        }

        split_result
    }

    /// Merge the root node, returning the planned target reference.
    ///
    /// Uses the same merge algorithm as intermediary nodes, but with a
    /// target_range that identifies which partition range contains the
    /// token being inserted.
    ///
    /// After merging, proposes a new root pattern that includes the target
    /// token (for Prefix, Postfix, and Infix modes).
    pub(crate) fn merge_root(&mut self) -> JoinRef {
        // Use the target range computed during split phase
        let target_range = self.ctx.ctx.interval.target_range.clone();
        info!("Starting root merge join");

        let result = self.merge_sub_partitions(Some(target_range.clone()));
        let target_token = result.target_token;
        let range_map = result.range_map;
        let had_perfect_replacement = result.had_perfect_replacement;

        info!(?target_token, "Target token extracted from range_map");

        // Propose a root pattern that includes the target token.
        // SKIP if a perfect replacement was already proposed for the pattern
        if !had_perfect_replacement {
            self.add_root_pattern(&range_map, target_token);
        } else {
            info!(
                "Skipping add_root_pattern - perfect replacement already proposed"
            );
        }

        info!(
            root_overlay = ?self.ctx.overlay,
            ?target_token,
            "Root join planned - returning target reference"
        );

        target_token
    }
}
