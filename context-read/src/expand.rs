//! Root expansion (reader plan §3.2; legacy `expand_root`).

use context_insert::{
    InsertOutcome,
    insert_next_match,
};
use crate::stage::StageHandle;
use context_trace::{
    positioned::{
        GraphRead,
        HasGraph,
        Navigate,
        Publication,
    },
    StageIndex,
    Token,
};

use crate::{
    forward::find_complete,
    root::Span,
};

/// An accepted probe, the span committing it and the edit record of the
/// insertion, if it created the expansion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Accepted {
    pub(crate) postfix: Token<StageIndex>,
    pub(crate) expansion: Token<StageIndex>,
    pub(crate) span: Span,
    pub(crate) publication: Option<Publication<StageIndex>>,
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
pub(crate) fn expand<H: StageHandle>(
    handle: &mut H,
    root: Token<StageIndex>,
    rest: &[Token<StageIndex>],
    cursor: usize,
) -> Option<Accepted> {
    if cursor == 0 || rest.is_empty() {
        return None;
    }
    // Every postfix token of the root, largest first (§3.2).
    let chain: Vec<Token<StageIndex>> = handle
        .graph()
        .vertex(root)
        .into_iter()
        .flat_map(|vertex| vertex.postfixes().map(|(_, postfix)| postfix))
        .collect();
    for postfix in chain {
        let mut query = Vec::with_capacity(rest.len() + 1);
        query.push(postfix);
        query.extend_from_slice(rest);
        let outcome = insert_next_match(&mut *handle, query)
            .unwrap_or_else(|error| panic!("expansion probe failed: {}", error));
        let expansion = outcome.token();
        if expansion.width <= postfix.width {
            debug_assert!(
                !matches!(outcome, InsertOutcome::Created { .. }),
                "a rejected probe created a token"
            );
            continue;
        }
        let publication = match outcome {
            InsertOutcome::Created { publication, .. } => Some(publication),
            _ => None,
        };
        let consumed = expansion.width.0 - postfix.width.0;
        assert!(consumed <= rest.len(), "expansion beyond the run");
        let mut union = Vec::with_capacity(consumed + 1);
        union.push(root);
        union.extend_from_slice(&rest[..consumed]);
        let span = match find_complete(handle, union) {
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
            publication,
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use crate::stage::overlay::OverlayHandle;

    use super::*;
    use crate::tests::fixture;

    /// A rejected probe leaves the graph unchanged.
    #[test]
    fn rejected_probe_does_not_edit() {
        let (graph, names) = fixture("linear_abc");
        let mut handle = OverlayHandle::open(&graph);
        assert_eq!(
            expand(&mut handle, names["abc"].into(), &[names["a"].into()], 3),
            None
        );
        assert_eq!(handle.graph().generation().stage, 0);
    }

    /// No probe at the start of a run.
    #[test]
    fn no_probe_at_run_start() {
        let (graph, names) = fixture("two_chars");
        let mut handle = OverlayHandle::open(&graph);
        assert_eq!(
            expand(&mut handle, names["a"].into(), &[names["b"].into()], 0),
            None
        );
    }

    /// An accepted probe returns the expansion, or the existing token of the
    /// root extended by it.
    #[test]
    fn accepted_probe() {
        let (graph, names) = fixture("closure_forward_seed");
        let mut handle = OverlayHandle::open(&graph);
        let (ab, b, c) = (names["ab"].into(), names["b"].into(), names["c"].into());
        assert_eq!(
            expand(&mut handle, ab, &[c], 1).map(|accepted| accepted.span),
            Some(Span::Expand {
                postfix: b,
                expansion: names["bc"].into(),
            })
        );
        let (graph, names) = fixture("canonical_closure");
        let mut handle = OverlayHandle::open(&graph);
        let span = expand(&mut handle, names["ab"].into(), &[names["c"].into()], 1)
            .map(|accepted| accepted.span);
        assert_eq!(span, Some(Span::ReplaceRoot(names["abc"].into())));
        assert_eq!(handle.graph().generation().stage, 0);
    }
}
