//! Internal cursor, commit, and closure contracts, not reader graph specifications.
//! Graph seeds come from checked-in oracle fixtures. Whole-graph semantics are
//! covered separately through the public reader API.

use super::oracle::{load, seed, token, GraphData};
use crate::{
    expansion::{closure, ExpansionCtx},
    pipeline::root::{RootManager, Span},
};
use context_insert::legacy::ToInsertCtx;
use context_trace::legacy::*;

#[test]
fn cursor_returns_existing_prefix_and_preserves_query_path() {
    let graph = seed("cursor_seed");
    let a = token(&graph, "a");
    let b = token(&graph, "b");
    let ab = token(&graph, "ab");
    let before = GraphData::from_graph(&graph).unwrap();
    let result = ToInsertCtx::<IndexWithPath>::insert_next_match(&graph, vec![a, b, a, b])
        .unwrap()
        .into_result();
    assert_eq!(result.index, ab);
    assert_eq!(result.path.path_root().as_ref(), &[a, b, a, b]);
    assert_eq!(GraphData::from_graph(&graph).unwrap(), before);
}

#[test]
fn cursor_finds_complete_repeated_token() {
    for (name, text) in [
        ("repeat_aa", "aa"),
        ("repeat_aaa", "aaa"),
        ("repeat_aaaa", "aaaa"),
    ] {
        let graph = seed(name);
        let a = token(&graph, "a");
        let result = ToInsertCtx::<IndexWithPath>::insert_next_match(&graph, vec![a; text.len()])
            .unwrap()
            .into_result();
        assert_eq!(result.index, token(&graph, text), "{name}");
    }
}

#[test]
fn cursor_path_tracks_advancement_and_exhaustion() {
    use std::ops::ControlFlow;
    let graph = seed("cursor_seed");
    let a = token(&graph, "a");
    let b = token(&graph, "b");
    let mut path = PatternRangePath::from(Pattern::from(vec![a, b]));
    assert_eq!(HasRootChildIndex::<Start>::root_child_index(&path), 0);
    assert_eq!(HasRootChildIndex::<End>::root_child_index(&path), 0);
    assert_eq!(
        MoveRootIndex::<Right, End>::move_root_index(&mut path, &graph),
        ControlFlow::Continue(())
    );
    assert_eq!(HasRootChildIndex::<End>::root_child_index(&path), 1);
    assert_eq!(HasRootChildIndex::<Start>::root_child_index(&path), 0);
    assert_eq!(
        MoveRootIndex::<Right, End>::move_root_index(&mut path, &graph),
        ControlFlow::Break(())
    );
}

#[test]
fn cursor_empty_query_is_rejected() {
    let graph = seed("cursor_seed");
    assert!(ToInsertCtx::<IndexWithPath>::insert_next_match(&graph, Vec::<Token>::new()).is_err());
}

#[test]
fn cursor_single_atom_returns_single_index() {
    let graph = seed("cursor_seed");
    let a = token(&graph, "a");
    match ToInsertCtx::<IndexWithPath>::insert_next_match(&graph, vec![a]) {
        Err(ErrorReason::SingleIndex(result)) => assert_eq!(result.index, a),
        other => panic!("expected SingleIndex, got {:?}", other),
    }
}

#[test]
fn expansion_commits_forward_match_before_overlap() {
    let fixture = load("canonical_closure");
    let graph = seed("closure_forward_seed");
    let mut sink = RootManager::new(graph.clone());
    let mut expansion = ExpansionCtx::new(
        graph.clone(),
        "abc"
            .chars()
            .map(|c| token(&graph, &c.to_string()))
            .collect::<Vec<_>>(),
    );
    let ab = token(&graph, "ab");
    assert_eq!(expansion.next(&sink), Some(Span::Append(ab)));
    assert!(sink.root.is_none());
    expansion.observe_commit(sink.commit(Span::Append(ab)));
    let span = expansion.next(&sink).unwrap();
    assert!(matches!(span, Span::ReplaceRoot { previous, .. } if previous == ab));
    assert_eq!(sink.root, Some(ab), "preparation must not commit the span");
    expansion.observe_commit(sink.commit(span));
    assert_eq!(expansion.next(&sink), None);
    assert_eq!(GraphData::from_graph(&graph).unwrap(), fixture.graph);
}

#[test]
fn closure_uses_retained_nested_anchor_path() {
    let fixture = load("nested_closure");
    let graph = seed("closure_nested_seed");
    let abc = token(&graph, "abc");
    let mut sink = RootManager::new(graph.clone());
    sink.append_completed(abc);
    let accepted = closure::accept(&graph, abc, token(&graph, "c"), token(&graph, "cd"));
    assert_eq!(accepted.anchor_path.len(), 2);
    let span = closure::prepare(&graph, &sink, accepted).apply(&graph);
    assert_eq!(sink.root, Some(abc));
    sink.commit(span);
    assert_eq!(GraphData::from_graph(&graph).unwrap(), fixture.graph);
}

#[test]
fn closure_reuses_existing_canonical_result() {
    let graph = seed("canonical_closure");
    let ab = token(&graph, "ab");
    let abc = token(&graph, "abc");
    let before = GraphData::from_graph(&graph).unwrap();
    let mut sink = RootManager::new(graph.clone());
    sink.append_completed(ab);
    let accepted = closure::accept(&graph, ab, token(&graph, "b"), token(&graph, "bc"));
    let span = closure::prepare(&graph, &sink, accepted).apply(&graph);
    assert_eq!(
        span,
        Span::ReplaceRoot {
            previous: ab,
            token: abc
        }
    );
    sink.commit(span);
    assert_eq!(GraphData::from_graph(&graph).unwrap(), before);
}

#[test]
fn whole_tail_replacement_skips_unused_complement_insertion() {
    let fixture = load("tail_replacement");
    let graph = seed("closure_tail_seed");
    assert!(!GraphData::from_graph(&graph)
        .unwrap()
        .vertices
        .contains_key("cd"));
    let ab = token(&graph, "ab");
    let abcd = token(&graph, "abcd");
    let mut sink = RootManager::new(graph.clone());
    sink.append_completed(token(&graph, "e"));
    sink.append_completed(ab);
    let previous = sink.root.unwrap();
    let count = graph.vertex_count();
    let accepted = closure::accept(&graph, previous, ab, abcd);
    let span = closure::prepare(&graph, &sink, accepted).apply(&graph);
    assert_eq!(
        span,
        Span::ReplaceTail {
            previous,
            old: ab,
            token: abcd
        }
    );
    sink.commit(span);
    assert_eq!(graph.vertex_count(), count);
    assert_eq!(sink.root.unwrap().vertex_index(), previous.vertex_index());
    assert_eq!(GraphData::from_graph(&graph).unwrap(), fixture.graph);
}

#[test]
fn accepted_materialization_rechecks_root_editability() {
    let graph = seed("linear_abc");
    let abc = token(&graph, "abc");
    let mut sink = RootManager::new(graph.clone());
    sink.append_completed(abc);
    let mut expansion = ExpansionCtx::new(
        graph.clone(),
        "abca"
            .chars()
            .map(|c| token(&graph, &c.to_string()))
            .collect::<Vec<_>>(),
    );
    let span = expansion.next(&sink).unwrap();
    expansion.observe_commit(sink.commit(span));
    let previous = sink.root.unwrap();
    assert!(sink.is_editable(previous));
    let span = expansion.next(&sink).unwrap();
    assert!(!sink.is_editable(previous));
    assert!(matches!(span, Span::ReplaceRoot { previous: root, .. } if root == previous));
    expansion.observe_commit(sink.commit(span));
    assert_eq!(graph.index_string(sink.root.unwrap()), "abcabca");
}

#[test]
fn rejected_root_probes_do_not_materialize() {
    let graph = seed("cursor_seed");
    let mut sink = RootManager::new(graph.clone());
    let mut expansion = ExpansionCtx::new(
        graph.clone(),
        "abc"
            .chars()
            .map(|c| token(&graph, &c.to_string()))
            .collect::<Vec<_>>(),
    );
    let span = expansion.next(&sink).unwrap();
    expansion.observe_commit(sink.commit(span));
    let before = GraphData::from_graph(&graph).unwrap();
    assert_eq!(
        expansion.next(&sink),
        Some(Span::Append(token(&graph, "c")))
    );
    assert_eq!(GraphData::from_graph(&graph).unwrap(), before);
}

#[test]
fn sink_does_not_mutate_adopted_token() {
    let graph = seed("cursor_seed");
    let ab = token(&graph, "ab");
    let before = graph.expect_child_patterns(ab);
    let mut sink = RootManager::new(graph.clone());
    sink.append_completed(ab);
    sink.append_completed(token(&graph, "c"));
    assert_eq!(graph.expect_child_patterns(ab), before);
    assert_eq!(graph.index_string(sink.root.unwrap()), "abc");
}

#[test]
#[should_panic(expected = "stale last child")]
fn sink_rejects_stale_tail_child() {
    let graph = seed("cursor_seed");
    let a = token(&graph, "a");
    let c = token(&graph, "c");
    let ab = token(&graph, "ab");
    let mut sink = RootManager::new(graph);
    sink.append_completed(a);
    sink.append_completed(c);
    let root = sink.root.unwrap();
    sink.commit(Span::ReplaceTail {
        previous: root,
        old: a,
        token: ab,
    });
}

#[test]
#[should_panic(expected = "replacement root is stale")]
fn sink_rejects_stale_tail_root() {
    let graph = seed("repeat_aa");
    let a = token(&graph, "a");
    let aa = token(&graph, "aa");
    let mut sink = RootManager::new(graph);
    sink.append_completed(a);
    sink.commit(Span::ReplaceTail {
        previous: aa,
        old: a,
        token: aa,
    });
}

#[test]
#[should_panic(expected = "replacement root is not editable")]
fn sink_rejects_adopted_tail_root() {
    let graph = seed("repeat_aaa");
    let a = token(&graph, "a");
    let aa = token(&graph, "aa");
    let aaa = token(&graph, "aaa");
    let mut sink = RootManager::new(graph);
    sink.append_completed(aa);
    sink.commit(Span::ReplaceTail {
        previous: aa,
        old: a,
        token: aaa,
    });
}

#[test]
#[should_panic(expected = "self-referential tail")]
fn sink_rejects_self_referential_tail() {
    let graph = seed("cursor_seed");
    let a = token(&graph, "a");
    let mut sink = RootManager::new(graph);
    sink.append_completed(a);
    sink.append_completed(a);
    let root = sink.root.unwrap();
    sink.commit(Span::ReplaceTail {
        previous: root,
        old: a,
        token: root,
    });
}
