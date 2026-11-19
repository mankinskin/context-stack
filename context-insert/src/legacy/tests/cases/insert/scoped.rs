//! Scoped complement insertion: results must be canonical, represented inside
//! the scope, and must not touch tokens outside the scope.

use crate::legacy::insert::{
    ToInsertCtx,
    scoped::{
        find_represented,
        insert_scope_postfix_width,
        insert_scope_prefix_width,
    },
};
use context_trace::legacy::{
    trace::has_graph::HasGraph,
    *,
};

fn atoms<const N: usize>(
    graph: &HypergraphRef,
    chars: &str,
) -> [Token; N] {
    graph
        .insert_atoms(chars.chars().map(Atom::Element))
        .try_into()
        .unwrap()
}

#[test]
#[cfg_attr(not(feature = "legacy-tests"), ignore = "legacy suite")]
fn scoped_prefix_splits_flat_scope_only() {
    let graph = HypergraphRef::from(Hypergraph::default());
    let [v, i, s, d] = atoms(&graph, "visd");
    let vis = graph.insert_pattern(vec![v, i, s]);
    let dvi = graph.insert_pattern(vec![d, v, i]);
    let vi = insert_scope_prefix_width(&graph, vis, 2).unwrap();
    assert_patterns! {
        graph,
        vi => [[v, i]],
        vis => [[vi, s]],
        dvi => [[d, v, i]]
    };
}

#[test]
#[cfg_attr(not(feature = "legacy-tests"), ignore = "legacy suite")]
fn scoped_prefix_reuses_represented_child() {
    let graph = HypergraphRef::from(Hypergraph::default());
    let [v, i, s, u] = atoms(&graph, "visu");
    let vi = graph.insert_pattern(vec![v, i]);
    let vis = graph.insert_pattern(vec![vi, s]);
    let su = graph.insert_pattern(vec![s, u]);
    let visu = graph.insert_patterns([vec![vis, u], vec![vi, su]]);
    let before = graph.vertex_count();
    assert_eq!(insert_scope_prefix_width(&graph, visu, 2).unwrap(), vi);
    assert_eq!(insert_scope_prefix_width(&graph, visu, 3).unwrap(), vis);
    assert_eq!(insert_scope_postfix_width(&graph, visu, 2).unwrap(), su);
    assert_eq!(graph.vertex_count(), before);
}

#[test]
#[cfg_attr(not(feature = "legacy-tests"), ignore = "legacy suite")]
fn scoped_prefix_splits_nested_child() {
    let graph = HypergraphRef::from(Hypergraph::default());
    let [a, b, c, d] = atoms(&graph, "abcd");
    let bc = graph.insert_pattern(vec![b, c]);
    let abc = graph.insert_pattern(vec![a, bc]);
    let abcd = graph.insert_pattern(vec![abc, d]);
    let ab = insert_scope_prefix_width(&graph, abcd, 2).unwrap();
    assert_patterns! {
        graph,
        ab => [[a, b]],
        abc => [[a, bc], [ab, c]],
        abcd => [[abc, d]]
    };
    assert_eq!(find_represented(&graph, abcd, 0, 2), Some(ab));
}

#[test]
#[cfg_attr(not(feature = "legacy-tests"), ignore = "legacy suite")]
fn scoped_postfix_splits_flat_scope() {
    let graph = HypergraphRef::from(Hypergraph::default());
    let [s, u, b] = atoms(&graph, "sub");
    let sub = graph.insert_pattern(vec![s, u, b]);
    let ub = insert_scope_postfix_width(&graph, sub, 2).unwrap();
    assert_patterns! {
        graph,
        ub => [[u, b]],
        sub => [[s, ub]]
    };
}

#[test]
#[cfg_attr(not(feature = "legacy-tests"), ignore = "legacy suite")]
fn scoped_postfix_spans_alternative_decompositions() {
    let graph = HypergraphRef::from(Hypergraph::default());
    let [v, i, s, u] = atoms(&graph, "visu");
    let vi = graph.insert_pattern(vec![v, i]);
    let vis = graph.insert_pattern(vec![vi, s]);
    let su = graph.insert_pattern(vec![s, u]);
    let visu = graph.insert_patterns([vec![vis, u], vec![vi, su]]);
    let isu = insert_scope_postfix_width(&graph, visu, 3).unwrap();
    let is = find_represented(&graph, vis, 1, 2).expect("is in vis");
    assert_patterns! {
        graph,
        is => [[i, s]],
        isu => [[is, u], [i, su]],
        vis => [[vi, s], [v, is]],
        visu => [[vis, u], [vi, su], [v, isu]]
    };
}

#[test]
#[cfg_attr(not(feature = "legacy-tests"), ignore = "legacy suite")]
fn scoped_postfix_narrows_into_last_child() {
    let graph = HypergraphRef::from(Hypergraph::default());
    let [a, b, c, d] = atoms(&graph, "abcd");
    let bc = graph.insert_pattern(vec![b, c]);
    let ab = graph.insert_pattern(vec![a, b]);
    let abc = graph.insert_patterns([vec![a, bc], vec![ab, c]]);
    let bcd = graph.insert_pattern(vec![bc, d]);
    let abcd = graph.insert_patterns([vec![a, bcd], vec![abc, d]]);
    let cd = insert_scope_postfix_width(&graph, abcd, 2).unwrap();
    assert_patterns! {
        graph,
        cd => [[c, d]],
        bcd => [[bc, d], [b, cd]],
        abc => [[a, bc], [ab, c]],
        abcd => [[a, bcd], [abc, d]]
    };
}

#[test]
#[cfg_attr(not(feature = "legacy-tests"), ignore = "legacy suite")]
fn scoped_postfix_materializes_conflicting_wrapper_first() {
    let graph = HypergraphRef::from(Hypergraph::default());
    let [a, b, c, d, e] = atoms(&graph, "abcde");
    let bc = graph.insert_pattern(vec![b, c]);
    let bcd = graph.insert_pattern(vec![bc, d]);
    let abc = graph.insert_pattern(vec![a, bc]);
    let de = graph.insert_pattern(vec![d, e]);
    let abcde = graph.insert_patterns([vec![a, bcd, e], vec![abc, de]]);
    let cde = insert_scope_postfix_width(&graph, abcde, 3).unwrap();
    let cd = find_represented(&graph, bcd, 1, 2).expect("cd in bcd");
    let bcde = find_represented(&graph, abcde, 1, 4).expect("bcde");
    assert_patterns! {
        graph,
        cde => [[c, de], [cd, e]],
        bcd => [[bc, d], [b, cd]],
        abc => [[a, bc]],
        abcde => [[a, bcde], [abc, de]]
    };
    assert_eq!(find_represented(&graph, bcde, 1, 3), Some(cde));
}

#[test]
#[cfg_attr(not(feature = "legacy-tests"), ignore = "legacy suite")]
fn scoped_prefix_materializes_conflicting_wrapper_first() {
    let graph = HypergraphRef::from(Hypergraph::default());
    let [a, b, c, d, e] = atoms(&graph, "abcde");
    let cd = graph.insert_pattern(vec![c, d]);
    let bcd = graph.insert_pattern(vec![b, cd]);
    let cde = graph.insert_pattern(vec![cd, e]);
    let ab = graph.insert_pattern(vec![a, b]);
    let abcde = graph.insert_patterns([vec![a, bcd, e], vec![ab, cde]]);
    let abc = insert_scope_prefix_width(&graph, abcde, 3).unwrap();
    let bc = find_represented(&graph, bcd, 0, 2).expect("bc in bcd");
    let abcd = find_represented(&graph, abcde, 0, 4).expect("abcd");
    assert_patterns! {
        graph,
        abc => [[a, bc], [ab, c]],
        bcd => [[b, cd], [bc, d]],
        cde => [[cd, e]],
        abcde => [[ab, cde], [abcd, e]]
    };
    assert_eq!(find_represented(&graph, abcd, 0, 3), Some(abc));
}

#[test]
#[cfg_attr(not(feature = "legacy-tests"), ignore = "legacy suite")]
fn scoped_complements_from_boundary_paths() {
    let graph = HypergraphRef::from(Hypergraph::default());
    let [v, i, s, u] = atoms(&graph, "visu");
    let (vis, vis_id) = graph.insert_pattern_with_id(vec![v, i, s]);
    let (su, su_id) = graph.insert_pattern_with_id(vec![s, u]);
    let vi = ToInsertCtx::<Token>::insert_scope_prefix(
        &graph,
        vis,
        &[ChildLocation::new(vis, vis_id.unwrap(), 2)],
    )
    .unwrap();
    let rest = ToInsertCtx::<Token>::insert_scope_postfix(
        &graph,
        su,
        &[ChildLocation::new(su, su_id.unwrap(), 0)],
    )
    .unwrap();
    assert_eq!(rest, u);
    assert_patterns! {
        graph,
        vi => [[v, i]],
        vis => [[vi, s]]
    };
}
