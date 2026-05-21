use super::*;

fn make_snap(
    nodes: Vec<(usize, &str, usize)>,
    edges: Vec<(usize, usize, usize, usize)>,
) -> GraphSnapshot {
    GraphSnapshot {
        nodes: nodes
            .into_iter()
            .map(|(index, label, width)| SnapshotNode {
                index,
                label: label.to_string(),
                width,
            })
            .collect(),
        edges: edges
            .into_iter()
            .map(|(from, to, pattern_idx, sub_index)| SnapshotEdge {
                from,
                to,
                pattern_idx,
                sub_index,
            })
            .collect(),
    }
}

#[test]
fn identical_graphs_are_equivalent() {
    let snap = make_snap(
        vec![(0, "a", 1), (1, "b", 1), (2, "ab", 2)],
        vec![(2, 0, 0, 0), (2, 1, 0, 1)],
    );
    let result =
        compare_snapshots(&snap, "ws_a", &snap, "ws_b", CompareMode::Full);
    assert_eq!(result.summary.verdict, DiffVerdict::Equivalent);
    assert!(result.only_in_a.is_empty());
    assert!(result.only_in_b.is_empty());
    assert_eq!(result.summary.width_mismatch_count, 0);
    assert_eq!(result.summary.pattern_mismatch_count, 0);
}

#[test]
fn extra_vertex_in_b_is_divergent_full() {
    let snap_a = make_snap(vec![(0, "a", 1)], vec![]);
    let snap_b = make_snap(vec![(0, "a", 1), (1, "b", 1)], vec![]);
    let result =
        compare_snapshots(&snap_a, "a", &snap_b, "b", CompareMode::Full);
    assert_eq!(result.summary.verdict, DiffVerdict::Divergent);
    assert_eq!(result.only_in_b.len(), 1);
    assert_eq!(result.only_in_b[0].label, "b");
}

#[test]
fn b_subset_of_a_returns_subset_verdict() {
    let snap_a = make_snap(vec![(0, "a", 1), (1, "b", 1), (2, "c", 1)], vec![]);
    let snap_b = make_snap(vec![(0, "a", 1), (1, "b", 1)], vec![]);
    let result =
        compare_snapshots(&snap_a, "a", &snap_b, "b", CompareMode::Subset);
    assert_eq!(result.summary.verdict, DiffVerdict::Subset);
    assert_eq!(result.only_in_a.len(), 1);
    assert_eq!(result.only_in_a[0].label, "c");
}

#[test]
fn b_adding_labels_fails_subset_check() {
    let snap_a = make_snap(vec![(0, "a", 1)], vec![]);
    let snap_b = make_snap(vec![(0, "a", 1), (1, "b", 1)], vec![]);
    let result =
        compare_snapshots(&snap_a, "a", &snap_b, "b", CompareMode::Subset);
    assert_eq!(result.summary.verdict, DiffVerdict::Divergent);
}

#[test]
fn width_mismatch_is_detected() {
    let snap_a = make_snap(
        vec![(0, "a", 1), (1, "b", 1), (2, "ab", 2)],
        vec![(2, 0, 0, 0), (2, 1, 0, 1)],
    );
    let snap_b = make_snap(
        vec![(0, "a", 1), (1, "b", 1), (2, "ab", 3)],
        vec![(2, 0, 0, 0), (2, 1, 0, 1)],
    );
    let result =
        compare_snapshots(&snap_a, "a", &snap_b, "b", CompareMode::Full);
    assert_eq!(result.summary.verdict, DiffVerdict::Divergent);
    assert_eq!(result.summary.width_mismatch_count, 1);
    let sv = result
        .shared
        .iter()
        .find(|sv| sv.label == "ab")
        .expect("shared vertex 'ab' should be present");
    assert!(
        matches!(
            sv.match_kind,
            VertexMatchKind::WidthMismatch {
                width_a: 2,
                width_b: 3
            }
        ),
        "expected WidthMismatch{{2,3}}, got {:?}",
        sv.match_kind
    );
}

#[test]
fn pattern_subset_in_b_counts_as_identical() {
    let snap_a = make_snap(
        vec![
            (0, "a", 1),
            (1, "b", 1),
            (2, "c", 1),
            (3, "ab", 2),
            (4, "bc", 2),
            (5, "abc", 3),
        ],
        vec![(5, 3, 0, 0), (5, 2, 0, 1), (5, 0, 1, 0), (5, 4, 1, 1)],
    );
    let snap_b = make_snap(
        vec![
            (0, "a", 1),
            (1, "b", 1),
            (2, "c", 1),
            (3, "ab", 2),
            (5, "abc", 3),
        ],
        vec![(5, 3, 0, 0), (5, 2, 0, 1)],
    );
    let result =
        compare_snapshots(&snap_a, "a", &snap_b, "b", CompareMode::Full);
    let abc = result.shared.iter().find(|sv| sv.label == "abc").unwrap();
    assert_eq!(abc.match_kind, VertexMatchKind::Identical);
}

#[test]
fn pattern_mismatch_no_common_pattern() {
    let snap_a = make_snap(
        vec![
            (0, "a", 1),
            (1, "b", 1),
            (2, "c", 1),
            (3, "ab", 2),
            (5, "abc", 3),
        ],
        vec![(5, 3, 0, 0), (5, 2, 0, 1)],
    );
    let snap_b = make_snap(
        vec![
            (0, "a", 1),
            (1, "b", 1),
            (2, "c", 1),
            (4, "bc", 2),
            (5, "abc", 3),
        ],
        vec![(5, 0, 0, 0), (5, 4, 0, 1)],
    );
    let result =
        compare_snapshots(&snap_a, "a", &snap_b, "b", CompareMode::Full);
    assert_eq!(result.summary.verdict, DiffVerdict::Divergent);
    assert_eq!(result.summary.pattern_mismatch_count, 1);
    let abc = result.shared.iter().find(|sv| sv.label == "abc").unwrap();
    assert_eq!(abc.match_kind, VertexMatchKind::PatternMismatch);
    let diff = abc.pattern_diff.as_ref().unwrap();
    assert_eq!(
        diff.only_in_a,
        vec![vec!["ab".to_string(), "c".to_string()]]
    );
    assert_eq!(
        diff.only_in_b,
        vec![vec!["a".to_string(), "bc".to_string()]]
    );
    assert!(diff.common.is_empty());
}

#[test]
fn extra_patterns_in_b() {
    let snap_a = make_snap(
        vec![
            (0, "a", 1),
            (1, "b", 1),
            (2, "c", 1),
            (3, "ab", 2),
            (5, "abc", 3),
        ],
        vec![(5, 3, 0, 0), (5, 2, 0, 1)],
    );
    let snap_b = make_snap(
        vec![
            (0, "a", 1),
            (1, "b", 1),
            (2, "c", 1),
            (3, "ab", 2),
            (4, "bc", 2),
            (5, "abc", 3),
        ],
        vec![(5, 3, 0, 0), (5, 2, 0, 1), (5, 0, 1, 0), (5, 4, 1, 1)],
    );
    let result =
        compare_snapshots(&snap_a, "a", &snap_b, "b", CompareMode::Full);
    assert_eq!(result.summary.verdict, DiffVerdict::Divergent);
    let abc = result.shared.iter().find(|sv| sv.label == "abc").unwrap();
    assert_eq!(abc.match_kind, VertexMatchKind::ExtraPatterns);
}

#[test]
fn vertex_subgraph_extracts_correct_nodes() {
    let snap = make_snap(
        vec![(0, "a", 1), (1, "b", 1), (2, "ab", 2)],
        vec![(2, 0, 0, 0), (2, 1, 0, 1)],
    );
    let sub = vertex_subgraph(&snap, 2).unwrap();
    assert_eq!(sub.nodes.len(), 3);
    assert_eq!(sub.edges.len(), 2);
}

#[test]
fn vertex_subgraph_missing_index_returns_none() {
    let snap = make_snap(vec![(0, "a", 1)], vec![]);
    assert!(vertex_subgraph(&snap, 99).is_none());
}
