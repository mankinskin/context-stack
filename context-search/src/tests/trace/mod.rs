//! Paths, trace states and the trace cache (search plan 2.2, canonical
//! rules 2.4) on the fixtures. Vertices are addressed through the name
//! map; expectations come from the fixture strings.

use std::collections::BTreeMap;

use pretty_assertions::assert_eq;

use crate::{
    ChildState,
    DirectedKey,
    End,
    IntoParentState,
    ParentState,
    RangeCommand,
    SpanCoverage,
    StateAdvance,
    Traceable,
    TraceCache,
    TraceCtx,
};
use context_trace::{
    AtomPosition,
    graph::vertex::VertexIndex,
    positioned::{
        fixtures::{
            fixture_graphs,
            FixtureGraph,
        },
        OccurrenceLocation,
        PositionedGraph,
    },
    Token,
};

mod exhaustive;
mod query;

fn chars(s: &str) -> Vec<char> {
    s.chars().collect()
}

fn fixture(name: &str) -> FixtureGraph {
    fixture_graphs()
        .into_iter()
        .find(|f| f.name == name)
        .unwrap_or_else(|| panic!("fixture {name}"))
}

/// Vertex index → fixture key (`Token`'s `Ord` compares widths only).
fn keys(fixture: &FixtureGraph) -> BTreeMap<VertexIndex, String> {
    fixture
        .names
        .iter()
        .map(|(key, token)| (token.index, key.clone()))
        .collect()
}

/// Start offsets of `needle` in `haystack`.
fn find_all(
    haystack: &str,
    needle: &str,
) -> Vec<usize> {
    let (h, n) = (chars(haystack), chars(needle));
    (0..(h.len() + 1).saturating_sub(n.len()))
        .filter(|&x| h[x..x + n.len()] == n[..])
        .collect()
}

fn pos(p: usize) -> AtomPosition {
    AtomPosition::from(p)
}

fn step(
    parent: Token,
    offset: usize,
    child: Token,
) -> OccurrenceLocation {
    OccurrenceLocation {
        parent: parent.index,
        offset: pos(offset),
        child: child.index,
    }
}

/// Match graph tokens up to the root end, as a query equal to the graph
/// content would (advance, then mark the end leaf matched).
fn match_to_root_end<G: context_trace::GraphKind>(
    graph: &PositionedGraph<G>,
    mut state: ChildState,
) -> ChildState {
    loop {
        state.match_end_leaf();
        match state.advance_state(graph) {
            Ok(next) => state = next,
            Err(end) => return end,
        }
    }
}

fn trace(
    graph: &PositionedGraph,
    state: &ChildState,
) -> TraceCache {
    let mut ctx = TraceCtx::new(graph, TraceCache::default());
    RangeCommand::from(state.clone()).trace(&mut ctx);
    ctx.cache
}

fn edges(cache: &TraceCache) -> Vec<(DirectedKey, DirectedKey, AtomPosition)> {
    let mut edges: Vec<_> = cache
        .edges()
        .map(|(parent, child, offset)| (parent, child.clone(), offset))
        .collect();
    edges.sort_by_key(|(p, c, o)| (p.index.index, *crate::trace::key::HasAtomPosition::pos(p), c.index.index, *o));
    edges
}

#[test]
fn worked_example_repeat_abcabca() {
    let fixture = fixture("repeat_abcabca");
    let graph = &fixture.graph;
    let [a, b, abca, abcabca] = ["a", "b", "abca", "abcabca"].map(|k| fixture.names[k]);
    assert_eq!(find_all("abca", "a"), vec![0, 3]);
    assert_eq!(find_all("abcabca", "abca"), vec![0, 3]);

    // Seed `a` in `abca@0`, then `b c a` match up to the root end.
    let seed = a.into_parent_state(abca, pos(0));
    assert_eq!(seed.span, pos(0)..pos(1));
    let entered = seed.clone().advance_state(graph).unwrap();
    assert_eq!(entered.root_parent, seed);
    assert_eq!(entered.path.end.leaf(), b);
    assert_eq!(entered.path.end.steps(), &[step(abca, 1, b)]);
    let exhausted = match_to_root_end(graph, entered.child_state.clone());
    assert_eq!(exhausted.path.span, pos(0)..pos(4));
    assert_eq!(exhausted.path.coverage(), SpanCoverage::EntireRoot);

    // Raise into both occurrences of `abca` in `abcabca`.
    let raised: Vec<ParentState> = [0, 3]
        .map(|o| {
            let mut state = exhausted.parent_state();
            state.path_raise(abcabca, pos(o));
            state
        })
        .into();
    for (state, o) in raised.iter().zip([0, 3]) {
        assert_eq!(state.span, pos(o)..pos(o + 4));
        assert_eq!(state.root_pos.0, pos(4));
        assert_eq!(state.prev_pos.0, pos(1));
        // The match starts at `abca`'s start: canonical path `[(abcabca, o, abca)]`.
        assert_eq!(state.path.steps(), &[step(abcabca, o, abca)]);
        assert_eq!(state.path.leaf(), abca);
    }
    assert_eq!(
        SpanCoverage::of(&raised[0].span, abcabca.width),
        SpanCoverage::Prefix
    );
    assert_eq!(
        SpanCoverage::of(&raised[1].span, abcabca.width),
        SpanCoverage::Postfix
    );
    // `abcabca@3` ends at once: the span reaches the root end.
    assert!(raised[1].clone().advance_state(graph).is_err());

    // `abcabca@0`: `abca@3` covers 4, `start_at(abca, 1) = b`.
    let mut state = raised[0].clone().advance_state(graph).unwrap().child_state;
    let end = [step(abcabca, 3, abca), step(abca, 1, b)];
    assert_eq!(state.path.end.steps(), &end);
    state.match_end_leaf();
    assert_eq!(state.path.span, pos(0)..pos(5));
    assert_eq!(state.path.coverage(), SpanCoverage::Prefix);

    // Up edges along `[(abcabca, 0, abca)]` at query position 4 (end of
    // `abca`), down edges along the end path at the query positions of the
    // token starts (`abca@3` → 3, `b` → 4).
    let cache = trace(graph, &state);
    let expected = vec![
        (
            DirectedKey::down(abcabca, pos(3)),
            DirectedKey::down(abca, pos(3)),
            pos(3),
        ),
        (
            DirectedKey::up(abcabca, pos(4)),
            DirectedKey::up(abca, pos(4)),
            pos(0),
        ),
        (
            DirectedKey::down(abca, pos(3)),
            DirectedKey::down(b, pos(4)),
            pos(1),
        ),
    ];
    let mut expected = expected;
    expected.sort_by_key(|(p, c, o)| (p.index.index, *crate::trace::key::HasAtomPosition::pos(p), c.index.index, *o));
    assert_eq!(edges(&cache), expected);
}

#[test]
fn raise_repeat_ababa() {
    let fixture = fixture("repeat_ababa");
    let graph = &fixture.graph;
    let [a, b, aba, ababa] = ["a", "b", "aba", "ababa"].map(|k| fixture.names[k]);
    assert_eq!(find_all("aba", "a"), vec![0, 2]);
    assert_eq!(find_all("ababa", "aba"), vec![0, 2]);

    // `a@0` in `aba`, matched to the end, raised into `ababa@0` and `@2`.
    let exhausted = match_to_root_end(
        graph,
        a.into_parent_state(aba, pos(0))
            .advance_state(graph)
            .unwrap()
            .child_state,
    );
    assert_eq!(exhausted.path.span, pos(0)..pos(3));
    for (o, coverage) in [(0, SpanCoverage::Prefix), (2, SpanCoverage::Postfix)] {
        let mut state = exhausted.parent_state();
        state.path_raise(ababa, pos(o));
        assert_eq!(state.span, pos(o)..pos(o + 3));
        assert_eq!(state.root_pos.0, pos(3));
        assert_eq!(state.path.steps(), &[step(ababa, o, aba)]);
        assert_eq!(SpanCoverage::of(&state.span, ababa.width), coverage);
    }

    // `ababa@0` continues in the overlapping `aba@2`: `b@1`, then `a@2`.
    let mut state = exhausted.parent_state();
    state.path_raise(ababa, pos(0));
    let mut state = state.advance_state(graph).unwrap().child_state;
    assert_eq!(state.path.end.steps(), &[step(ababa, 2, aba), step(aba, 1, b)]);
    state.match_end_leaf();
    assert_eq!(state.path.coverage(), SpanCoverage::Prefix);
    let mut state = state.advance_state(graph).unwrap();
    assert_eq!(state.path.end.steps(), &[step(ababa, 2, aba), step(aba, 2, a)]);
    state.match_end_leaf();
    assert_eq!(state.path.span, pos(0)..pos(5));
    assert_eq!(state.path.coverage(), SpanCoverage::EntireRoot);
    assert!(state.clone().advance_state(graph).is_err());
    // Canonical end path: the largest token ending at the match end, `aba@2`.
    state.path.canonicalize::<End, _>(graph);
    assert_eq!(state.path.end.steps(), &[step(ababa, 2, aba)]);
    assert_eq!(state.path.end.leaf(), aba);

    // `a@2` in `aba` (match not at `aba`'s start): the step is prepended.
    let seed = a.into_parent_state(aba, pos(2));
    let seed = seed.advance_state(graph).unwrap_err();
    for (o, coverage) in [(0, SpanCoverage::Range), (2, SpanCoverage::Postfix)] {
        let mut state = seed.clone();
        state.path_raise(ababa, pos(o));
        assert_eq!(state.span, pos(o + 2)..pos(o + 3));
        assert_eq!(state.root_pos.0, pos(1));
        assert_eq!(state.path.steps(), &[step(aba, 2, a), step(ababa, o, aba)]);
        assert_eq!(state.path.leaf(), a);
        assert_eq!(state.path.leaf_offset(), pos(o + 2));
        assert_eq!(SpanCoverage::of(&state.span, ababa.width), coverage);
    }
}

#[test]
fn span_coverage_classification() {
    let width = context_trace::TokenWidth(5);
    for (s, e, expected) in [
        (0, 5, SpanCoverage::EntireRoot),
        (0, 3, SpanCoverage::Prefix),
        (2, 5, SpanCoverage::Postfix),
        (1, 4, SpanCoverage::Range),
    ] {
        assert_eq!(SpanCoverage::of(&(pos(s)..pos(e)), width), expected);
    }
}
