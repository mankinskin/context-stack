//! Paths, trace states and the trace cache (search plan 2.2, canonical
//! rules 2.4) on the v3 fixtures. Vertices are addressed through the name
//! map; expectations come from the fixture strings.

use std::{
    collections::BTreeMap,
    ops::ControlFlow,
};

use pretty_assertions::assert_eq;

use crate::{
    AtomPosition,
    DirectedKey,
    DownKey,
    Token,
    UpKey,
    graph::vertex::VertexIndex,
    positioned::{
        ChildState,
        EndPath,
        IntoParentState,
        OccurrenceLocation,
        ParentState,
        PositionedGraph,
        QueryPath,
        RangeCommand,
        SpanCoverage,
        StateAdvance,
        TraceCache,
        TraceCtx,
        Traceable,
        fixtures::{
            V3Graph,
            v3_graphs,
        },
    },
};

fn chars(s: &str) -> Vec<char> {
    s.chars().collect()
}

fn fixture(name: &str) -> V3Graph {
    v3_graphs()
        .into_iter()
        .find(|f| f.name == name)
        .unwrap_or_else(|| panic!("fixture {name}"))
}

/// Vertex index → fixture key (`Token`'s `Ord` compares widths only).
fn keys(fixture: &V3Graph) -> BTreeMap<VertexIndex, String> {
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
fn match_to_root_end<G: crate::GraphKind>(
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
    edges.sort_by_key(|(p, c, o)| (p.index.index, *crate::HasAtomPosition::pos(p), c.index.index, *o));
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
        assert_eq!(state.path.bottom_up(), &[step(abcabca, o, abca)]);
        assert_eq!(state.path.start(), abca);
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
    state.path.simplify(graph);
    assert_eq!(state.path.end.steps(), &end);

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
    expected.sort_by_key(|(p, c, o)| (p.index.index, *crate::HasAtomPosition::pos(p), c.index.index, *o));
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
        assert_eq!(state.path.bottom_up(), &[step(ababa, o, aba)]);
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
    // Canonical end path: stop at `aba@2`, which ends at the match end.
    state.path.simplify(graph);
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
        assert_eq!(state.path.bottom_up(), &[step(aba, 2, a), step(ababa, o, aba)]);
        assert_eq!(state.path.start(), a);
        assert_eq!(state.path.start_offset(), pos(o + 2));
        assert_eq!(SpanCoverage::of(&state.span, ababa.width), coverage);
    }
}

#[test]
fn span_coverage_classification() {
    let width = crate::TokenWidth(5);
    for (s, e, expected) in [
        (0, 5, SpanCoverage::EntireRoot),
        (0, 3, SpanCoverage::Prefix),
        (2, 5, SpanCoverage::Postfix),
        (1, 4, SpanCoverage::Range),
    ] {
        assert_eq!(SpanCoverage::of(&(pos(s)..pos(e)), width), expected);
    }
}

/// Checks of one exhaustive walk: every seed `(token, parent, offset)` of
/// every fixture, matched to the root end in every root reached by raising.
struct Walk<'f> {
    fixture: &'f V3Graph,
    keys: BTreeMap<VertexIndex, String>,
    clamped: usize,
    edges: usize,
}

impl Walk<'_> {
    fn key(
        &self,
        token: impl crate::HasVertexIndex,
    ) -> &str {
        &self.keys[&token.vertex_index()]
    }
    fn substr(
        &self,
        token: Token,
        range: std::ops::Range<usize>,
    ) -> String {
        chars(self.key(token))[range].iter().collect()
    }
    /// `climbed`: tokens raised through with their absolute start in the
    /// current root (the seed first).
    fn parent_state(
        &mut self,
        state: ParentState,
        matched: &str,
        climbed: &[(Token, usize)],
    ) {
        let graph = &self.fixture.graph;
        let root = state.root();
        let (s, e) = (state.span.start.0, state.span.end.0);
        let context = format!(
            "{}: {:?} in {:?} [{s}, {e})",
            self.fixture.name,
            matched,
            self.key(root)
        );
        assert_eq!(self.substr(root, s..e), matched, "{context}");
        assert_eq!(state.anchor().query, pos(0), "{context}");
        assert_eq!(state.path.start_offset(), state.span.start, "{context}");
        assert_eq!(state.path.entry_end(), state.span.end, "{context}");
        assert!(graph.contains_path(&state.path.to_occurrence_path()), "{context}");
        // Canonical start path: it stops at the largest climbed token
        // starting at the match start, and no intermediate token starts there.
        let expected_start = climbed
            .iter()
            .filter(|(_, at)| *at == s)
            .max_by_key(|(t, _)| t.width)
            .unwrap()
            .0;
        assert_eq!(state.path.start(), expected_start, "{context}");
        let mut at = 0;
        for step in state.path.top_down() {
            at += step.offset.0;
            if step.child != state.path.start().index {
                assert_ne!(at, s, "{context}: intermediate token at the match start");
            }
        }
        let expected = match (s == 0, e == chars(self.key(root)).len()) {
            (true, true) => SpanCoverage::EntireRoot,
            (true, false) => SpanCoverage::Prefix,
            (false, true) => SpanCoverage::Postfix,
            (false, false) => SpanCoverage::Range,
        };
        assert_eq!(SpanCoverage::of(&state.span, root.width), expected, "{context}");

        let exhausted = match state.clone().advance_state(graph) {
            Ok(entered) => {
                assert_eq!(entered.root_parent, state, "{context}");
                self.child_state(entered.child_state, &context)
            },
            Err(state) => {
                assert_eq!(e, root.width.0, "{context}");
                // A state at the root end; trace its start path alone.
                let mut ctx = TraceCtx::new(graph, TraceCache::default());
                crate::positioned::PostfixCommand {
                    path: state.path.clone(),
                    entry_pos: state.root_pos,
                }
                .trace(&mut ctx);
                self.check_edges(&ctx.cache, &context);
                state
            },
        };
        let matched: String = self.substr(root, s..root.width.0);
        for (parent, o) in graph.parent_occurrences(root) {
            let mut raised = exhausted.clone();
            raised.path_raise(parent, o);
            let o = o.0;
            assert_eq!(self.substr(parent, o..o + root.width.0), self.key(root), "{context}");
            assert_eq!(raised.span, pos(o + s)..pos(o + root.width.0), "{context}");
            let mut climbed: Vec<(Token, usize)> =
                climbed.iter().map(|&(t, at)| (t, at + o)).collect();
            climbed.push((root, o));
            self.parent_state(raised, &matched, &climbed);
        }
    }
    /// Match to the root end, checking every end path against `start_at`;
    /// returns the start path as a parent state for raising.
    fn child_state(
        &mut self,
        mut state: ChildState,
        context: &str,
    ) -> ParentState {
        let graph = &self.fixture.graph;
        let root = state.root();
        loop {
            let p = state.path.span.end;
            let context = format!("{context} @ {p}");
            let (leaf, steps) = graph.start_at(root, p).unwrap();
            assert_eq!(state.path.end.leaf(), leaf, "{context}");
            assert_eq!(state.path.end.steps(), &steps[..], "{context}");
            assert_eq!(state.path.end.leaf_offset(), p, "{context}");
            let exit = state.path.end.root_exit().offset.0;
            if exit < state.path.span.start.0 {
                self.clamped += 1;
            }
            assert_eq!(
                state.exit_pos.0,
                pos(exit.max(state.path.span.start.0) - state.path.span.start.0),
                "{context}"
            );
            state.match_end_leaf();
            let mut simplified = state.path.clone();
            simplified.simplify(graph);
            let end = &simplified.end;
            assert!(graph.contains_path(&end.to_occurrence_path()), "{context}");
            assert_eq!(end.leaf_end(), simplified.span.end, "{context}");
            for (i, step) in end.steps().iter().enumerate().take(end.steps().len() - 1) {
                let at: usize = end.steps()[..=i].iter().map(|s| s.offset.0).sum();
                let width = graph.token(step.child).unwrap().width.0;
                assert_ne!(at + width, simplified.span.end.0, "{context}: not maximal");
            }
            let cache = trace(graph, &state);
            self.check_edges(&cache, &context);
            // Cache entries for every vertex on both paths.
            let start = &state.path.start;
            let end = &state.path.end;
            assert!(cache.exists(&UpKey::new(root, state.entry_pos).into()), "{context}");
            assert!(cache.exists(&DownKey::new(root, state.exit_pos).into()), "{context}");
            for step in start.bottom_up().iter().chain(end.steps()) {
                assert!(cache.exists_vertex(step.child), "{context}: {step:?}");
            }
            match state.advance_state(graph) {
                Ok(next) => state = next,
                Err(end) => return end.parent_state(),
            }
        }
    }
    /// Every edge is a composition occurrence (parent link) and agrees
    /// with the fixture strings.
    fn check_edges(
        &mut self,
        cache: &TraceCache,
        context: &str,
    ) {
        let graph = &self.fixture.graph;
        for (parent, child, offset) in cache.edges() {
            assert!(
                graph
                    .parent_occurrences(child.index)
                    .any(|(p, o)| p.index == parent.index.index && o == offset),
                "{context}: edge {parent:?} -> {child:?} @ {offset}"
            );
            let o = offset.0;
            assert_eq!(
                self.substr(parent.index, o..o + child.index.width.0),
                self.key(child.index),
                "{context}"
            );
            self.edges += 1;
        }
    }
}

#[test]
fn trace_states_on_every_fixture() {
    let (mut clamped, mut edges) = (0, 0);
    for fixture in v3_graphs() {
        let mut walk = Walk {
            keys: keys(&fixture),
            fixture: &fixture,
            clamped: 0,
            edges: 0,
        };
        for token in fixture.names.values() {
            for (parent, offset) in fixture.graph.parent_occurrences(*token) {
                let state = token.into_parent_state(parent, offset);
                let matched = walk.key(*token).to_string();
                walk.parent_state(state, &matched, &[(*token, offset.0)]);
            }
        }
        clamped += walk.clamped;
        edges += walk.edges;
    }
    assert!(edges > 0);
    // Overlap fixtures reach end-path root children starting before the
    // match start (clamped down positions).
    assert!(clamped > 0);
}

#[test]
fn child_advance_state_equals_start_at() {
    for fixture in v3_graphs() {
        let graph = &fixture.graph;
        for token in fixture.names.values() {
            for (parent, offset) in graph.parent_occurrences(*token) {
                let Ok(mut state) = token.into_parent_state(parent, offset).advance_state(graph)
                else {
                    continue;
                };
                // Advance by single atoms, decomposing the end leaf first.
                loop {
                    while state.path.end.push_prefix(graph) {}
                    state.match_end_leaf();
                    let p = state.path.span.end;
                    match state.child_state.clone().advance_state(graph) {
                        Ok(next) => {
                            let expected = EndPath::start_at(graph, parent, p).unwrap();
                            assert_eq!(next.path.end, expected, "{}: {parent:?} @ {p}", fixture.name);
                            state.child_state = next;
                        },
                        Err(_) => {
                            assert_eq!(p.0, parent.width.0);
                            break;
                        },
                    }
                }
            }
        }
    }
}

/// Walk a query of fixture tokens, decomposing the leaf `depth(i)` times
/// before each advance; every leaf is checked against the query string.
fn walk_query(
    fixture: &V3Graph,
    keys: &BTreeMap<VertexIndex, String>,
    pattern: &[Token],
    depth: impl Fn(usize) -> usize,
) {
    let graph = &fixture.graph;
    let query: Vec<char> = pattern.iter().flat_map(|t| chars(&keys[&t.index])).collect();
    let context = format!("{}: {pattern:?}", fixture.name);
    let mut path = QueryPath::new(pattern.to_vec()).unwrap();
    let mut entry_start = 0;
    let mut i = 0;
    loop {
        let p = path.position().0;
        let leaf = chars(&keys[&path.leaf().index]);
        assert_eq!(&query[p..p + leaf.len()], &leaf[..], "{context} @ {p}");
        // The leaf is `start_at(pattern[entry], local)` or the token itself.
        let token = pattern[path.entry()];
        if path.end().is_empty() {
            assert_eq!(path.leaf(), token);
            if path.entry() > 0 {
                entry_start = p;
            }
        }
        for _ in 0..depth(i) {
            let before = path.leaf();
            let split = path.prefix_child(graph);
            assert_eq!(split, before.width.0 > 1, "{context}");
            if split {
                let prefix = chars(&keys[&path.leaf().index]);
                assert!(prefix.len() < before.width.0);
                assert_eq!(&query[p..p + prefix.len()], &prefix[..], "{context}");
            }
        }
        if !path.end().is_empty() {
            let steps = path.end();
            assert_eq!(steps[0].parent, token.index, "{context}");
            assert!(graph.contains_path(
                &crate::positioned::OccurrencePath::new(steps.to_vec()).unwrap()
            ));
            let offset: usize = steps.iter().map(|s| s.offset.0).sum();
            assert_eq!(entry_start + offset, p, "{context}");
        }
        let end = path.leaf_end();
        let entry = path.entry();
        match path.advance(graph) {
            ControlFlow::Continue(()) => {
                assert_eq!(path.position(), end, "{context}");
                let local = end.0 - entry_start;
                if path.entry() == entry {
                    let (expected, _) = graph.start_at(token, local).unwrap();
                    assert_eq!(path.leaf(), expected, "{context} @ {end}");
                } else {
                    assert_eq!(local, token.width.0, "{context}");
                    assert_eq!(path.entry(), entry + 1);
                }
            },
            ControlFlow::Break(()) => {
                assert!(path.at_end(), "{context}");
                assert_eq!(end.0, query.len(), "{context}");
                break;
            },
        }
        i += 1;
    }
}

#[test]
fn query_path_through_composite_tokens() {
    for fixture in v3_graphs() {
        let keys = keys(&fixture);
        let tokens: Vec<Token> = fixture.names.values().copied().collect();
        for &first in &tokens {
            for &second in &tokens {
                for depth in [0, 1, 2, usize::MAX] {
                    walk_query(&fixture, &keys, &[first, second], |i| {
                        if depth == usize::MAX { i % 3 } else { depth }
                    });
                }
            }
        }
    }
}

#[test]
fn query_path_atoms_of_composite() {
    // Fully decomposing every leaf visits the query's atoms in order.
    let fixture = fixture("seam_bcd");
    let graph = &fixture.graph;
    let keys = keys(&fixture);
    let pattern = ["abcde", "xbcdy"].map(|k| fixture.names[k]).to_vec();
    let mut path = QueryPath::new(pattern).unwrap();
    let mut atoms = String::new();
    loop {
        while path.prefix_child(graph) {}
        atoms.push_str(&keys[&path.leaf().index]);
        if path.advance(graph).is_break() {
            break;
        }
    }
    assert_eq!(atoms, "abcdexbcdy");
    assert!(QueryPath::new(Vec::new()).is_none());
}
