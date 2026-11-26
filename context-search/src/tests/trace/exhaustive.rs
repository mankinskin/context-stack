//! Exhaustive walks over every fixture: trace states of every seed and
//! child advance against `start_at`.

use std::collections::BTreeMap;

use pretty_assertions::assert_eq;

use super::{
    chars,
    keys,
    pos,
    trace,
};
use crate::{
    ChildState,
    DownKey,
    End,
    EndPath,
    IntoParentState,
    ParentState,
    SpanCoverage,
    StateAdvance,
    Traceable,
    TraceCache,
    TraceCtx,
    UpKey,
};
use context_trace::{
    graph::vertex::VertexIndex,
    positioned::{
        fixtures::{
            fixture_graphs,
            FixtureGraph,
        },
        GraphRead,
        Navigate,
    },
    Token,
    ToToken,
};

/// Checks of one exhaustive walk: every seed `(token, parent, offset)` of
/// every fixture, matched to the root end in every root reached by raising.
struct Walk<'f> {
    fixture: &'f FixtureGraph,
    keys: BTreeMap<VertexIndex, String>,
    clamped: usize,
    edges: usize,
}

impl Walk<'_> {
    fn key(
        &self,
        token: impl context_trace::HasVertexIndex<Index = context_trace::CommittedIndex>,
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
        assert_eq!(state.path.leaf_offset(), state.span.start, "{context}");
        assert_eq!(state.path.root_child_end(), state.span.end, "{context}");
        assert!(graph.vertex(root).unwrap().contains_path(state.path.top_down()), "{context}");
        // Canonical start path: it stops at the largest climbed token
        // starting at the match start, and no intermediate token starts there.
        let expected_start = climbed
            .iter()
            .filter(|(_, at)| *at == s)
            .max_by_key(|(t, _)| t.width)
            .unwrap()
            .0;
        assert_eq!(state.path.leaf(), expected_start, "{context}");
        let mut at = 0;
        for step in state.path.top_down() {
            at += step.offset.0;
            if step.child != state.path.leaf().index {
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
                crate::PostfixCommand {
                    path: state.path.clone(),
                    entry_pos: state.root_pos,
                }
                .trace(&mut ctx);
                self.check_edges(&ctx.cache, &context);
                state
            },
        };
        let matched: String = self.substr(root, s..root.width.0);
        for (parent, o) in graph.vertex(root).into_iter().flat_map(|v| v.parent_occurrences()) {
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
            let (leaf, steps) = graph.vertex(root).and_then(|v| v.start_at(p)).unwrap();
            assert_eq!(state.path.end.leaf(), leaf, "{context}");
            assert_eq!(state.path.end.steps(), &steps[..], "{context}");
            assert_eq!(state.path.end.leaf_offset(), p, "{context}");
            let exit = state.path.end.root_child_offset().0;
            if exit < state.path.span.start.0 {
                self.clamped += 1;
            }
            assert_eq!(
                state.exit_pos.0,
                pos(exit.max(state.path.span.start.0) - state.path.span.start.0),
                "{context}"
            );
            state.match_end_leaf();
            let mut canonical = state.path.clone();
            canonical.canonicalize::<End, _>(graph);
            let end = &canonical.end;
            assert!(graph.vertex(root).unwrap().contains_path(end.top_down()), "{context}");
            assert_eq!(end.leaf_end(), canonical.span.end, "{context}");
            for (i, step) in end.steps().iter().enumerate().take(end.steps().len() - 1) {
                let at: usize = end.steps()[..=i].iter().map(|s| s.offset.0).sum();
                let width = graph.vertex(step.child).map(|v| v.to_token()).unwrap().width.0;
                assert_ne!(at + width, canonical.span.end.0, "{context}: not maximal");
            }
            let cache = trace(graph, &state);
            self.check_edges(&cache, &context);
            // Cache entries for every vertex on both paths.
            let start = &state.path.start;
            let end = &state.path.end;
            assert!(cache.exists(&UpKey::new(root, state.entry_pos).into()), "{context}");
            assert!(cache.exists(&DownKey::new(root, state.exit_pos).into()), "{context}");
            for step in start.bottom_up().chain(end.steps()) {
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
                graph.vertex(child.index).into_iter().flat_map(|v| v.parent_occurrences())
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
    for fixture in fixture_graphs() {
        let mut walk = Walk {
            keys: keys(&fixture),
            fixture: &fixture,
            clamped: 0,
            edges: 0,
        };
        for token in fixture.names.values() {
            for (parent, offset) in fixture.graph.vertex(*token).unwrap().parent_occurrences() {
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
    for fixture in fixture_graphs() {
        let graph = &fixture.graph;
        for token in fixture.names.values() {
            for (parent, offset) in graph.vertex(*token).unwrap().parent_occurrences() {
                let Ok(mut state) = token.into_parent_state(parent, offset).advance_state(graph)
                else {
                    continue;
                };
                // Advance by single atoms, decomposing the end leaf first.
                loop {
                    while state.path.end.push_border_child(graph) {}
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
