//! Root node join implementation.
//!
//! This module implements the unified "smallest-to-largest merge" algorithm
//! for root node joining, supporting Prefix, Postfix, and Infix target partition types.
//!
//! Algorithm follows `NodeMergeCtx::merge_partitions` pattern:
//! 1. Determine valid offset range based on mode (don't include offsets outside wrapper bounds)
//! 2. Iterate by offset COUNT: for len in 1..num_offsets { for start in 0..num_offsets-len+1 }
//! 3. For each partition, use `Infix::info_partition` + `range_sub_merges` pattern
//! 4. Track merged partitions in RangeMap by offset INDEX (not atom position)
//!
//! Key difference from NodeMergeCtx: root has additional partition classifications
//! (Inner, Target, Wrapper) that determine replacement behavior.

use std::{
    borrow::Borrow,
    collections::HashMap,
    ops::Range,
};

use itertools::Itertools;

use crate::{
    TokenTracePositions,
    interval::partition::{
        Infix,
        Postfix,
        Prefix,
        info::{
            InfoPartition,
            PartitionInfo,
            range::{
                role::{
                    In,
                    Post,
                    Pre,
                    RangeRole,
                },
                splits::PostfixRangeFrom,
            },
        },
    },
    join::{
        context::{
            node::context::NodeJoinCtx,
            pattern::borders::JoinBorders,
        },
        joined::patterns::JoinedPatterns,
        partition::{
            Join,
            info::JoinPartitionInfo,
        },
    },
    split::{
        cache::vertex::SplitVertexCache,
        vertex::{
            PosSplitCtx,
            VertexSplits,
            output::RootMode,
        },
    },
};
use context_trace::*;
use tracing::{
    debug,
    info,
    trace,
};

/// RangeMap for tracking merged partitions by offset index range.
/// This mirrors `NodeMergeCtx`'s RangeMap - range is offset INDEX, not atom position.
#[derive(Debug, Default)]
struct RangeMap {
    map: HashMap<Range<usize>, Token>,
}

impl RangeMap {
    /// Initialize with single-element partitions at each offset index.
    /// `partitions` should have num_offsets+1 elements (one per partition between offsets).
    fn from_partitions(partitions: &[Token]) -> Self {
        let mut map = HashMap::default();
        for (i, &part) in partitions.iter().enumerate() {
            map.insert(i..i, part);
        }
        Self { map }
    }

    /// Get all 2-way merge combinations for a range of offset indices.
    /// For range start..end, returns patterns [left, right] for each split point.
    ///
    /// Split points are from start+1 to end-1 (exclusive), because:
    /// - At ri=start, right would be start..end which is what we're building
    /// - At ri=end, left would be start..end which is what we're building
    ///
    /// For adjacent partitions (len=1, e.g. range 0..1), there are no internal
    /// split points, so this returns empty. The merge comes from info_partition.
    fn range_sub_merges(
        &self,
        range: Range<usize>,
    ) -> impl IntoIterator<Item = Pattern> + '_ {
        let (start, end) = (range.start, range.end);
        // Iterate over split points start+1..end (not including start or end)
        (start + 1..end).map(move |ri| {
            let &left = self.map.get(&(start..ri)).unwrap();
            let &right = self.map.get(&(ri..end)).unwrap();
            Pattern::from(vec![left, right])
        })
    }

    fn insert(
        &mut self,
        range: Range<usize>,
        token: Token,
    ) {
        self.map.insert(range, token);
    }

    fn get(
        &self,
        range: &Range<usize>,
    ) -> Option<&Token> {
        self.map.get(range)
    }
}

/// Classification of a partition's role during merge
#[derive(Debug, Clone)]
enum PartitionType {
    /// Inner partition - inside target, belongs to exactly one pattern  
    Inner { owner_pattern: PatternId },
    /// Target partition - the partition being inserted
    Target,
    /// Wrapper partition - contains target, belongs to exactly one pattern
    Wrapper { owner_pattern: PatternId },
    /// Intermediate partition - building block, no special handling
    Intermediate,
}

/// Main entry point for root node joining
///
/// This function joins partitions at the root level based on the root mode
/// (Prefix, Postfix, or Infix). It generalizes `NodeMergeCtx::merge_partitions`
/// with additional partition classification for replacements.
pub fn join_root_partitions(ctx: &mut NodeJoinCtx) -> Token {
    let root_mode = ctx.ctx.interval.cache.root_mode;
    let offsets = ctx.vertex_cache().clone();
    let root_width = *ctx.index.width();

    info!(
        ?root_mode,
        num_offsets = offsets.len(),
        root_index = ?ctx.index,
        root_width,
        "Starting root join with unified algorithm"
    );

    // Determine the target offset range based on mode
    let target_offset_range = get_target_offset_range(&offsets, root_mode);

    info!(?target_offset_range, "Target offset range determined");

    // Run the merge algorithm following NodeMergeCtx pattern
    merge_root_partitions(ctx, &offsets, target_offset_range, root_mode)
}

/// Determine the target offset range based on root mode.
///
/// Returns Range<usize> of offset indices that form the target partition.
/// The range represents which partition "slot" is the target:
/// - For n offsets, there are n+1 partitions (slots indexed 0 to n)
/// - Partition slot i spans from offset i-1 to offset i (or root boundaries)
/// - Empty range i..i means partition at slot i (a single initial partition, not merged)
///
/// Understanding RootMode semantics:
/// - Prefix: inserted thing is a PREFIX of the root → target is LEFT side (before offsets)
/// - Postfix: inserted thing is a POSTFIX of the root → target is RIGHT side (after offsets)
/// - Infix: inserted thing is in the MIDDLE → target is between first two offsets
fn get_target_offset_range(
    offsets: &SplitVertexCache,
    root_mode: RootMode,
) -> Range<usize> {
    let num_offsets = offsets.len();

    match root_mode {
        RootMode::Prefix => {
            // Prefix: inserted thing is a prefix of root
            // Target is BEFORE all offsets (partition slot 0)
            // Using 0..0 to represent "slot 0, no merge needed if single offset"
            0..0
        },
        RootMode::Postfix => {
            // Postfix: inserted thing is a postfix of root
            // Target is AFTER all offsets (partition slot n)
            // Using num_offsets..num_offsets to represent "slot n, no merge needed"
            num_offsets..num_offsets
        },
        RootMode::Infix => {
            // Infix: inserted thing is in the middle
            // Target is between first two offsets (partitions 0 and 1 merged)
            0..1
        },
    }
}

/// Merge root partitions following the NodeMergeCtx pattern.
///
/// Iterates by offset COUNT: for len in 1..num_offsets { for start in 0..num_offsets-len+1 }
fn merge_root_partitions(
    ctx: &mut NodeJoinCtx,
    offsets: &SplitVertexCache,
    target_offset_range: Range<usize>,
    root_mode: RootMode,
) -> Token {
    let num_offsets = offsets.len();
    let root_index = ctx.index;
    let root_width = *ctx.index.width();

    // Get initial partitions from root patterns
    let initial_partitions = get_initial_partitions(ctx, offsets);

    debug!(
        ?initial_partitions,
        num_partitions = initial_partitions.len(),
        "Initial partitions"
    );

    // Ensure we have the right number of partitions (num_offsets + 1)
    assert_eq!(
        initial_partitions.len(),
        num_offsets + 1,
        "Expected {} partitions for {} offsets, got {}",
        num_offsets + 1,
        num_offsets,
        initial_partitions.len()
    );

    let mut range_map = RangeMap::from_partitions(&initial_partitions);

    // Calculate inner and wrapper bounds for partition classification
    let (inner_bounds, wrapper_bounds) =
        calculate_bounds(ctx, offsets, root_mode, root_width);

    debug!(
        ?inner_bounds,
        ?wrapper_bounds,
        "Calculated pattern bounds (offset indices)"
    );

    let mut target_token: Option<Token> = None;

    // For single-offset cases (Prefix/Postfix), the target is one of the initial partitions
    // and no merging is needed. Extract it directly.
    if target_offset_range.is_empty() {
        // Empty range like 0..0 or n..n means target is initial partition at that index
        let target_idx = target_offset_range.start;
        target_token = Some(initial_partitions[target_idx]);
        debug!(
            target_idx,
            ?target_token,
            "Target is initial partition (no merge needed)"
        );
    }

    // Merge from smallest to largest (by offset count, not atom size)
    // Note: len is number of offsets SPANNED, not number of children
    // The loop goes from 1 to num_offsets-1 (exclusive) because:
    // - len=1 merges partitions spanning 1 offset
    // - len=num_offsets-1 is the largest merge before the full partition
    // - The full partition (spanning all offsets) is handled separately after the loop
    //
    // The inner loop bound is num_offsets - len (not +1) because:
    // - For range start..start+len, we need offsets at indices start and start+len
    // - The maximum valid index is num_offsets-1
    // - So start+len <= num_offsets-1, meaning start <= num_offsets-1-len < num_offsets-len
    for len in 1..num_offsets {
        for start in 0..num_offsets - len {
            let range = start..start + len;

            // Skip if this is a single partition (already in range_map from initialization)
            if len == 0 {
                continue;
            }

            // Get the left and right offset contexts
            let lo = offsets.iter().map(PosSplitCtx::from).nth(start).unwrap();
            let ro = offsets
                .iter()
                .map(PosSplitCtx::from)
                .nth(start + len)
                .unwrap();

            trace!(?range, lo_pos = ?lo.pos, ro_pos = ?ro.pos, "Processing partition range");

            // Use Infix::info_partition following NodeMergeCtx pattern
            let infix = Infix::new(lo, ro);
            let res: Result<PartitionInfo<In<Join>>, _> =
                infix.info_partition(ctx);

            let index = match res {
                Ok(info) => {
                    // Get all 2-way merge combinations for this range
                    let merges = range_map.range_sub_merges(range.clone());

                    // Get patterns from info (perfect boundaries)
                    let joined =
                        info.patterns.into_iter().map(|(pid, pinfo)| {
                            Pattern::from(
                                (pinfo.join_pattern(ctx, &pid).borrow()
                                    as &'_ Pattern)
                                    .iter()
                                    .cloned()
                                    .collect_vec(),
                            )
                        });

                    let patterns: Vec<Pattern> =
                        merges.into_iter().chain(joined).collect_vec();
                    ctx.trav.insert_patterns(patterns)
                },
                Err(existing) => existing,
            };

            range_map.insert(range.clone(), index);
            debug!(?index, ?range, "Merged partition");

            // Classify and handle this partition
            let partition_type = classify_partition(
                &range,
                &target_offset_range,
                &inner_bounds,
                &wrapper_bounds,
            );

            match partition_type {
                PartitionType::Inner { owner_pattern } => {
                    debug!(
                        ?range,
                        ?owner_pattern,
                        "Inner partition - replacing in pattern"
                    );
                    replace_in_pattern(
                        ctx,
                        owner_pattern,
                        index,
                        &range,
                        offsets,
                    );
                },
                PartitionType::Target => {
                    info!(?range, ?index, "Target partition found");
                    target_token = Some(index);
                },
                PartitionType::Wrapper { owner_pattern } => {
                    debug!(
                        ?range,
                        ?owner_pattern,
                        "Wrapper partition - replacing in pattern"
                    );

                    // Add complement patterns if needed
                    let final_token = add_wrapper_complement_patterns(
                        ctx,
                        index,
                        &range,
                        &target_offset_range,
                        target_token,
                        &range_map,
                    );

                    replace_in_pattern(
                        ctx,
                        owner_pattern,
                        final_token,
                        &range,
                        offsets,
                    );
                },
                PartitionType::Intermediate => {
                    trace!(?range, "Intermediate partition - cached only");
                },
            }
        }
    }

    // The full partition spanning all offsets replaces the root's children
    let full_range = 0..num_offsets;
    if let Some(&final_token) = range_map.get(&full_range) {
        // Replace children in all root patterns with the final merged result
        for (pattern_id, pat) in ctx.patterns().clone().iter() {
            let loc = root_index.to_pattern_location(*pattern_id);
            ctx.trav.replace_in_pattern(
                loc,
                PostfixRangeFrom::from(0..pat.len()),
                vec![final_token],
            );
        }
    }

    target_token.expect("Target token should have been created during merge")
}

/// Get initial partitions from root node using Prefix/Infix/Postfix join.
///
/// Returns a vec of tokens, one for each partition slot.
/// With n offsets, there are n+1 partition slots.
///
/// This mirrors the non-root `join_node_partitions` approach:
/// 1. Prefix for partition before first offset
/// 2. Infix for each partition between consecutive offsets
/// 3. Postfix for partition after last offset
///
/// IMPORTANT: Unlike regular join, we do NOT replace in patterns here.
/// The initial partitions are building blocks for the merge algorithm.
/// Replacements happen later when wrapper partitions are created.
fn get_initial_partitions(
    ctx: &mut NodeJoinCtx,
    offsets: &SplitVertexCache,
) -> Vec<Token> {
    let num_offsets = offsets.len();
    let mut partitions = Vec::<Token>::with_capacity(num_offsets + 1);

    // Create VertexSplits iterator from offsets
    let mut iter = offsets.iter().map(|(&pos, splits)| VertexSplits {
        pos,
        splits: (splits.borrow() as &TokenTracePositions).clone(),
    });

    // First partition: everything before first offset (Prefix)
    let first = iter.next().expect("Root must have at least one offset");
    let prefix_token =
        join_partition_without_replace::<Pre<Join>>(ctx, Prefix::new(&first));
    partitions.push(prefix_token);

    // Middle partitions: between consecutive offsets (Infix)
    let mut prev = first;
    for offset in iter {
        let infix_token = join_partition_without_replace::<In<Join>>(
            ctx,
            Infix::new(&prev, &offset),
        );
        partitions.push(infix_token);
        prev = offset;
    }

    // Last partition: everything after last offset (Postfix)
    let postfix_token =
        join_partition_without_replace::<Post<Join>>(ctx, Postfix::new(prev));
    partitions.push(postfix_token);

    debug!(
        ?partitions,
        num_partitions = partitions.len(),
        "Created initial partitions using Prefix/Infix/Postfix"
    );

    partitions
}

/// Join a partition WITHOUT doing the replace_in_pattern step.
/// This is used for initial partitions in root join where we don't want
/// to modify the root pattern until the full merge is complete.
fn join_partition_without_replace<'a, R>(
    ctx: &mut NodeJoinCtx<'a>,
    partition: impl InfoPartition<R>,
) -> Token
where
    R: RangeRole<Mode = Join> + 'a,
    R::Borders: JoinBorders<R>,
{
    match partition.info_partition(ctx) {
        Ok(info) => {
            let pats = JoinedPatterns::from_partition_info(
                JoinPartitionInfo::new(info),
                ctx,
            );
            // Just insert patterns, DON'T replace
            ctx.trav.insert_patterns(pats.patterns)
        },
        Err(existing) => existing,
    }
}

/// Calculate inner and wrapper bounds for each pattern.
///
/// Inner bounds: offset index range that is completely inside target with perfect boundaries
/// Wrapper bounds: offset index range that contains target with perfect boundaries
///
/// Returns (inner_bounds, wrapper_bounds) where each is a map from PatternId to Range<usize>
fn calculate_bounds(
    ctx: &NodeJoinCtx,
    offsets: &SplitVertexCache,
    root_mode: RootMode,
    root_width: usize,
) -> (
    HashMap<PatternId, Range<usize>>,
    HashMap<PatternId, Range<usize>>,
) {
    let mut inner_bounds = HashMap::new();
    let mut wrapper_bounds = HashMap::new();

    // Get offset positions in atom space
    let offset_positions: Vec<usize> =
        offsets.positions.keys().map(|p| p.get()).collect();

    for (pattern_id, pattern) in ctx.patterns().iter() {
        // Get pattern boundaries (cumulative widths) in atom space
        let boundaries = get_pattern_boundaries(pattern);

        debug!(?pattern_id, ?boundaries, "Pattern boundaries");

        // For each pattern, find which offset indices correspond to perfect boundaries
        // and compute inner/wrapper bounds in offset index space

        // Map atom positions to offset indices
        let boundary_offset_indices: Vec<usize> = boundaries
            .iter()
            .filter_map(|&b| {
                if b == 0 {
                    Some(0) // Start of root
                } else if b == root_width {
                    Some(offset_positions.len()) // End of root
                } else {
                    // Find which offset index this boundary corresponds to
                    offset_positions.iter().position(|&p| p == b)
                }
            })
            .collect();

        debug!(
            ?pattern_id,
            ?boundary_offset_indices,
            "Boundary offset indices"
        );

        // Inner bounds: largest contiguous range of offset indices with perfect boundaries
        // that is strictly inside the target
        // (For now, we'll compute this based on the mode)

        // Wrapper bounds: smallest range of offset indices with perfect boundaries
        // that contains the target
        match root_mode {
            RootMode::Prefix => {
                // Target is BEFORE all offsets (left side, slot 0)
                // Wrapper extends from start (0) to some perfect boundary after target
                // Find the smallest perfect boundary index > 0
                if let Some(&first_boundary_idx) =
                    boundary_offset_indices.iter().filter(|&&i| i > 0).min()
                {
                    // Wrapper from 0 to first_boundary_idx
                    wrapper_bounds.insert(*pattern_id, 0..first_boundary_idx);
                }
            },
            RootMode::Postfix => {
                // Target is AFTER all offsets (right side, slot n)
                // Wrapper extends from some perfect boundary to end
                // Find the largest perfect boundary index < end
                if let Some(&last_boundary_idx) = boundary_offset_indices
                    .iter()
                    .filter(|&&i| i < offset_positions.len())
                    .max()
                {
                    // Wrapper from last_boundary_idx to end
                    wrapper_bounds.insert(
                        *pattern_id,
                        last_boundary_idx..offset_positions.len(),
                    );
                }
            },
            RootMode::Infix => {
                // Target is between first two offsets (0..1)
                // Inner: perfect boundaries strictly inside target
                // Wrapper: perfect boundaries containing target

                // Find wrapper bounds
                let left_wrapper = boundary_offset_indices
                    .iter()
                    .filter(|&&i| i == 0)
                    .max()
                    .copied()
                    .unwrap_or(0);
                let right_wrapper = boundary_offset_indices
                    .iter()
                    .filter(|&&i| i >= 1)
                    .min()
                    .copied()
                    .unwrap_or(offset_positions.len());

                if left_wrapper < right_wrapper {
                    wrapper_bounds
                        .insert(*pattern_id, left_wrapper..right_wrapper);
                }
            },
        }
    }

    (inner_bounds, wrapper_bounds)
}

/// Get all perfect boundaries (cumulative offsets) for a pattern
fn get_pattern_boundaries(pattern: &Pattern) -> Vec<usize> {
    let mut boundaries = vec![0]; // Start is always a boundary
    let mut cumulative = 0;
    for child in pattern.iter() {
        cumulative += *child.width();
        boundaries.push(cumulative);
    }
    boundaries
}

/// Classify a partition based on its offset index range.
fn classify_partition(
    range: &Range<usize>,
    target_offset_range: &Range<usize>,
    inner_bounds: &HashMap<PatternId, Range<usize>>,
    wrapper_bounds: &HashMap<PatternId, Range<usize>>,
) -> PartitionType {
    // Check if this is the target partition
    if range == target_offset_range {
        return PartitionType::Target;
    }

    // Check if this is an inner partition
    for (pattern_id, inner_range) in inner_bounds.iter() {
        if range.start >= inner_range.start && range.end <= inner_range.end {
            return PartitionType::Inner {
                owner_pattern: *pattern_id,
            };
        }
    }

    // Check if this is a wrapper partition
    for (pattern_id, wrapper_range) in wrapper_bounds.iter() {
        if range == wrapper_range {
            return PartitionType::Wrapper {
                owner_pattern: *pattern_id,
            };
        }
    }

    PartitionType::Intermediate
}

/// Add complement patterns to a wrapper if needed.
///
/// When a wrapper contains a target that doesn't align with its boundaries,
/// we need to add patterns that combine the split complement with the target.
fn add_wrapper_complement_patterns(
    ctx: &mut NodeJoinCtx,
    wrapper_token: Token,
    wrapper_range: &Range<usize>,
    target_offset_range: &Range<usize>,
    target_token: Option<Token>,
    range_map: &RangeMap,
) -> Token {
    let target_token = match target_token {
        Some(t) => t,
        None => return wrapper_token,
    };

    // Check if wrapper contains target
    if !(wrapper_range.start <= target_offset_range.start
        && target_offset_range.end <= wrapper_range.end)
    {
        return wrapper_token;
    }

    let mut complement_patterns: Vec<Pattern> = Vec::new();

    // Left complement: partition from wrapper_start to target_start
    if wrapper_range.start < target_offset_range.start {
        let left_range = wrapper_range.start..target_offset_range.start;
        if let Some(&left_token) = range_map.get(&left_range) {
            complement_patterns
                .push(Pattern::from(vec![left_token, target_token]));
            debug!(
                ?left_token,
                ?target_token,
                "Adding left complement pattern"
            );
        }
    }

    // Right complement: partition from target_end to wrapper_end
    if target_offset_range.end < wrapper_range.end {
        let right_range = target_offset_range.end..wrapper_range.end;
        if let Some(&right_token) = range_map.get(&right_range) {
            complement_patterns
                .push(Pattern::from(vec![target_token, right_token]));
            debug!(
                ?target_token,
                ?right_token,
                "Adding right complement pattern"
            );
        }
    }

    if complement_patterns.is_empty() {
        return wrapper_token;
    }

    ctx.trav
        .add_patterns_with_update(wrapper_token, complement_patterns);
    wrapper_token
}

/// Replace children in a pattern with a merged partition token.
fn replace_in_pattern(
    ctx: &mut NodeJoinCtx,
    pattern_id: PatternId,
    token: Token,
    range: &Range<usize>,
    offsets: &SplitVertexCache,
) {
    let root_index = ctx.index;
    let pattern = match ctx.patterns().get(&pattern_id) {
        Some(p) => p.clone(),
        None => return,
    };

    // Convert offset index range to child index range
    let offset_positions: Vec<usize> =
        offsets.positions.keys().map(|p| p.get()).collect();

    // Get atom positions for this range
    let start_atom = if range.start == 0 {
        0
    } else {
        offset_positions[range.start - 1]
    };
    let end_atom = if range.end >= offset_positions.len() {
        *ctx.index.width()
    } else {
        offset_positions[range.end]
    };

    // Find child indices that match these atom positions
    if let Some((start_idx, end_idx)) =
        get_spanning_child_range(&pattern, start_atom, end_atom)
    {
        let loc = root_index.to_pattern_location(pattern_id);
        debug!(
            ?pattern_id,
            start_idx,
            end_idx,
            ?token,
            start_atom,
            end_atom,
            "Replacing children in pattern"
        );
        ctx.trav
            .replace_in_pattern(loc, start_idx..end_idx, vec![token]);
    }
}

/// Get the child index range that a partition spans in a given pattern.
fn get_spanning_child_range(
    pattern: &Pattern,
    start_atom: usize,
    end_atom: usize,
) -> Option<(usize, usize)> {
    let boundaries = get_pattern_boundaries(pattern);

    // Find start index: first boundary >= start_atom
    let start_idx = boundaries.iter().position(|&b| b == start_atom)?;

    // Find end index: first boundary == end_atom
    let end_idx = boundaries.iter().position(|&b| b == end_atom)?;

    if end_idx > start_idx {
        Some((start_idx, end_idx))
    } else {
        None
    }
}
