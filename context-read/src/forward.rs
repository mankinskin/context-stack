//! Forward match (reader plan §3.3; legacy `match_forward`,
//! `overlap::find_complete`).

use context_insert::{
    InsertError,
    insert_next_match,
};
use context_search::{
    Find,
    SearchError,
};
use context_trace::{
    Token,
    positioned::PositionedGraphRef,
};

use crate::root::Span;

/// The longest occurring prefix of `rest` as a token (inserted if it was not
/// one); the root adopts `[root, t]` if that is already a token.
pub(crate) fn forward(
    graph: &PositionedGraphRef,
    root: Option<Token>,
    rest: &[Token],
) -> Span {
    let token = match insert_next_match(graph, rest.to_vec()) {
        Ok(outcome) => outcome.token(),
        Err(InsertError::Search(SearchError::SingleIndex(atom))) => atom,
        Err(error) => panic!("forward match failed: {}", error),
    };
    assert!(
        token.width.0 > 0 && token.width.0 <= rest.len(),
        "forward match returned an invalid width"
    );
    match root.and_then(|root| find_complete(graph, vec![root, token])) {
        Some(replacement) => Span::ReplaceRoot(replacement),
        None => Span::Append(token),
    }
}

/// The existing token whose content is exactly `query`, if any. A lookup:
/// never inserts.
pub(crate) fn find_complete(
    graph: &PositionedGraphRef,
    query: Vec<Token>,
) -> Option<Token> {
    let response = graph
        .find_ancestor(query)
        .unwrap_or_else(|error| panic!("lookup failed: {}", error));
    (response.is_entire_root() && response.query_exhausted())
        .then(|| response.root_token())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::fixture;

    /// One atom left: the atom itself, without a search edit.
    #[test]
    fn one_atom_left() {
        let (graph, names) = fixture("linear_abc");
        let generation = graph.read().generation();
        let c = names["c"];
        assert_eq!(forward(&graph, None, &[c]), Span::Append(c));
        assert_eq!(graph.read().generation(), generation);
    }

    /// `[R, t]` is adopted iff it is a complete token; a partial match of
    /// `[R, t]` is not inserted.
    #[test]
    fn root_pair_adopted_only_when_complete() {
        let (graph, names) = fixture("two_chars");
        let (a, b) = (names["a"], names["b"]);
        assert_eq!(forward(&graph, Some(a), &[b]), Span::ReplaceRoot(names["ab"]));

        let (graph, names) = fixture("linear_abc");
        let (a, b) = (names["a"], names["b"]);
        let generation = graph.read().generation();
        let count = graph.read().vertex_count();
        assert_eq!(forward(&graph, Some(a), &[b]), Span::Append(b));
        assert_eq!(graph.read().generation(), generation);
        assert_eq!(graph.read().vertex_count(), count);
    }

    /// The longest occurring prefix of the rest is inserted.
    #[test]
    fn longest_prefix_inserted() {
        let (graph, names) = fixture("linear_abc");
        let (a, b) = (names["a"], names["b"]);
        let Span::Append(token) = forward(&graph, None, &[a, b, a]) else {
            panic!("no root: append");
        };
        assert_eq!(token.width.0, 2);
        assert_eq!(
            find_complete(&graph, vec![a, b]).map(|t| t.index),
            Some(token.index)
        );
        graph.read().validate().unwrap();
    }
}
