//! Query paths through composite tokens, checked against the query string.

use std::{
    collections::BTreeMap,
    ops::ControlFlow,
};

use pretty_assertions::assert_eq;

use super::{
    chars,
    fixture,
    keys,
};
use crate::{
    Token,
    graph::vertex::VertexIndex,
    positioned::{
        GraphRead,
        Navigate,
        QueryPath,
        fixtures::{
            FixtureGraph,
            fixture_graphs,
        },
    },
};

/// Walk a query of fixture tokens, decomposing the leaf `depth(i)` times
/// before each advance; every leaf is checked against the query string.
fn walk_query(
    fixture: &FixtureGraph,
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
        let p = path.leaf_offset().0;
        let leaf = chars(&keys[&path.leaf().index]);
        assert_eq!(&query[p..p + leaf.len()], &leaf[..], "{context} @ {p}");
        // The leaf is `start_at(pattern[entry], local)` or the token itself.
        let token = pattern[path.entry()];
        if path.steps().is_empty() {
            assert_eq!(path.leaf(), token);
            if path.entry() > 0 {
                entry_start = p;
            }
        }
        for _ in 0..depth(i) {
            let before = path.leaf();
            let split = path.push_border_child(graph);
            assert_eq!(split, before.width.0 > 1, "{context}");
            if split {
                let prefix = chars(&keys[&path.leaf().index]);
                assert!(prefix.len() < before.width.0);
                assert_eq!(&query[p..p + prefix.len()], &prefix[..], "{context}");
            }
        }
        if !path.steps().is_empty() {
            let steps = path.steps();
            assert_eq!(steps[0].parent, token.index, "{context}");
            assert!(graph.vertex(token).unwrap().contains_path(steps), "{context}");
            let offset: usize = steps.iter().map(|s| s.offset.0).sum();
            assert_eq!(entry_start + offset, p, "{context}");
        }
        let end = path.leaf_end();
        let entry = path.entry();
        match path.advance_leaf(graph) {
            ControlFlow::Continue(()) => {
                assert_eq!(path.leaf_offset(), end, "{context}");
                let local = end.0 - entry_start;
                if path.entry() == entry {
                    let (expected, _) = graph.vertex(token).unwrap().start_at(local).unwrap();
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
    for fixture in fixture_graphs() {
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
        while path.push_border_child(graph) {}
        atoms.push_str(&keys[&path.leaf().index]);
        if path.advance_leaf(graph).is_break() {
            break;
        }
    }
    assert_eq!(atoms, "abcdexbcdy");
    assert!(QueryPath::new(Vec::<Token>::new()).is_none());
}
