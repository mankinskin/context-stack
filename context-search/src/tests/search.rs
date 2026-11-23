//! Positioned ancestor search on named fixtures (mirroring
//! `legacy/tests/search/ancestor.rs`), errors and continuation.
//! Expectations come from the fixture strings via the name map.

use context_trace::{
    positioned::{fixtures::FixtureGraph, SpanCoverage},
    AtomPosition, DirectedKey, DirectedPosition, DownPosition, HasAtomPosition, Token, UpPosition,
};
use pretty_assertions::assert_eq;

use super::{
    util::{fixture, pos, span, step},
    validate::{validate, Oracle},
};
use crate::{search::stats::last_search_stats, Find, Response, SearchError};

/// Search `query` and check it against the brute-force expectation (plan
/// 2.6, [`validate`]) and the uniqueness counter.
pub(super) fn check(f: &FixtureGraph, query: &str) -> Response {
    let context = format!("{}: {query}", f.name);
    let response = f
        .graph
        .find_ancestor(query.chars())
        .unwrap_or_else(|error| panic!("{context}: {error}"));
    let stats = last_search_stats();
    validate(&Oracle::new(f), query, 0, &response);
    assert_eq!(stats.equal_width_first_matches, 0, "{context}: uniqueness");
    response
}

pub(super) fn names<const N: usize>(f: &FixtureGraph, keys: [&str; N]) -> [Token; N] {
    keys.map(|key| f.names[key])
}

/// Cache edges `(parent key, child key, child offset)`, narrowest parent
/// first.
pub(super) fn edges(response: &Response) -> Vec<(DirectedKey, DirectedKey, AtomPosition)> {
    let mut edges: Vec<_> = response
        .cache
        .edges()
        .map(|(parent, child, offset)| (parent, child.clone(), offset))
        .collect();
    edges.sort_by_key(|(parent, child, _)| {
        (parent.index.width, *parent.pos.pos(), child.index.width)
    });
    edges
}

pub(super) fn start_steps(response: &Response) -> Vec<context_trace::positioned::OccurrenceLocation> {
    response
        .path()
        .start_path()
        .unwrap()
        .top_down()
        .copied()
        .collect()
}

#[test]
fn repeat_abcabca() {
    let f = fixture("repeat_abcabca");
    let [b, abca, abcabca] = names(&f, ["b", "abca", "abcabca"]);
    let response = check(&f, "abcab");
    assert_eq!(response.root_token(), abcabca);
    assert_eq!(response.span(), span(0, 5));
    assert_eq!(response.path().coverage(), SpanCoverage::Prefix);
    assert!(response.query_exhausted());
    assert_eq!(start_steps(&response), vec![step(abcabca, 0, abca)]);
    assert_eq!(
        response.path().end_path().unwrap().steps(),
        &[step(abcabca, 3, abca), step(abca, 1, b)]
    );
    // Prefix: down edges along the end path, keyed by the query position
    // at each token start, labelled by the child offset.
    let down = |t: Token, p: usize| DirectedKey::down(t, DownPosition(pos(p)));
    assert_eq!(
        edges(&response),
        vec![
            (down(abca, 3), down(b, 4), pos(1)),
            (down(abcabca, 3), down(abca, 3), pos(3)),
        ]
    );
}

/// Postfix: up edges along the start path, keyed by the query position at
/// each token end.
#[test]
fn repeat_abcabca_postfix() {
    let f = fixture("repeat_abcabca");
    let [c, abca, abcabca] = names(&f, ["c", "abca", "abcabca"]);
    let response = check(&f, "cabcab");
    assert_eq!(response.root_token(), abcabca);
    assert_eq!(response.span(), span(2, 7));
    assert_eq!(response.path().coverage(), SpanCoverage::Postfix);
    assert!(!response.query_exhausted());
    assert_eq!(
        start_steps(&response),
        vec![step(abcabca, 0, abca), step(abca, 2, c)]
    );
    let up = |t: Token, p: usize| DirectedKey::up(t, UpPosition(pos(p)));
    assert_eq!(
        edges(&response),
        vec![
            (up(abca, 2), up(c, 1), pos(2)),
            (up(abcabca, 2), up(abca, 2), pos(0)),
        ]
    );
}

#[test]
fn repeat_abcabca_composite_query() {
    let f = fixture("repeat_abcabca");
    let [b, abca, abcabca] = names(&f, ["b", "abca", "abcabca"]);
    let atoms = check(&f, "abcab");
    let response = f.graph.find_ancestor(vec![abca, b]).unwrap();
    assert_eq!(response.root_token(), abcabca);
    assert_eq!(response.span(), span(0, 5));
    assert!(response.query_exhausted());
    assert_eq!(response.path(), atoms.path());
    // Same traced edges; only the (edge-less) start token entry differs.
    let edges = |r: &Response| {
        let mut edges: Vec<_> = r
            .cache
            .edges()
            .map(|(parent, child, offset)| format!("{parent:?} {child:?} {offset}"))
            .collect();
        edges.sort();
        edges
    };
    assert_eq!(edges(&response), edges(&atoms));
}

#[test]
fn repeat_ababa() {
    let f = fixture("repeat_ababa");
    let [b, aba, ababa] = names(&f, ["b", "aba", "ababa"]);
    let whole = check(&f, "ababa");
    assert_eq!(whole.root_token(), ababa);
    assert_eq!(whole.path().coverage(), SpanCoverage::EntireRoot);
    assert_eq!(
        whole.entire_root_start_path(),
        Some(vec![step(ababa, 0, aba)])
    );

    let inner = check(&f, "bab");
    assert_eq!(inner.root_token(), ababa);
    assert_eq!(inner.span(), span(1, 4));
    assert_eq!(inner.path().coverage(), SpanCoverage::Range);
    assert_eq!(
        start_steps(&inner),
        vec![step(ababa, 0, aba), step(aba, 1, b)]
    );
    assert_eq!(
        inner.path().end_path().unwrap().steps(),
        &[step(ababa, 2, aba), step(aba, 1, b)]
    );
}

#[test]
fn seam_bcd() {
    let f = fixture("seam_bcd");
    let [bc, bcd, cd, abc, abcde] = names(&f, ["bc", "bcd", "cd", "abc", "abcde"]);
    let response = check(&f, "bcd");
    assert_eq!(response.root_token(), bcd);
    assert_eq!(response.path().coverage(), SpanCoverage::EntireRoot);
    assert_eq!(start_steps(&response), vec![step(bcd, 0, bc)]);
    assert_eq!(
        response.path().end_path().unwrap().steps(),
        &[step(bcd, 1, cd)]
    );

    let response = check(&f, "abcd");
    assert_eq!(response.root_token(), abcde);
    assert_eq!(response.span(), span(0, 4));
    assert_eq!(response.path().coverage(), SpanCoverage::Prefix);
    assert_eq!(start_steps(&response), vec![step(abcde, 0, abc)]);
    // The end path ends at the largest token ending at the match end.
    assert_eq!(
        response.path().end_path().unwrap().steps(),
        &[step(abcde, 1, bcd)]
    );
}

#[test]
fn repeat_ababab() {
    let f = fixture("repeat_ababab");
    for query in ["ab", "aba", "abab", "babab", "bab", "baba"] {
        check(&f, query);
    }
    let whole = check(&f, "ababab");
    assert_eq!(whole.root_token(), f.names["ababab"]);
    assert_eq!(whole.path().coverage(), SpanCoverage::EntireRoot);
}

#[test]
fn repeating_known() {
    let f = fixture("repeating_known");
    let response = check(&f, "xyyxy");
    assert_eq!(response.root_token(), f.names["xyyxy"]);
    assert_eq!(response.path().coverage(), SpanCoverage::EntireRoot);
    for query in ["yx", "yxy", "xyy", "yy"] {
        check(&f, query);
    }
}

#[test]
#[allow(non_snake_case)]
fn late_retention__2() {
    let f = fixture("late_retention__2");
    for (query, root) in [("vim", "vim"), ("vis", "vis"), ("vi", "vi"), ("im", "vim")] {
        let response = check(&f, query);
        assert_eq!(response.root_token(), f.names[root], "{query}");
        assert!(response.query_exhausted());
    }
    // The root ends before the query and has no parents.
    let response = check(&f, "vimv");
    assert_eq!(response.root_token(), f.names["vim"]);
    assert!(!response.query_exhausted());
    assert_eq!(response.checkpoint_position(), pos(3));
    assert_eq!(response.cursor_position(), pos(4));
}

#[test]
fn no_parent_match_is_entire_root_of_first_token() {
    let f = fixture("late_retention__2");
    let [i] = names(&f, ["i"]);
    let response = check(&f, "ivi");
    assert_eq!(response.root_token(), i);
    assert!(response.is_entire_root());
    assert!(response.path().range().is_none());
    assert_eq!(response.span(), span(0, 1));
    assert_eq!(response.checkpoint_position(), pos(1));
    assert!(!response.query_exhausted());
    assert_eq!(response.entire_root_start_path(), Some(vec![]));
}

#[test]
fn errors() {
    let f = fixture("late_retention__2");
    let [v, vi] = names(&f, ["v", "vi"]);
    assert_eq!(
        f.graph.find_ancestor("v".chars()),
        Err(SearchError::SingleIndex(v))
    );
    assert_eq!(
        f.graph.find_ancestor(vec![vi]),
        Err(SearchError::SingleIndex(vi))
    );
    assert_eq!(
        f.graph.find_ancestor("vz".chars()),
        Err(SearchError::UnknownAtom)
    );
    assert_eq!(
        f.graph.find_ancestor("".chars()),
        Err(SearchError::EmptyPatterns)
    );
    assert_eq!(
        f.graph.find_ancestor(Vec::<Token>::new()),
        Err(SearchError::EmptyPatterns)
    );
}

#[test]
fn positioned_graph_ref() {
    let f = fixture("repeat_abcabca");
    let expected = f.graph.find_ancestor("abcab".chars()).unwrap();
    assert_eq!(expected.generation(), f.graph.generation());
    let graph = context_trace::positioned::PositionedGraphRef::new(f.graph);
    assert_eq!(graph.find_ancestor("abcab".chars()).unwrap(), expected);
}

/// A continuation from the response cursor equals a fresh search of the
/// rest, shifted by the checkpoint, and compares no earlier position.
#[test]
fn continuation() {
    let f = fixture("late_retention__2");
    let first = check(&f, "vimvis");
    assert_eq!(first.root_token(), f.names["vim"]);
    assert_eq!(first.checkpoint_position(), pos(3));

    let next = f.graph.find_ancestor(first.query_cursor()).unwrap();
    let compared = last_search_stats().compared_positions;
    assert!(compared.iter().all(|&p| p >= pos(3)), "{compared:?}");
    let fresh = check(&f, "vis");
    let fresh_compared = last_search_stats().compared_positions;

    assert_eq!(next.root_token(), fresh.root_token());
    assert_eq!(next.span(), fresh.span());
    assert_eq!(next.query_start(), pos(3));
    assert_eq!(next.checkpoint_position(), pos(6));
    assert!(next.query_exhausted());
    assert_eq!(
        next.path().range().map(|r| &r.path),
        fresh.path().range().map(|r| &r.path)
    );
    assert_eq!(
        compared.iter().map(|&p| *p - 3).collect::<Vec<_>>(),
        fresh_compared.iter().map(|&p| *p).collect::<Vec<_>>()
    );
    // Cache keys are absolute query positions: shifted by the checkpoint.
    let shift = |key: DirectedKey| {
        let p = pos(**key.pos.pos() - 3);
        match key.pos {
            DirectedPosition::BottomUp(_) => DirectedKey::up(key.index, UpPosition(p)),
            DirectedPosition::TopDown(_) => DirectedKey::down(key.index, DownPosition(p)),
        }
    };
    let shifted: Vec<_> = edges(&next)
        .into_iter()
        .map(|(parent, child, offset)| (shift(parent), shift(child), offset))
        .collect();
    assert_eq!(shifted, edges(&fresh));
    // Continuing at the query end has nothing left to search.
    assert_eq!(
        f.graph.find_ancestor(&next),
        Err(SearchError::EmptyPatterns)
    );
}

/// 2.6 finding: an unmatched composite first token is `EntireRoot` of that
/// token with the same canonical paths as the atom query matching the whole
/// token (it used to have no paths).
#[test]
fn unmatched_composite_first_token_has_atom_query_paths() {
    let f = fixture("repeat_abcabca");
    let [a, c, abca] = names(&f, ["a", "c", "abca"]);
    let atoms = check(&f, "abcac");
    assert_eq!(atoms.root_token(), abca);
    assert!(atoms.is_entire_root());
    let response = f.graph.find_ancestor(vec![abca, c]).unwrap();
    assert_eq!(response.root_token(), abca);
    assert!(response.is_entire_root());
    assert_eq!(response.checkpoint_position(), pos(4));
    assert_eq!(response.path(), atoms.path());
    assert_eq!(start_steps(&response), vec![step(abca, 0, a)]);
    assert_eq!(
        response.path().end_path().unwrap().steps(),
        &[step(abca, 3, a)]
    );
    assert!(response.cache.edges().next().is_none());
}
