//! Root expansion (reader plan §3.2; legacy `expand_root`).

use context_insert::insert_next_match;
use context_trace::{
    Token,
    positioned::{
        PositionedGraph,
        PositionedGraphRef,
    },
};

use crate::{
    forward::find_complete,
    root::Span,
};

/// The postfix tokens of `vertex`, largest first: the chain of last
/// children down to an atom. With positioned compositions this is every
/// postfix token of `vertex` (§3.2); `vertex` itself is not yielded.
pub(crate) fn postfixes(
    graph: &PositionedGraph,
    vertex: Token,
) -> Vec<Token> {
    let mut chain = Vec::new();
    let mut current = vertex;
    while let Some(composition) =
        graph.vertex(current.index).and_then(|v| v.composition())
    {
        let width = graph.vertex(current.index).map_or(0, |v| v.width().0);
        current = composition
            .ending_at(width)
            .expect("a composition reaches its width")
            .child;
        chain.push(current);
    }
    chain
}

/// An accepted probe and the span committing it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Accepted {
    pub(crate) postfix: Token,
    pub(crate) expansion: Token,
    pub(crate) span: Span,
}

/// The first postfix `P` of `root` (largest first) whose match
/// `[P] ++ rest` extends beyond `P`. The match is inserted as the expansion
/// `E`; rejected probes do not edit the graph.
///
/// No probe at `cursor == 0` (the start of a known run): the root then ends
/// with an atom first seen in this read, which occurs only there, so every
/// probe would be rejected (reader plan §7, Q3).
///
/// If `R[0, o) + E` is already a token, the root adopts it.
pub(crate) fn expand(
    graph: &PositionedGraphRef,
    root: Token,
    rest: &[Token],
    cursor: usize,
) -> Option<Accepted> {
    if cursor == 0 || rest.is_empty() {
        return None;
    }
    let chain = postfixes(&graph.read(), root);
    for postfix in chain {
        let mut query = Vec::with_capacity(rest.len() + 1);
        query.push(postfix);
        query.extend_from_slice(rest);
        let expansion = insert_next_match(graph, query)
            .unwrap_or_else(|error| panic!("expansion probe failed: {}", error))
            .token();
        if expansion.width <= postfix.width {
            continue;
        }
        let consumed = expansion.width.0 - postfix.width.0;
        assert!(consumed <= rest.len(), "expansion beyond the run");
        let mut union = Vec::with_capacity(consumed + 1);
        union.push(root);
        union.extend_from_slice(&rest[..consumed]);
        let span = match find_complete(graph, union) {
            Some(token) => Span::ReplaceRoot(token),
            None => Span::Expand {
                postfix,
                expansion,
            },
        };
        return Some(Accepted {
            postfix,
            expansion,
            span,
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::tests::fixture;
    use context_trace::positioned::fixtures::fixture_graphs;

    /// On every fixture graph, `postfixes(v)` is every vertex whose string
    /// is a proper suffix of `v`'s, largest first.
    #[test]
    fn postfixes_are_all_postfix_tokens() {
        for fixture in fixture_graphs() {
            let keys: BTreeMap<_, _> = fixture
                .names
                .iter()
                .map(|(key, token)| (token.index, key.as_str()))
                .collect();
            for (key, token) in &fixture.names {
                let mut expected: Vec<&str> = fixture
                    .names
                    .keys()
                    .filter(|other| other.len() < key.len() && key.ends_with(other.as_str()))
                    .map(String::as_str)
                    .collect();
                expected.sort_by_key(|other| std::cmp::Reverse(other.len()));
                let found: Vec<&str> = postfixes(&fixture.graph, *token)
                    .into_iter()
                    .map(|postfix| keys[&postfix.index])
                    .collect();
                assert_eq!(found, expected, "{}: postfixes of {}", fixture.name, key);
            }
        }
    }

    /// A rejected probe leaves the graph unchanged.
    #[test]
    fn rejected_probe_does_not_edit() {
        let (graph, names) = fixture("linear_abc");
        let generation = graph.read().generation();
        assert_eq!(expand(&graph, names["abc"], &[names["a"]], 3), None);
        assert_eq!(graph.read().generation(), generation);
    }

    /// No probe at the start of a run.
    #[test]
    fn no_probe_at_run_start() {
        let (graph, names) = fixture("two_chars");
        assert_eq!(expand(&graph, names["a"], &[names["b"]], 0), None);
    }

    /// An accepted probe returns the expansion, or the existing token of the
    /// root extended by it.
    #[test]
    fn accepted_probe() {
        let (graph, names) = fixture("closure_forward_seed");
        let (ab, b, c) = (names["ab"], names["b"], names["c"]);
        assert_eq!(
            expand(&graph, ab, &[c], 1).map(|accepted| accepted.span),
            Some(Span::Expand {
                postfix: b,
                expansion: names["bc"],
            })
        );
        let (graph, names) = fixture("canonical_closure");
        let abc = names["abc"];
        let generation = graph.read().generation();
        let span = expand(&graph, names["ab"], &[names["c"]], 1).map(|accepted| accepted.span);
        assert_eq!(span, Some(Span::ReplaceRoot(abc)));
        assert_eq!(graph.read().generation(), generation);
    }
}
