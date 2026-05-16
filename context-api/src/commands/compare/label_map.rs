use std::collections::{
    BTreeSet,
    HashMap,
};

use context_trace::graph::snapshot::GraphSnapshot;

/// Per-label entry derived from a `GraphSnapshot`.
#[derive(Debug, Clone)]
pub(super) struct LabelEntry {
    pub(super) width: usize,
    /// Sorted set of child patterns; each pattern is an ordered child-label
    /// sequence. The outer `BTreeSet` provides order-insensitive pattern-set
    /// comparison while preserving the order within each pattern.
    pub(super) patterns: BTreeSet<Vec<String>>,
    pub(super) is_atom: bool,
}

/// Label-indexed view of a `GraphSnapshot`.
pub(super) type LabelMap = HashMap<String, LabelEntry>;

/// Build a `LabelMap` from a `GraphSnapshot`.
///
/// Steps:
/// 1. Build `index → label` from `snapshot.nodes`.
/// 2. Seed every node as a `LabelEntry` (patterns initially empty).
/// 3. Group `snapshot.edges` by `(from, pattern_idx)`, sort children by
///    `sub_index`, resolve indices to labels, and insert each resulting
///    `Vec<String>` into the parent's pattern set.
///
/// The algorithm is intentionally identical to the one in
/// `tests/common/graph_compare.rs` so the two can serve as independent
/// cross-checks for correctness.
pub(super) fn build_label_map(snap: &GraphSnapshot) -> LabelMap {
    let index_to_label: HashMap<usize, String> = snap
        .nodes
        .iter()
        .map(|n| (n.index, n.label.clone()))
        .collect();

    let mut map: LabelMap = HashMap::new();
    for node in &snap.nodes {
        map.entry(node.label.clone()).or_insert_with(|| LabelEntry {
            width: node.width,
            patterns: BTreeSet::new(),
            is_atom: node.width == 1,
        });
    }

    let mut by_parent_pattern: HashMap<(usize, usize), Vec<(usize, usize)>> =
        HashMap::new();
    for edge in &snap.edges {
        by_parent_pattern
            .entry((edge.from, edge.pattern_idx))
            .or_default()
            .push((edge.sub_index, edge.to));
    }

    for ((parent_idx, _pat_idx), mut children) in by_parent_pattern {
        let parent_label = match index_to_label.get(&parent_idx) {
            Some(label) => label.clone(),
            None => continue,
        };
        children.sort_by_key(|(sub_idx, _)| *sub_idx);
        let pattern: Vec<String> = children
            .iter()
            .filter_map(|(_, child_idx)| index_to_label.get(child_idx))
            .cloned()
            .collect();
        if pattern.is_empty() {
            continue;
        }
        if let Some(entry) = map.get_mut(&parent_label) {
            entry.patterns.insert(pattern);
        }
    }

    map
}