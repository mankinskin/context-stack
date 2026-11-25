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
use context_insert::InsertOutcome;
use context_trace::{
    StageIndex,
    Token,
    positioned::{
        Publication,
        StageHandle,
    },
};

use crate::root::Span;

/// The longest occurring prefix of `rest` as a token (inserted if it was not
/// one); the root adopts `[root, t]` if that is already a token. Also the
/// edit record of the insertion, if it created `t`.
pub(crate) fn forward<H: StageHandle>(
    handle: &mut H,
    root: Option<Token<StageIndex>>,
    rest: &[Token<StageIndex>],
) -> (Span, Option<Publication<StageIndex>>) {
    let (token, publication) = match insert_next_match(&mut *handle, rest.to_vec()) {
        Ok(InsertOutcome::Created {
            token, publication, ..
        }) => (token, Some(publication)),
        Ok(outcome) => (outcome.token(), None),
        Err(InsertError::Search(SearchError::SingleIndex(atom))) => (atom, None),
        Err(error) => panic!("forward match failed: {}", error),
    };
    assert!(
        token.width.0 > 0 && token.width.0 <= rest.len(),
        "forward match returned an invalid width"
    );
    let span = match root.and_then(|root| find_complete(handle, vec![root, token])) {
        Some(replacement) => Span::ReplaceRoot(replacement),
        None => Span::Append(token),
    };
    (span, publication)
}

/// The existing token whose content is exactly `query`, if any. A lookup:
/// never inserts.
pub(crate) fn find_complete<H: StageHandle>(
    handle: &H,
    query: Vec<Token<StageIndex>>,
) -> Option<Token<StageIndex>> {
    let response = handle
        .find_ancestor(query)
        .unwrap_or_else(|error| panic!("lookup failed: {}", error));
    (response.is_entire_root() && response.query_exhausted())
        .then(|| response.root_token())
}

#[cfg(test)]
mod tests {
    use context_trace::positioned::{
        GraphRead,
        HasGraph,
        OverlayHandle,
    };

    use super::*;
    use crate::tests::fixture;

    /// One atom left: the atom itself, without a search edit.
    #[test]
    fn one_atom_left() {
        let (graph, names) = fixture("linear_abc");
        let mut handle = OverlayHandle::open(&graph);
        let c = names["c"].into();
        assert_eq!(forward(&mut handle, None, &[c]).0, Span::Append(c));
        assert_eq!(handle.graph().generation().stage, 0);
    }

    /// `[R, t]` is adopted iff it is a complete token; a partial match of
    /// `[R, t]` is not inserted.
    #[test]
    fn root_pair_adopted_only_when_complete() {
        let (graph, names) = fixture("two_chars");
        let mut handle = OverlayHandle::open(&graph);
        let (a, b) = (names["a"].into(), names["b"].into());
        assert_eq!(
            forward(&mut handle, Some(a), &[b]).0,
            Span::ReplaceRoot(names["ab"].into())
        );

        let (graph, names) = fixture("linear_abc");
        let mut handle = OverlayHandle::open(&graph);
        let (a, b) = (names["a"].into(), names["b"].into());
        assert_eq!(forward(&mut handle, Some(a), &[b]).0, Span::Append(b));
        assert_eq!(handle.graph().generation().stage, 0);
        assert_eq!(handle.staged_count(), 0);
    }

    /// The longest occurring prefix of the rest is inserted.
    #[test]
    fn longest_prefix_inserted() {
        let (graph, names) = fixture("linear_abc");
        let mut handle = OverlayHandle::open(&graph);
        let (a, b) = (names["a"].into(), names["b"].into());
        let (Span::Append(token), Some(publication)) = forward(&mut handle, None, &[a, b, a])
        else {
            panic!("no root: append a created token");
        };
        assert_eq!(publication.created.len(), 1);
        assert_eq!(publication.created[0].index, token.index);
        assert_eq!(token.width.0, 2);
        assert_eq!(
            find_complete(&handle, vec![a, b]).map(|t| t.index),
            Some(token.index)
        );
        handle.commit(token).unwrap();
        graph.read().validate().unwrap();
    }
}
