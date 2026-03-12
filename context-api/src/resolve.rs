//! TokenRef resolution — resolves `TokenRef` values into concrete `Token`
//! values from the graph.
//!
//! This module is used by the search, insert, and read command modules to
//! convert user-facing token references (by index or label string) into
//! the internal `Token` type used by the algorithm crates.
//!
//! ## Resolution Rules
//!
//! - `TokenRef::Index(n)` → direct vertex lookup by `VertexIndex(n)`.
//! - `TokenRef::Label(s)` where `s.len() == 1` → atom lookup by character.
//! - `TokenRef::Label(s)` where `s.len() > 1` → search for a token whose
//!   atom sequence matches `s` using `Find::find_ancestor`.

use context_trace::{
    VertexSet,
    graph::{
        HypergraphRef,
        kind::BaseGraphKind,
        vertex::{
            VertexIndex,
            atom::Atom,
            token::Token,
        },
    },
};

use context_search::Find;

use crate::{
    error::SearchError,
    types::TokenRef,
};

/// Resolve a single `TokenRef` to a `Token` in the graph.
///
/// # Errors
///
/// - `SearchError::TokenNotFound` if the referenced vertex does not exist.
/// - `SearchError::InternalError` if a search operation fails unexpectedly.
pub fn resolve_token_ref(
    graph: &HypergraphRef<BaseGraphKind>,
    token_ref: &TokenRef,
) -> Result<Token, SearchError> {
    match token_ref {
        TokenRef::Index(idx) => {
            let vi = VertexIndex(*idx);
            let data = graph.get_vertex_data(vi).map_err(|_| {
                SearchError::TokenNotFound {
                    description: format!("no vertex at index {idx}"),
                }
            })?;
            Ok(data.to_token())
        },
        TokenRef::Label(label) => {
            if label.is_empty() {
                return Err(SearchError::TokenNotFound {
                    description: "empty label".to_string(),
                });
            }

            if label.chars().count() == 1 {
                // Single char → atom lookup
                let ch = label.chars().next().unwrap();
                let index =
                    graph.get_atom_index(Atom::Element(ch)).map_err(|_| {
                        SearchError::TokenNotFound {
                            description: format!("no atom for char '{ch}'"),
                        }
                    })?;
                let data = graph.get_vertex_data(index).map_err(|_| {
                    SearchError::TokenNotFound {
                        description: format!(
                            "atom '{ch}' index exists but vertex data missing"
                        ),
                    }
                })?;
                Ok(data.to_token())
            } else {
                // Multi-char → search for the token by its atom sequence
                let response =
                    graph.find_ancestor(label.chars()).map_err(|e| {
                        SearchError::InternalError(format!("{e:?}"))
                    })?;

                if response.is_entire_root() && response.query_exhausted() {
                    Ok(response.root_token())
                } else {
                    Err(SearchError::TokenNotFound {
                        description: format!(
                            "no complete token found for label \"{label}\""
                        ),
                    })
                }
            }
        },
    }
}

/// Resolve a slice of `TokenRef` values to `Token` values.
///
/// Stops at the first resolution failure and returns the error.
pub fn resolve_token_refs(
    graph: &HypergraphRef<BaseGraphKind>,
    refs: &[TokenRef],
) -> Result<Vec<Token>, SearchError> {
    refs.iter().map(|r| resolve_token_ref(graph, r)).collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use context_trace::graph::{
        Hypergraph,
        HypergraphRef,
        kind::BaseGraphKind,
        vertex::atom::Atom,
    };

    fn make_graph() -> HypergraphRef<BaseGraphKind> {
        HypergraphRef::from(Hypergraph::default())
    }

    #[test]
    fn resolve_by_index_existing() {
        let graph = make_graph();
        let token = graph.insert_atom(Atom::Element('a'));
        let resolved =
            resolve_token_ref(&graph, &TokenRef::Index(token.index.0)).unwrap();
        assert_eq!(resolved.index, token.index);
    }

    #[test]
    fn resolve_by_index_missing() {
        let graph = make_graph();
        let result = resolve_token_ref(&graph, &TokenRef::Index(999));
        assert!(result.is_err());
        match result.unwrap_err() {
            SearchError::TokenNotFound { description } => {
                assert!(description.contains("999"));
            },
            other => panic!("expected TokenNotFound, got: {other}"),
        }
    }

    #[test]
    fn resolve_by_single_char_label() {
        let graph = make_graph();
        let token = graph.insert_atom(Atom::Element('x'));
        let resolved =
            resolve_token_ref(&graph, &TokenRef::Label("x".into())).unwrap();
        assert_eq!(resolved.index, token.index);
    }

    #[test]
    fn resolve_by_single_char_label_missing() {
        let graph = make_graph();
        let result = resolve_token_ref(&graph, &TokenRef::Label("z".into()));
        assert!(result.is_err());
    }

    #[test]
    fn resolve_by_multi_char_label_existing() {
        let graph = make_graph();
        let ta = graph.insert_atom(Atom::Element('a'));
        let tb = graph.insert_atom(Atom::Element('b'));
        let pattern = graph.insert_pattern(vec![ta, tb]);

        let resolved =
            resolve_token_ref(&graph, &TokenRef::Label("ab".into())).unwrap();
        assert_eq!(resolved.index, pattern.index);
    }

    #[test]
    fn resolve_by_multi_char_label_missing() {
        let graph = make_graph();
        graph.insert_atom(Atom::Element('a'));
        // 'b' doesn't exist, so "ab" can't be found
        let result = resolve_token_ref(&graph, &TokenRef::Label("ab".into()));
        assert!(result.is_err());
    }

    #[test]
    fn resolve_empty_label() {
        let graph = make_graph();
        let result = resolve_token_ref(&graph, &TokenRef::Label("".into()));
        assert!(result.is_err());
    }

    #[test]
    fn resolve_multiple_refs() {
        let graph = make_graph();
        let ta = graph.insert_atom(Atom::Element('a'));
        let tb = graph.insert_atom(Atom::Element('b'));

        let refs =
            vec![TokenRef::Index(ta.index.0), TokenRef::Label("b".into())];
        let resolved = resolve_token_refs(&graph, &refs).unwrap();
        assert_eq!(resolved.len(), 2);
        assert_eq!(resolved[0].index, ta.index);
        assert_eq!(resolved[1].index, tb.index);
    }

    #[test]
    fn resolve_multiple_refs_fails_on_first_error() {
        let graph = make_graph();
        graph.insert_atom(Atom::Element('a'));

        let refs = vec![
            TokenRef::Label("a".into()),
            TokenRef::Index(999), // this should fail
        ];
        let result = resolve_token_refs(&graph, &refs);
        assert!(result.is_err());
    }
}
