//! Split graph (task 3.2) on every insert case and on the §9 tables.

use std::collections::BTreeSet;

use context_trace::positioned::{
    diff,
    fixtures::insert_cases,
};
use pretty_assertions::assert_eq;

use super::{
    insert_case,
    labels,
    pos,
    search_case,
    slice,
};
use crate::split::{
    traced_positions,
    Roles,
    SplitGraph,
    SplitStats,
};

/// Every required interval is a base vertex or a new vertex of the expected
/// graph, every new vertex is some interval, the trace cache gives exactly
/// the positions of the response path edges, path edges are split edges and
/// the graph is unchanged.
#[test]
fn split_every_insert_case() {
    let cases = insert_cases();
    let mut stats = SplitStats::default();
    for case in &cases {
        let graph = &case.base.graph;
        let before = graph.to_interchange();
        let generation = graph.generation();
        let response = search_case(case, graph);
        let split = SplitGraph::from_response(graph, &response).expect("not EntireRoot");
        let names = labels(&case.base);
        let expected = case.expected.graph.to_interchange();
        let new: BTreeSet<&String> = expected
            .vertices
            .keys()
            .filter(|key| !before.vertices.contains_key(*key))
            .collect();
        let mut interval_strings = BTreeSet::new();
        for &vertex in split.nodes().keys() {
            for interval in split.intervals(vertex) {
                let s = slice(&names[&vertex], interval.clone());
                assert!(
                    before.vertices.contains_key(&s) || new.contains(&s),
                    "{}: interval {interval:?} of {:?} is {s:?}, neither a base nor a new vertex",
                    case.name,
                    names[&vertex],
                );
                interval_strings.insert(s);
            }
        }
        for vertex in &new {
            assert!(
                interval_strings.contains(*vertex),
                "{}: new vertex {vertex:?} is no interval",
                case.name
            );
        }
        let path = response.path();
        let borders = [
            (path.start_path().unwrap().top_down().collect::<Vec<_>>(), response.span().start, Roles::START),
            (path.end_path().unwrap().top_down().collect::<Vec<_>>(), response.span().end, Roles::END),
        ];
        let mut from_paths = BTreeSet::new();
        for (steps, border, role) in borders {
            let mut position = border;
            for step in &steps[..steps.len() - 1] {
                position -= *step.offset;
                from_paths.insert((step.child, position, role));
                let roles = split.node(step.child).and_then(|n| n.positions().get(&position));
                assert!(
                    roles.is_some_and(|r| (r.start || !role.start) && (r.end || !role.end)),
                    "{}: path edge {step:?} is not a split edge",
                    case.name
                );
            }
        }
        let traced: BTreeSet<_> =
            traced_positions(&response.cache, response.root_token(), response.span())
                .into_iter()
                .collect();
        assert_eq!(traced, from_paths, "{}: traced edges differ from the response paths", case.name);
        assert_eq!(graph.generation(), generation, "{}", case.name);
        assert!(diff(&before, &graph.to_interchange()).is_empty(), "{}", case.name);
        stats.positions += split.stats().positions;
        stats.path_positions += split.stats().path_positions;
    }
    println!(
        "split positions on {} cases: {} non-root, {} from response path edges, {} off the paths",
        cases.len(),
        stats.positions,
        stats.path_positions,
        stats.off_path(),
    );
    assert!(stats.path_positions <= stats.positions);
}

fn roles(
    start: bool,
    end: bool,
) -> Roles {
    Roles { start, end }
}

/// §9.3: `abab` gets one position from each of two occurrences, `ab` one
/// position with both roles.
#[test]
fn split_small_periodic() {
    let case = insert_case("insert_small_periodic", "baba");
    let graph = &case.base.graph;
    let response = search_case(&case, graph);
    let split = SplitGraph::from_response(graph, &response).unwrap();
    let n = |key: &str| case.base.names[key].index;
    let positions = |key: &str| {
        split.node(n(key)).unwrap().positions().iter().map(|(&p, &r)| (*p, r)).collect::<Vec<_>>()
    };
    let (s, e) = (roles(true, false), roles(false, true));
    assert_eq!(split.nodes().len(), 3);
    assert_eq!(positions("ababab"), vec![(1, s), (5, e)]);
    assert_eq!(positions("abab"), vec![(1, s), (3, e)]);
    assert_eq!(positions("ab"), vec![(1, roles(true, true))]);
    assert_eq!(split.intervals(n("ababab")), vec![pos(1)..pos(5)]);
    assert_eq!(
        split.intervals(n("abab")),
        vec![pos(1)..pos(3), pos(0)..pos(3), pos(1)..pos(4)]
    );
    assert_eq!(split.intervals(n("ab")), vec![pos(0)..pos(1), pos(1)..pos(2)]);
    assert_eq!(split.stats().off_path(), 0);
}

/// §9.1: a vertex on both paths; all positions come from path edges.
#[test]
fn split_shared_vertex() {
    let case = insert_case("insert_shared_vertex", "abcabca");
    let graph = &case.base.graph;
    let response = search_case(&case, graph);
    assert_eq!(response.span(), pos(2)..pos(9));
    let split = SplitGraph::from_response(graph, &response).unwrap();
    let n = |key: &str| case.base.names[key].index;
    let positions = |key: &str| {
        split.node(n(key)).unwrap().positions().iter().map(|(&p, &r)| (*p, r)).collect::<Vec<_>>()
    };
    let (s, e) = (roles(true, false), roles(false, true));
    assert_eq!(split.nodes().len(), 5);
    assert_eq!(positions("acabcabcab"), vec![(2, s), (9, e)]);
    assert_eq!(positions("cabcab"), vec![(1, s), (5, e)]);
    assert_eq!(positions("cabc"), vec![(1, s)]);
    assert_eq!(positions("bcab"), vec![(3, e)]);
    assert_eq!(positions("cab"), vec![(1, s), (2, e)]);
    let iv = |x: usize, y: usize| pos(x)..pos(y);
    assert_eq!(split.intervals(n("acabcabcab")), vec![iv(2, 9)]);
    assert_eq!(split.intervals(n("cabcab")), vec![iv(1, 5), iv(0, 5), iv(1, 6)]);
    assert_eq!(split.intervals(n("cabc")), vec![iv(1, 4)]);
    assert_eq!(split.intervals(n("bcab")), vec![iv(0, 3)]);
    assert_eq!(split.intervals(n("cab")), vec![iv(1, 2), iv(0, 2), iv(1, 3)]);
    assert_eq!(split.stats().off_path(), 0);
}

/// §9.2: `abcaba` gets `3:S` from an occurrence off the response paths.
#[test]
fn split_off_path_position() {
    let case = insert_case("complex_overlap", "abababc");
    let graph = &case.base.graph;
    let response = search_case(&case, graph);
    assert_eq!(response.span(), pos(3)..pos(10));
    let split = SplitGraph::from_response(graph, &response).unwrap();
    let n = |key: &str| case.base.names[key].index;
    let abcaba = split.node(n("abcaba")).unwrap();
    assert_eq!(
        abcaba.positions().iter().map(|(&p, &r)| (*p, r)).collect::<Vec<_>>(),
        vec![(3, roles(true, true))]
    );
    assert_eq!(abcaba.intervals(false), vec![pos(0)..pos(3), pos(3)..pos(6)]);
    assert!(split.stats().off_path() >= 1);
}
