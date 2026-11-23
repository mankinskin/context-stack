//! Positioned insertion tests over the insert lane (insert plan rev 3,
//! tasks 3.2–3.4). Expectations come from fixture strings through the name
//! maps; strings are never algorithm input.

mod insert;
mod split;

use std::{
    collections::{
        BTreeMap,
        BTreeSet,
    },
    ops::Range,
};

use context_search::{
    Find,
    Response,
};
use context_trace::{
    AtomPosition,
    BaseGraphKind,
    Token,
    graph::vertex::VertexIndex,
    positioned::{
        Occurrence,
        PositionedGraph,
        fixtures::{
            FixtureGraph,
            InsertCase,
            insert_cases,
        },
        interchange::PositionedGraphData,
    },
};

pub(crate) fn pos(p: usize) -> AtomPosition {
    AtomPosition::from(p)
}

/// The insert case of scenario `scenario` with query `query`.
pub(crate) fn insert_case(
    scenario: &str,
    query: &str,
) -> InsertCase {
    insert_cases()
        .into_iter()
        .find(|case| case.base.name == scenario && case.query == query)
        .unwrap_or_else(|| panic!("no insert case {scenario}: {query}"))
}

/// Vertex index → fixture string.
pub(crate) fn labels(f: &FixtureGraph) -> BTreeMap<VertexIndex, String> {
    f.names.iter().map(|(name, token)| (token.index, name.clone())).collect()
}

/// String of `token` in `graph`, read by traversal (tests only).
pub(crate) fn content(
    graph: &PositionedGraph<BaseGraphKind>,
    token: Token,
) -> String {
    let view = graph
        .interval(token.index, pos(0)..AtomPosition::from(token.width))
        .expect("token of this graph");
    graph.read_interval(&view).expect("valid interval").into_iter().collect()
}

/// `(string, offset)` of occurrences, sorted by offset.
pub(crate) fn named(
    graph: &PositionedGraph<BaseGraphKind>,
    occurrences: &[Occurrence],
) -> Vec<(String, usize)> {
    occurrences
        .iter()
        .map(|o| (content(graph, o.child), *o.offset))
        .collect()
}

/// Brute-force minimal host of `query` from the fixture strings: the unique
/// `(vertex, x)` with `query` at `x` in no child occurrence (G5).
pub(crate) fn minimal_host(
    data: &PositionedGraphData,
    query: &str,
) -> (String, usize) {
    let query: Vec<char> = query.chars().collect();
    let hosts: Vec<(String, usize)> = data
        .vertices
        .iter()
        .flat_map(|(key, cover)| {
            let chars: Vec<char> = key.chars().collect();
            let query = &query;
            (0..(chars.len() + 1).saturating_sub(query.len()))
                .filter(move |&x| chars[x..x + query.len()] == query[..])
                .filter(|&x| {
                    !cover.iter().any(|(offset, child)| {
                        *offset <= x && x + query.len() <= offset + child.chars().count()
                    })
                })
                .map(|x| (key.clone(), x))
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(hosts.len(), 1, "minimal hosts {hosts:?}");
    hosts.into_iter().next().unwrap()
}

/// Search the case's query on its base graph and check the response
/// against the brute-force minimal host.
pub(crate) fn search_case(
    case: &InsertCase,
    graph: &PositionedGraph<BaseGraphKind>,
) -> Response {
    let response = graph
        .find_ancestor(case.query.chars())
        .unwrap_or_else(|error| panic!("{}: {error}", case.name));
    let (host, x) = minimal_host(&graph.to_interchange(), &case.query);
    let width = case.query.chars().count();
    assert_eq!(response.root_token().index, case.base.names[&host].index, "{}: root", case.name);
    assert_eq!(response.span(), pos(x)..pos(x + width), "{}: span", case.name);
    assert!(response.query_exhausted(), "{}: query exhausted", case.name);
    assert!(!response.is_entire_root(), "{}: not a token", case.name);
    response
}

/// Fixture strings of the vertices whose composition differs between the
/// two graphs (present in both).
pub(crate) fn changed_vertices(
    before: &PositionedGraphData,
    after: &PositionedGraphData,
) -> BTreeSet<String> {
    before
        .vertices
        .iter()
        .filter(|(key, cover)| {
            let set = |c: &Vec<(usize, String)>| c.iter().cloned().collect::<BTreeSet<_>>();
            after.vertices.get(*key).is_some_and(|other| set(cover) != set(other))
        })
        .map(|(key, _)| key.clone())
        .collect()
}

/// Substring `range` of `s` (in chars).
pub(crate) fn slice(
    s: &str,
    range: Range<AtomPosition>,
) -> String {
    s.chars().skip(*range.start).take(*range.end - *range.start).collect()
}
