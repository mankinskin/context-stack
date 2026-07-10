//! Graph comparison commands — Phase 3.2
//!
//! Implements `compare_workspaces` and `compare_vertices` on
//! `WorkspaceManager`.
//!
//! Both are pure reads: they acquire `GraphSnapshot`s, run the label-indexed
//! diff algorithm, and return a `GraphDiffResult`.  No workspace state is
//! modified; no cross-workspace locks are held during computation
//! (Design Decision D24).
//!
//! # Algorithm
//!
//! 1. Build a `LabelMap` (`HashMap<label, LabelEntry{width, patterns, is_atom}>`)
//!    for each snapshot.
//! 2. Partition labels into `shared`, `only_in_a`, `only_in_b`.
//! 3. For each shared label classify the pair as `Identical`, `WidthMismatch`,
//!    `ExtraPatterns`, or `PatternMismatch`.
//! 4. Compute a `PatternDiff` for labels that differ in their pattern sets.
//! 5. Derive the overall `DiffVerdict` from the counts and the requested mode.
//!
//! See `20260315_PLAN_GRAPH_DIFF_COMMAND.md` §Comparison Algorithm for the
//! detailed rationale (Design Decisions D17, D21–D24).

mod label_map;
#[cfg(test)]
mod tests;

use std::collections::HashSet;

use context_trace::graph::snapshot::{
    GraphSnapshot,
    SnapshotEdge,
    SnapshotNode,
};

use self::label_map::build_label_map;

use crate::{
    error::CompareError,
    types::{
        CompareMode,
        DiffSummary,
        DiffVerdict,
        DiffVertexEntry,
        GraphDiffResult,
        PatternDiff,
        SharedVertex,
        VertexMatchKind,
    },
    workspace::manager::WorkspaceManager,
};

fn diff_vertex_entries(
    source: &label_map::LabelMap,
    source_keys: &HashSet<&String>,
    other_keys: &HashSet<&String>,
) -> Vec<DiffVertexEntry> {
    let mut entries: Vec<_> = source_keys
        .difference(other_keys)
        .map(|label| {
            let entry = &source[*label];
            DiffVertexEntry {
                label: (*label).clone(),
                width: entry.width,
                patterns: entry.patterns.iter().cloned().collect(),
                is_atom: entry.is_atom,
            }
        })
        .collect();
    entries.sort_by(|a, b| a.label.cmp(&b.label));
    entries
}

fn compare_shared_vertices(
    shared_labels: &[&&String],
    map_a: &label_map::LabelMap,
    map_b: &label_map::LabelMap,
) -> (Vec<SharedVertex>, usize, usize) {
    let mut shared = Vec::new();
    let mut pattern_mismatch_count = 0usize;
    let mut width_mismatch_count = 0usize;

    for label in shared_labels {
        let entry_a = &map_a[**label];
        let entry_b = &map_b[**label];

        if entry_a.width != entry_b.width {
            width_mismatch_count += 1;
            shared.push(SharedVertex {
                label: (*label).to_string(),
                width: entry_a.width,
                match_kind: VertexMatchKind::WidthMismatch {
                    width_a: entry_a.width,
                    width_b: entry_b.width,
                },
                pattern_diff: None,
            });
            continue;
        }

        let common: Vec<Vec<String>> = entry_a
            .patterns
            .intersection(&entry_b.patterns)
            .cloned()
            .collect();
        let only_a_pats: Vec<Vec<String>> = entry_a
            .patterns
            .difference(&entry_b.patterns)
            .cloned()
            .collect();
        let only_b_pats: Vec<Vec<String>> = entry_b
            .patterns
            .difference(&entry_a.patterns)
            .cloned()
            .collect();

        let match_kind = if entry_a.patterns == entry_b.patterns {
            VertexMatchKind::Identical
        } else if entry_b.patterns.is_subset(&entry_a.patterns) {
            VertexMatchKind::Identical
        } else if !common.is_empty() {
            VertexMatchKind::ExtraPatterns
        } else {
            pattern_mismatch_count += 1;
            VertexMatchKind::PatternMismatch
        };

        let pattern_diff = match match_kind {
            VertexMatchKind::ExtraPatterns
            | VertexMatchKind::PatternMismatch => Some(PatternDiff {
                only_in_a: only_a_pats,
                only_in_b: only_b_pats,
                common,
            }),
            _ => None,
        };

        shared.push(SharedVertex {
            label: (*label).to_string(),
            width: entry_a.width,
            match_kind,
            pattern_diff,
        });
    }

    (shared, pattern_mismatch_count, width_mismatch_count)
}

fn compute_verdict(
    mode: &CompareMode,
    only_in_a: &[DiffVertexEntry],
    only_in_b: &[DiffVertexEntry],
    shared: &[SharedVertex],
    pattern_mismatch_count: usize,
    width_mismatch_count: usize,
) -> DiffVerdict {
    match mode {
        CompareMode::Full => {
            if only_in_a.is_empty()
                && only_in_b.is_empty()
                && width_mismatch_count == 0
                && pattern_mismatch_count == 0
            {
                DiffVerdict::Equivalent
            } else {
                DiffVerdict::Divergent
            }
        },
        CompareMode::Subset => {
            let extra_patterns_in_b = shared.iter().any(|vertex| {
                vertex.match_kind == VertexMatchKind::ExtraPatterns
            });
            let b_is_subset = only_in_b.is_empty()
                && width_mismatch_count == 0
                && pattern_mismatch_count == 0
                && !extra_patterns_in_b;
            if b_is_subset {
                DiffVerdict::Subset
            } else {
                DiffVerdict::Divergent
            }
        },
    }
}

// ---------------------------------------------------------------------------
// Core comparison algorithm (pure function)
// ---------------------------------------------------------------------------

/// Compare two `GraphSnapshot`s and produce a `GraphDiffResult`.
///
/// This is a pure function — no I/O, no workspace state mutation.
pub(crate) fn compare_snapshots(
    snap_a: &GraphSnapshot,
    workspace_a: &str,
    snap_b: &GraphSnapshot,
    workspace_b: &str,
    mode: CompareMode,
) -> GraphDiffResult {
    let map_a = build_label_map(snap_a);
    let map_b = build_label_map(snap_b);

    let keys_a: HashSet<&String> = map_a.keys().collect();
    let keys_b: HashSet<&String> = map_b.keys().collect();

    // ── Step 2: partition labels ─────────────────────────────────────────

    let shared_labels: Vec<&&String> = {
        let mut v: Vec<_> = keys_a.intersection(&keys_b).collect();
        v.sort_by_key(|l| l.as_str());
        v
    };
    let only_in_a = diff_vertex_entries(&map_a, &keys_a, &keys_b);
    let only_in_b = diff_vertex_entries(&map_b, &keys_b, &keys_a);
    let (shared, pattern_mismatch_count, width_mismatch_count) =
        compare_shared_vertices(&shared_labels, &map_a, &map_b);
    let verdict = compute_verdict(
        &mode,
        &only_in_a,
        &only_in_b,
        &shared,
        pattern_mismatch_count,
        width_mismatch_count,
    );

    let summary = DiffSummary {
        verdict: verdict.clone(),
        shared_count: shared.len(),
        only_in_a_count: only_in_a.len(),
        only_in_b_count: only_in_b.len(),
        pattern_mismatch_count,
        width_mismatch_count,
    };

    GraphDiffResult {
        workspace_a: workspace_a.to_string(),
        workspace_b: workspace_b.to_string(),
        mode,
        summary,
        shared,
        only_in_a,
        only_in_b,
    }
}

// ---------------------------------------------------------------------------
// WorkspaceManager impls
// ---------------------------------------------------------------------------

impl WorkspaceManager {
    /// Compare two workspace graphs and return a structured diff result.
    ///
    /// Both workspaces must be open before calling this method.  Snapshots
    /// are acquired and released immediately; no cross-workspace locks are
    /// held during computation (Design Decision D24).
    ///
    /// # Errors
    ///
    /// - [`CompareError::WorkspaceNotOpen`] if either workspace is not open.
    pub fn compare_workspaces(
        &self,
        ws_a: &str,
        ws_b: &str,
        mode: CompareMode,
    ) -> Result<GraphDiffResult, CompareError> {
        let snap_a = self.get_snapshot(ws_a).map_err(|_| {
            CompareError::WorkspaceNotOpen {
                name: ws_a.to_string(),
            }
        })?;
        let snap_b = self.get_snapshot(ws_b).map_err(|_| {
            CompareError::WorkspaceNotOpen {
                name: ws_b.to_string(),
            }
        })?;
        Ok(compare_snapshots(&snap_a, ws_a, &snap_b, ws_b, mode))
    }

    /// Compare two individual vertices (by index) from two workspaces.
    ///
    /// Each vertex's sub-graph (the vertex node itself plus its direct child
    /// edges and child nodes) is extracted from the full snapshot and compared
    /// independently.
    ///
    /// # Errors
    ///
    /// - [`CompareError::WorkspaceNotOpen`] if either workspace is not open.
    /// - [`CompareError::VertexNotFound`] if the given index does not exist.
    pub fn compare_vertices(
        &self,
        ws_a: &str,
        index_a: usize,
        ws_b: &str,
        index_b: usize,
    ) -> Result<GraphDiffResult, CompareError> {
        let snap_a = self.get_snapshot(ws_a).map_err(|_| {
            CompareError::WorkspaceNotOpen {
                name: ws_a.to_string(),
            }
        })?;
        let snap_b = self.get_snapshot(ws_b).map_err(|_| {
            CompareError::WorkspaceNotOpen {
                name: ws_b.to_string(),
            }
        })?;

        let sub_a = vertex_subgraph(&snap_a, index_a).ok_or_else(|| {
            CompareError::VertexNotFound {
                workspace: ws_a.to_string(),
                index: index_a,
            }
        })?;
        let sub_b = vertex_subgraph(&snap_b, index_b).ok_or_else(|| {
            CompareError::VertexNotFound {
                workspace: ws_b.to_string(),
                index: index_b,
            }
        })?;

        Ok(compare_snapshots(
            &sub_a,
            &format!("{ws_a}[{index_a}]"),
            &sub_b,
            &format!("{ws_b}[{index_b}]"),
            CompareMode::Full,
        ))
    }
}

/// Build a minimal snapshot containing only the given vertex node and its
/// direct child edges + child nodes.  Used by `compare_vertices` to scope
/// the diff to a single vertex pair.
fn vertex_subgraph(
    snap: &GraphSnapshot,
    index: usize,
) -> Option<GraphSnapshot> {
    let root = snap.nodes.iter().find(|n| n.index == index)?;

    let edges: Vec<SnapshotEdge> = snap
        .edges
        .iter()
        .filter(|e| e.from == index)
        .cloned()
        .collect();

    let child_indices: HashSet<usize> = edges.iter().map(|e| e.to).collect();

    let mut nodes: Vec<SnapshotNode> = vec![root.clone()];
    for node in &snap.nodes {
        if child_indices.contains(&node.index) {
            nodes.push(node.clone());
        }
    }

    Some(GraphSnapshot { nodes, edges })
}
