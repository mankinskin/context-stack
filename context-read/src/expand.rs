//! Root expansion (reader plan §3.2; legacy `expand_root`).

use context_insert::insert_next_match;
use context_trace::{
    Token,
    positioned::{
        GraphRead,
        Navigate,
        PositionedGraphRef,
    },
};

use crate::{
    forward::find_complete,
    root::Span,
};

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
    // Every postfix token of the root, largest first (§3.2).
    let chain: Vec<Token> = graph
        .read()
        .vertex(root)
        .into_iter()
        .flat_map(|vertex| vertex.postfixes().map(|(_, postfix)| postfix))
        .collect();
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
    use super::*;
    use crate::tests::fixture;

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
