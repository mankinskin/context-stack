//! Materializing accepted responses, translating known locations through
//! their publication, and closing overlaps from structural evidence.

use crate::{
    insert::{
        Materialization,
        ToInsertCtx,
    },
    overlap::{
        ClosurePolicy,
        Overlap,
        find_complete,
        prefix_frames,
    },
};
use context_search::Find;
use context_trace::legacy::*;

fn atoms<const N: usize>(
    graph: &HypergraphRef,
    chars: &str,
) -> [Token; N] {
    graph
        .insert_atoms(chars.chars().map(Atom::Element))
        .try_into()
        .unwrap()
}

fn only_pattern_id(
    graph: &HypergraphRef,
    token: Token,
) -> PatternId {
    let patterns = graph.expect_child_patterns(token);
    assert_eq!(patterns.len(), 1);
    *patterns.keys().next().unwrap()
}

#[test]
fn materialize_translates_shifted_location() {
    let graph = HypergraphRef::from(Hypergraph::default());
    let [a, b, y] = atoms(&graph, "aby");
    let (root, id) = graph.insert_pattern_with_id(vec![a, b, y]);
    let id = id.unwrap();
    let response = graph.find_ancestor(vec![a, b]).unwrap();
    let materialized =
        ToInsertCtx::<IndexWithPath>::materialize(&graph, response).unwrap();
    assert!(materialized.outcome.is_expanded());
    let ab = materialized.outcome.token();
    assert_patterns! {
        graph,
        ab => [[a, b]],
        root => [[ab, y]]
    };
    let translated = materialized.publication.translate(
        &graph,
        &[ChildLocation::new(root, id, 2)],
        y,
        false,
    );
    assert_eq!(translated, vec![ChildLocation::new(root, id, 1)]);
    assert_eq!(
        materialized.prefix_path(&graph),
        Some(vec![ChildLocation::new(ab, only_pattern_id(&graph, ab), 0)])
    );
}

#[test]
fn materialize_translates_location_into_replacement() {
    let graph = HypergraphRef::from(Hypergraph::default());
    let [x, a, b] = atoms(&graph, "xab");
    let (root, id) = graph.insert_pattern_with_id(vec![x, a, b]);
    let id = id.unwrap();
    let response = graph.find_ancestor(vec![a, b]).unwrap();
    let materialized =
        ToInsertCtx::<IndexWithPath>::materialize(&graph, response).unwrap();
    let ab = materialized.outcome.token();
    assert_patterns! {
        graph,
        ab => [[a, b]],
        root => [[x, ab]]
    };
    let translated = materialized.publication.translate(
        &graph,
        &[ChildLocation::new(root, id, 2)],
        b,
        false,
    );
    assert_eq!(
        translated,
        vec![
            ChildLocation::new(root, id, 1),
            ChildLocation::new(ab, only_pattern_id(&graph, ab), 1),
        ]
    );
}

#[test]
fn materialize_reuses_complete_match_without_edits() {
    let graph = HypergraphRef::from(Hypergraph::default());
    let [a, b, c] = atoms(&graph, "abc");
    let ab = graph.insert_pattern(vec![a, b]);
    let abc = graph.insert_pattern(vec![ab, c]);
    let response = graph.find_ancestor(vec![a, b, c]).unwrap();
    let before = graph.vertex_count();
    let materialized =
        ToInsertCtx::<IndexWithPath>::materialize(&graph, response).unwrap();
    assert!(materialized.outcome.is_complete());
    assert_eq!(materialized.outcome.token(), abc);
    assert!(materialized.publication.is_unchanged());
    assert_eq!(graph.vertex_count(), before);
    assert_eq!(
        materialized.prefix_path(&graph),
        Some(vec![
            ChildLocation::new(abc, only_pattern_id(&graph, abc), 0),
            ChildLocation::new(ab, only_pattern_id(&graph, ab), 0),
        ])
    );
}

#[test]
fn single_step_closes_scope_without_nested_union() {
    let graph = HypergraphRef::from(Hypergraph::default());
    let [a, b, c, d] = atoms(&graph, "abcd");
    let (bc, bc_id) = graph.insert_pattern_with_id(vec![b, c]);
    let (abc, abc_id) = graph.insert_pattern_with_id(vec![a, bc]);
    let cd = graph.insert_pattern(vec![c, d]);
    let overlap = Overlap {
        scope: abc,
        anchor_path: vec![
            ChildLocation::new(abc, abc_id.unwrap(), 1),
            ChildLocation::new(bc, bc_id.unwrap(), 1),
        ],
        expansion: cd,
        expansion_path: prefix_frames(&graph, cd, c).unwrap(),
    };
    assert_eq!(overlap.shared(&graph), c);
    let abcd = overlap.close(&graph, ClosurePolicy::SingleStep).unwrap();
    let ab = graph.find_ancestor(vec![a, b]).unwrap().root_token();
    assert_patterns! {
        graph,
        abcd => [[abc, d], [ab, cd]],
        abc => [[a, bc], [ab, c]]
    };
}

#[test]
fn nested_closure_steps_ignore_wrappers_from_complements() {
    let graph = HypergraphRef::from(Hypergraph::default());
    let [x, a, b] = atoms(&graph, "xab");
    let (aba, aba_id) = graph.insert_pattern_with_id(vec![a, b, a]);
    let (root, root_id) = graph.insert_pattern_with_id(vec![x, aba]);
    let overlap = Overlap {
        scope: root,
        anchor_path: vec![
            ChildLocation::new(root, root_id.unwrap(), 1),
            ChildLocation::new(aba, aba_id.unwrap(), 2),
        ],
        expansion: aba,
        expansion_path: prefix_frames(&graph, aba, a).unwrap(),
    };
    let before = graph.vertex_count();
    // Resolving `T = ba` wraps the anchor frame `aba -> a` into `ba`, which
    // must not become a closure step of its own (no `baba`).
    let closed = overlap.close(&graph, ClosurePolicy::Nested).unwrap();
    assert_eq!(graph.vertex_count(), before + 4);
    let ab = graph.find_ancestor(vec![a, b]).unwrap().root_token();
    let ba = graph.find_ancestor(vec![b, a]).unwrap().root_token();
    let ababa = graph.find_ancestor(vec![aba, b, a]).unwrap().root_token();
    assert_patterns! {
        graph,
        aba => [[a, ba], [ab, a]],
        ababa => [[aba, ba], [ab, aba]],
        closed => [[root, ba], [x, ababa]]
    };
}

/// Widest boundary chains of every vertex: `(path, target, prefix)` for each
/// token reached along the widest postfix and widest prefix traversal.
fn boundary_chains(graph: &HypergraphRef) -> Vec<(Vec<ChildLocation>, Token, bool)> {
    let mut chains = Vec::new();
    for (_, data) in graph.vertex_iter() {
        let token = data.to_token();
        if data.child_patterns().is_empty() {
            continue;
        }
        let mut path = Vec::new();
        for (location, postfix) in token.postfix_iter(graph.clone()) {
            path.push(location);
            chains.push((path.clone(), postfix, false));
        }
        let mut prefix = token;
        let mut path = Vec::new();
        while let Some((location, child)) = graph
            .expect_child_patterns(prefix)
            .iter()
            .map(|(&id, p)| (ChildLocation::new(prefix, id, 0), p[0]))
            .max_by_key(|(_, child)| child.width())
        {
            path.push(location);
            chains.push((path.clone(), child, true));
            prefix = child;
        }
    }
    chains
}

/// Materialize `query` and translate every boundary chain recorded before.
///
/// Translation validates that each result reaches its original target.
fn materialize_translating_all(
    graph: &HypergraphRef,
    query: Vec<Token>,
) -> Materialization {
    let chains = boundary_chains(graph);
    let response = graph.find_ancestor(query).unwrap();
    let materialized =
        ToInsertCtx::<IndexWithPath>::materialize(graph, response).unwrap();
    for (path, target, prefix) in &chains {
        materialized
            .publication
            .translate(graph, path, *target, *prefix);
        materialized
            .publication
            .translate_chain(graph, path, *target, *prefix);
    }
    materialized
}

#[test]
fn materialize_keeps_the_accepted_response() {
    let graph = HypergraphRef::from(Hypergraph::default());
    let [a, b, y] = atoms(&graph, "aby");
    graph.insert_pattern(vec![a, b, y]);
    let response = graph.find_ancestor(vec![a, b]).unwrap();
    let materialized = ToInsertCtx::<IndexWithPath>::materialize(
        &graph,
        response.clone(),
    )
    .unwrap();
    assert_eq!(materialized.outcome.response(), &response);
}

#[test]
fn translation_survives_helper_inlining() {
    let graph = HypergraphRef::from(Hypergraph::default());
    let [a, b, c] = atoms(&graph, "abc");
    let ab = graph.insert_pattern(vec![a, b]);
    let abab = graph.insert_pattern(vec![ab, ab]);
    let ababab = graph.insert_patterns(vec![vec![ab, abab], vec![abab, ab]]);
    graph.insert_pattern(vec![ab, c, ababab]);
    // Selection inlines the helper `bab` (see
    // `selection_rejects_proposal_only_support`).
    let materialized = materialize_translating_all(&graph, vec![ab, c, a, b, a]);
    assert!(materialized.outcome.is_expanded());
    assert_eq!(find_complete(&graph, &Pattern::from(vec![b, a, b])), None);
    assert!(!materialized.publication.is_unchanged());
}

#[test]
fn translation_distinguishes_repeated_occurrences() {
    let graph = HypergraphRef::from(Hypergraph::default());
    let [a, b] = atoms(&graph, "ab");
    let (root, id) = graph.insert_pattern_with_id(vec![a, b, a]);
    let id = id.unwrap();
    let materialized = materialize_translating_all(&graph, vec![a, b]);
    let ab = materialized.outcome.token();
    assert_patterns! {
        graph,
        root => [[ab, a]]
    };
    let publication = &materialized.publication;
    assert_eq!(
        publication.translate(&graph, &[ChildLocation::new(root, id, 2)], a, false),
        vec![ChildLocation::new(root, id, 1)]
    );
    assert_eq!(
        publication.translate(&graph, &[ChildLocation::new(root, id, 0)], a, true),
        vec![
            ChildLocation::new(root, id, 0),
            ChildLocation::new(ab, only_pattern_id(&graph, ab), 0),
        ]
    );
}

#[test]
fn prefix_path_projects_the_trace_of_a_larger_host() {
    let graph = HypergraphRef::from(Hypergraph::default());
    let [x, a, b, c] = atoms(&graph, "xabc");
    let (ab, ab_id) = graph.insert_pattern_with_id(vec![a, b]);
    let host = graph.insert_pattern(vec![x, ab, c]);
    let materialized = materialize_translating_all(&graph, vec![a, b, c]);
    assert!(materialized.outcome.is_expanded());
    let abc = materialized.outcome.token();
    assert_ne!(abc, host);
    assert_patterns! {
        graph,
        abc => [[ab, c]],
        host => [[x, abc]]
    };
    assert_eq!(
        materialized.prefix_path(&graph),
        Some(vec![
            ChildLocation::new(abc, only_pattern_id(&graph, abc), 0),
            ChildLocation::new(ab, ab_id.unwrap(), 0),
        ])
    );
}

#[test]
fn translate_chain_follows_wider_boundary_created_by_publication() {
    let graph = HypergraphRef::from(Hypergraph::default());
    let [x, y, p] = atoms(&graph, "xyp");
    let xy = graph.insert_pattern(vec![x, y]);
    let (scope, id) = graph.insert_pattern_with_id(vec![xy, p]);
    let narrow = ChildLocation::new(scope, id.unwrap(), 1);
    let materialized = materialize_translating_all(&graph, vec![y, p]);
    let yp = materialized.outcome.token();
    let publication = &materialized.publication;
    // The rewritten pattern is untouched along the original path ...
    assert_eq!(publication.translate(&graph, &[narrow], p, false), vec![narrow]);
    // ... but the widest chain now passes through the new postfix `yp`.
    let chain = publication.translate_chain(&graph, &[narrow], p, false);
    assert_eq!(chain.len(), 2);
    assert_eq!(chain[1].parent, yp);
    assert_eq!(boundary_target(&graph, scope, &chain[..1], false), yp);
}

#[test]
fn prefix_path_of_unextended_start_token_is_empty() {
    let graph = HypergraphRef::from(Hypergraph::default());
    let [a, b, x] = atoms(&graph, "abx");
    graph.insert_pattern(vec![a, b]);
    let response = graph.find_ancestor(vec![a, x]).unwrap();
    let materialized =
        ToInsertCtx::<IndexWithPath>::materialize(&graph, response).unwrap();
    assert!(materialized.outcome.is_no_expansion());
    assert_eq!(materialized.outcome.token(), a);
    assert_eq!(materialized.prefix_path(&graph), Some(Vec::new()));
}
