//! Plan 2.6 per-query validation: a response is checked against the
//! brute-force expectation derived only from the fixture strings (through
//! the name map), never from search output.

use std::collections::{BTreeSet, HashMap};

use context_trace::{
    graph::vertex::VertexIndex,
    positioned::{fixtures::FixtureGraph, OccurrenceLocation, SpanCoverage},
    AtomPosition, DirectedKey, DownPosition, Token, UpPosition,
};
use pretty_assertions::assert_eq;

use super::util::{chars, expected, pos, span};
use crate::{state::matched::PathCoverage, Response};

/// Fixture strings indexed for brute-force lookups.
pub(super) struct Oracle<'f> {
    pub(super) f: &'f FixtureGraph,
    keys: HashMap<VertexIndex, Vec<char>>,
}

impl<'f> Oracle<'f> {
    pub(super) fn new(f: &'f FixtureGraph) -> Self {
        let keys = f
            .names
            .iter()
            .map(|(key, token)| (token.index, chars(key)))
            .collect();
        Self { f, keys }
    }
    pub(super) fn key(&self, index: VertexIndex) -> &[char] {
        &self.keys[&index]
    }
    fn token(&self, key: &[char]) -> Token {
        self.f.names[&key.iter().collect::<String>()]
    }
    /// Largest fixture key narrower than `root` occurring in it at `x`.
    pub(super) fn largest_starting_at(&self, root: &[char], x: usize) -> Token {
        self.f
            .names
            .keys()
            .map(|key| chars(key))
            .filter(|key| key.len() < root.len() && root[x..].starts_with(key))
            .max_by_key(Vec::len)
            .map(|key| self.token(&key))
            .expect("the atom at x is a key")
    }
    /// Largest fixture key narrower than `root` occurring in it ending at `e`.
    pub(super) fn largest_ending_at(&self, root: &[char], e: usize) -> Token {
        self.f
            .names
            .keys()
            .map(|key| chars(key))
            .filter(|key| key.len() < root.len() && root[..e].ends_with(key))
            .max_by_key(Vec::len)
            .map(|key| self.token(&key))
            .expect("the atom before e is a key")
    }
}

/// `(parent key, child key, child offset)` as comparable strings.
type Edges = BTreeSet<String>;

fn edge(parent: &DirectedKey, child: &DirectedKey, offset: AtomPosition) -> String {
    format!("{parent:?} -> {child:?} @ {}", *offset)
}

/// Every cache edge, with absolute query positions shifted by `-shift`.
pub(super) fn cache_edges(response: &Response, shift: usize) -> Edges {
    let shifted = |key: &DirectedKey| {
        let p = pos(**context_trace::HasAtomPosition::pos(&key.pos) - shift);
        match key.pos {
            context_trace::DirectedPosition::BottomUp(_) => {
                DirectedKey::up(key.index, UpPosition(p))
            }
            context_trace::DirectedPosition::TopDown(_) => {
                DirectedKey::down(key.index, DownPosition(p))
            }
        }
    };
    response
        .cache
        .edges()
        .map(|(parent, child, offset)| edge(&shifted(&parent), &shifted(child), offset))
        .collect()
}

/// Steps with the absolute offset of each child in the root.
fn absolute<'a>(
    steps: impl IntoIterator<Item = &'a OccurrenceLocation>,
) -> Vec<(OccurrenceLocation, usize)> {
    let mut offset = 0;
    steps
        .into_iter()
        .map(|step| {
            offset += *step.offset;
            (*step, offset)
        })
        .collect()
}

/// Check `response` for the query `query` (the rest of a query starting at
/// the absolute query position `query_start`) against the brute-force
/// expectation of 2.6: root, span, coverage, end reason, diagnostic read,
/// canonical and maximally trimmed paths, trace cache.
pub(super) fn validate(oracle: &Oracle, query: &str, query_start: usize, response: &Response) {
    let f = oracle.f;
    let graph = &f.graph;
    let context = format!("{}: {query} at {query_start}", f.name);
    let q = chars(query);
    let e = expected(&f.corpus, &f.names, query);
    let (o, k) = (e.offset, e.k);
    let r = chars(&e.root);
    let root = f.names[&e.root];
    let qpos = |x: usize| pos(query_start + x - o);

    // Root, span, end reason, coverage.
    assert_eq!(response.root_token(), root, "{context}: root");
    assert_eq!(response.span(), span(o, o + k), "{context}: span");
    assert_eq!(
        response.query_start(),
        pos(query_start),
        "{context}: query start"
    );
    assert_eq!(
        response.checkpoint_position(),
        pos(query_start + k),
        "{context}: match end"
    );
    assert_eq!(
        response.query_exhausted(),
        k == q.len(),
        "{context}: end reason"
    );
    let coverage = match (o == 0, o + k == r.len()) {
        (true, true) => SpanCoverage::EntireRoot,
        (true, false) => SpanCoverage::Prefix,
        (false, true) => SpanCoverage::Postfix,
        (false, false) => SpanCoverage::Range,
    };
    assert_eq!(response.path().coverage(), coverage, "{context}: coverage");
    // Diagnostic read (tests only).
    let view = graph.interval(root.index, span(o, o + k)).unwrap();
    assert_eq!(
        graph.read_interval(&view).unwrap(),
        q[..k],
        "{context}: read_interval"
    );

    // Paths: absent only for an unmatched atom `q0`.
    let Some(range) = response.path().range() else {
        assert_eq!(r.len(), 1, "{context}: composite root without paths");
        assert!(
            matches!(response.path(), PathCoverage::EntireRoot(_)),
            "{context}"
        );
        assert!(response.cache.edges().next().is_none(), "{context}: edges");
        return;
    };
    let path = &range.path;
    assert_eq!(path.root(), root, "{context}");
    assert_eq!(path.span, span(o, o + k), "{context}");
    let width = |v: VertexIndex| oracle.key(v).len();

    // Start path: canonical and maximally trimmed.
    let start = absolute(path.start.top_down());
    assert!(
        graph.contains_path(path.start.top_down()),
        "{context}: start path"
    );
    assert_eq!(start[0].0.parent, root.index, "{context}");
    let &(last, s_offset) = start.last().unwrap();
    assert_eq!(last.child, path.start.leaf().index, "{context}");
    assert_eq!(s_offset, o, "{context}: start token at o");
    assert_eq!(
        path.start.leaf(),
        oracle.largest_starting_at(&r, o),
        "{context}: start path ends at the largest token at o"
    );
    assert!(
        o + width(last.child) <= o + k,
        "{context}: start token inside"
    );
    for &(step, a) in &start[..start.len() - 1] {
        let end = a + width(step.child);
        assert!(
            !(a == o && end <= o + k),
            "{context}: start trimming at {step:?}"
        );
        assert!(
            a <= o && end <= o + k,
            "{context}: start path token {step:?}"
        );
    }
    let mut v = path.start.leaf();
    while let Some(prefix) = graph.prefix_child(v) {
        assert!(
            r[o..].starts_with(oracle.key(prefix.child.index)),
            "{context}"
        );
        v = prefix.child;
    }
    assert_eq!(v, f.names[&r[o].to_string()], "{context}: prefix chain");

    // End path: largest token ending at o + k (2.4 deviation), trimmed.
    let end = absolute(path.end.steps());
    assert!(
        graph.contains_path(path.end.top_down()),
        "{context}: end path"
    );
    assert_eq!(end[0].0.parent, root.index, "{context}");
    let &(last, e_offset) = end.last().unwrap();
    assert_eq!(last.child, path.end.leaf().index, "{context}");
    assert_eq!(
        e_offset + width(last.child),
        o + k,
        "{context}: end token at o + k"
    );
    assert!(e_offset >= o, "{context}: end token inside");
    assert_eq!(
        path.end.leaf(),
        oracle.largest_ending_at(&r, o + k),
        "{context}: end path ends at the largest token ending at o + k"
    );
    for &(step, b) in &end[..end.len() - 1] {
        let stop = b + width(step.child);
        assert!(
            !(stop == o + k && b >= o),
            "{context}: end trimming at {step:?}"
        );
        assert!(stop >= o + k, "{context}: end path token {step:?}");
        // Down keys of final responses are never clamped.
        assert!(b >= o, "{context}: clamped {step:?}");
    }
    let mut v = path.end.leaf();
    while let Some(composition) = graph.vertex(v.index).unwrap().composition() {
        let child = composition.occurrences().last().unwrap().child;
        assert!(r[..o + k].ends_with(oracle.key(child.index)), "{context}");
        v = child;
    }
    assert_eq!(
        v,
        f.names[&r[o + k - 1].to_string()],
        "{context}: last chain"
    );

    // Trace cache, per coverage as legacy: up edges along the start path
    // (`Postfix`, `Range`), down edges along the end path (`Prefix`,
    // `Range`), nothing for `EntireRoot`.
    let token = |v: VertexIndex| graph.token(v).unwrap();
    let (trace_start, trace_end) = match coverage {
        SpanCoverage::Range => (true, true),
        SpanCoverage::Postfix => (true, false),
        SpanCoverage::Prefix => (false, true),
        SpanCoverage::EntireRoot => (false, false),
    };
    let mut expected_edges = Edges::new();
    let mut traced = BTreeSet::new();
    if trace_start {
        let entry_end = start[0].1 + width(start[0].0.child);
        let mut parent = DirectedKey::up(root, UpPosition(qpos(entry_end)));
        for &(step, a) in &start {
            let child = DirectedKey::up(token(step.child), UpPosition(qpos(a + width(step.child))));
            expected_edges.insert(edge(&parent, &child, step.offset));
            traced.extend([step.parent, step.child]);
            parent = child;
        }
    }
    if trace_end {
        let mut parent = DirectedKey::down(root, DownPosition(qpos(end[0].1)));
        for &(step, b) in &end {
            let child = DirectedKey::down(token(step.child), DownPosition(qpos(b)));
            expected_edges.insert(edge(&parent, &child, step.offset));
            traced.extend([step.parent, step.child]);
            parent = child;
        }
    }
    assert_eq!(
        cache_edges(response, 0),
        expected_edges,
        "{context}: cache edges"
    );
    for &v in &traced {
        assert!(
            response.cache.exists_vertex(v),
            "{context}: cache entry {v:?}"
        );
    }
    let untraced: Vec<_> = response
        .cache
        .entries
        .keys()
        .filter(|v| !traced.contains(v))
        .collect();
    assert!(
        untraced.len() <= 1,
        "{context}: untraced entries {untraced:?}"
    );
    for (parent, child, offset) in response.cache.edges() {
        let ctx = format!(
            "{context}: cache edge {parent:?} -> {child:?} @ {}",
            *offset
        );
        match parent.pos {
            context_trace::DirectedPosition::BottomUp(_) => assert!(
                graph
                    .vertex(child.index.index)
                    .unwrap()
                    .parents()
                    .get(&parent.index.index)
                    .is_some_and(|offsets| offsets.contains(&offset)),
                "{ctx}: up edge is a parent link"
            ),
            context_trace::DirectedPosition::TopDown(_) => assert!(
                graph
                    .vertex(parent.index.index)
                    .unwrap()
                    .composition()
                    .unwrap()
                    .occurrences()
                    .iter()
                    .any(|occ| occ.offset == offset && occ.child.index == child.index.index),
                "{ctx}: down edge is an occurrence"
            ),
        }
    }
}
