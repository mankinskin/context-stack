//! Search commands — search for patterns and sequences in the hypergraph.
//!
//! Wraps `context-search`'s `Find::find_ancestor` to search for token
//! sequences in the graph. Supports two entry points:
//!
//! - `search_pattern` — search by a list of `TokenRef` values (resolved to
//!   concrete tokens first).
//! - `search_sequence` — search by a text string (each character is looked up
//!   as an atom).
//!
//! Both return a [`SearchResult`] indicating complete, partial, or not-found.

use context_search::Find;
use context_trace::graph::{
    HypergraphRef,
    kind::BaseGraphKind,
};

use crate::{
    error::SearchError,
    resolve::resolve_token_refs,
    types::{
        PartialMatchInfo,
        PartialMatchKind,
        SearchResult,
        TokenInfo,
        TokenRef,
    },
    workspace::manager::WorkspaceManager,
};

impl WorkspaceManager {
    /// Search for a pattern specified by a list of token references.
    ///
    /// Each `TokenRef` is resolved to a concrete `Token` in the graph (by
    /// index or label). The resolved token sequence is then searched using
    /// `Find::find_ancestor`.
    ///
    /// # Arguments
    ///
    /// * `ws_name` — name of the open workspace.
    /// * `query` — ordered list of token references forming the search query.
    ///
    /// # Errors
    ///
    /// - `SearchError::WorkspaceNotOpen` if the workspace is not currently open.
    /// - `SearchError::QueryTooShort` if fewer than 2 token refs are given.
    /// - `SearchError::TokenNotFound` if a token ref cannot be resolved.
    /// - `SearchError::InternalError` on unexpected search failures.
    pub fn search_pattern(
        &self,
        ws_name: &str,
        query: Vec<TokenRef>,
    ) -> Result<SearchResult, SearchError> {
        if query.len() < 2 {
            return Err(SearchError::QueryTooShort);
        }

        let ws = self.get_workspace(ws_name).map_err(|_| {
            SearchError::WorkspaceNotOpen {
                workspace: ws_name.to_string(),
            }
        })?;
        let graph = ws.graph_ref();

        // Resolve TokenRefs to Tokens
        let tokens = resolve_token_refs(&graph, &query)?;

        // Execute search via Find::find_ancestor
        match graph.find_ancestor(tokens.as_slice()) {
            Ok(response) => Ok(build_search_result(&graph, &response)),
            Err(reason) => {
                // Check if this is a SingleIndex error — which means the query
                // resolved to a single existing vertex (that IS a match).
                // For now, map all search errors to InternalError.
                Err(SearchError::InternalError(format!("{reason:?}")))
            },
        }
    }

    /// Search for a text sequence in the graph.
    ///
    /// Each character in the text is treated as an atom. The search uses
    /// `Find::find_ancestor` with the character iterator directly (which
    /// `context-search` supports via its `Searchable` impl for `Chars`).
    ///
    /// # Arguments
    ///
    /// * `ws_name` — name of the open workspace.
    /// * `text` — the text string to search for (must be at least 2 characters).
    ///
    /// # Errors
    ///
    /// - `SearchError::WorkspaceNotOpen` if the workspace is not currently open.
    /// - `SearchError::QueryTooShort` if the text is shorter than 2 characters.
    /// - `SearchError::InternalError` on unexpected search failures.
    pub fn search_sequence(
        &self,
        ws_name: &str,
        text: &str,
    ) -> Result<SearchResult, SearchError> {
        if text.chars().count() < 2 {
            return Err(SearchError::QueryTooShort);
        }

        let ws = self.get_workspace(ws_name).map_err(|_| {
            SearchError::WorkspaceNotOpen {
                workspace: ws_name.to_string(),
            }
        })?;
        let graph = ws.graph_ref();

        match graph.find_ancestor(text.chars()) {
            Ok(response) => Ok(build_search_result(&graph, &response)),
            Err(reason) =>
                Err(SearchError::InternalError(format!("{reason:?}"))),
        }
    }
}

/// Convert a `context-search::Response` into our API `SearchResult`.
///
/// Examines the response to determine if the match is complete (entire root
/// with query exhausted), partial, or failed.
fn build_search_result(
    graph: &HypergraphRef<BaseGraphKind>,
    response: &context_search::Response,
) -> SearchResult {
    if response.is_entire_root() && response.query_exhausted() {
        // Full match — the query exactly matches an existing vertex
        let root = response.root_token();
        SearchResult {
            complete: true,
            token: TokenInfo::from_graph(graph, root),
            query_exhausted: true,
            partial: None,
        }
    } else {
        // Partial or incomplete match
        let root = response.root_token();
        let root_info = TokenInfo::from_graph(graph, root);

        let kind = if response.query_exhausted() {
            // Query was fully consumed but matched only part of a larger token
            PartialMatchKind::Range
        } else if response.is_entire_root() {
            // Matched an entire token but query has more remaining
            PartialMatchKind::Postfix
        } else {
            // Partial coverage of some token
            PartialMatchKind::Prefix
        };

        SearchResult {
            complete: false,
            token: None,
            query_exhausted: response.query_exhausted(),
            partial: Some(PartialMatchInfo {
                kind,
                root_token: root_info,
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::workspace::manager::WorkspaceManager;

    /// Helper: create a `WorkspaceManager` backed by a temporary directory
    /// with a workspace already created and open.
    fn setup(ws_name: &str) -> (tempfile::TempDir, WorkspaceManager) {
        let tmp = tempfile::tempdir().expect("failed to create temp dir");
        let mut mgr = WorkspaceManager::new(tmp.path().to_path_buf());
        mgr.create_workspace(ws_name).unwrap();
        (tmp, mgr)
    }

    /// Helper: add atoms for all characters in the string.
    fn add_atoms(
        mgr: &mut WorkspaceManager,
        ws: &str,
        chars: &str,
    ) {
        let char_vec: Vec<char> = chars.chars().collect();
        mgr.add_atoms(ws, char_vec).unwrap();
    }

    // -- search_sequence -----------------------------------------------------

    #[test]
    fn search_sequence_finds_existing_pattern() {
        let (_tmp, mut mgr) = setup("ws");
        add_atoms(&mut mgr, "ws", "ab");
        mgr.add_simple_pattern("ws", vec!['a', 'b']).unwrap();

        let result = mgr.search_sequence("ws", "ab").unwrap();
        assert!(result.complete, "should find existing pattern 'ab'");
        assert!(result.token.is_some());
        assert_eq!(result.token.as_ref().unwrap().label, "ab");
        assert!(result.query_exhausted);
        assert!(result.partial.is_none());
    }

    #[test]
    fn search_sequence_too_short() {
        let (_tmp, mgr) = setup("ws");
        let err = mgr.search_sequence("ws", "a").unwrap_err();
        match err {
            crate::error::SearchError::QueryTooShort => {},
            other => panic!("expected QueryTooShort, got: {other}"),
        }
    }

    #[test]
    fn search_sequence_workspace_not_open() {
        let (_tmp, mgr) = setup("ws");
        let err = mgr.search_sequence("nope", "ab").unwrap_err();
        match err {
            crate::error::SearchError::WorkspaceNotOpen { workspace } => {
                assert_eq!(workspace, "nope");
            },
            other => panic!("expected WorkspaceNotOpen, got: {other}"),
        }
    }

    // -- search_pattern (by TokenRef) ----------------------------------------

    #[test]
    fn search_pattern_too_short() {
        let (_tmp, mgr) = setup("ws");
        let err = mgr
            .search_pattern("ws", vec![crate::types::TokenRef::Index(0)])
            .unwrap_err();
        match err {
            crate::error::SearchError::QueryTooShort => {},
            other => panic!("expected QueryTooShort, got: {other}"),
        }
    }

    #[test]
    fn search_pattern_token_not_found() {
        let (_tmp, mgr) = setup("ws");
        let err = mgr
            .search_pattern(
                "ws",
                vec![
                    crate::types::TokenRef::Index(999),
                    crate::types::TokenRef::Index(998),
                ],
            )
            .unwrap_err();
        match err {
            crate::error::SearchError::TokenNotFound { .. } => {},
            other => panic!("expected TokenNotFound, got: {other}"),
        }
    }

    #[test]
    fn search_pattern_by_index() {
        let (_tmp, mut mgr) = setup("ws");
        add_atoms(&mut mgr, "ws", "ab");
        let pat = mgr.add_simple_pattern("ws", vec!['a', 'b']).unwrap();

        // Get atom indices
        let a = mgr.get_atom("ws", 'a').unwrap().unwrap();
        let b = mgr.get_atom("ws", 'b').unwrap().unwrap();

        let result = mgr
            .search_pattern(
                "ws",
                vec![
                    crate::types::TokenRef::Index(a.index),
                    crate::types::TokenRef::Index(b.index),
                ],
            )
            .unwrap();

        assert!(result.complete);
        assert_eq!(result.token.as_ref().unwrap().index, pat.index);
    }

    #[test]
    fn search_pattern_by_label() {
        let (_tmp, mut mgr) = setup("ws");
        add_atoms(&mut mgr, "ws", "abc");
        mgr.add_simple_pattern("ws", vec!['a', 'b']).unwrap();

        let result = mgr
            .search_pattern(
                "ws",
                vec![
                    crate::types::TokenRef::Label("a".into()),
                    crate::types::TokenRef::Label("b".into()),
                ],
            )
            .unwrap();

        assert!(result.complete);
        assert_eq!(result.token.as_ref().unwrap().label, "ab");
    }

    #[test]
    fn search_pattern_workspace_not_open() {
        let (_tmp, mgr) = setup("ws");
        let err = mgr
            .search_pattern(
                "nope",
                vec![
                    crate::types::TokenRef::Index(0),
                    crate::types::TokenRef::Index(1),
                ],
            )
            .unwrap_err();
        match err {
            crate::error::SearchError::WorkspaceNotOpen { workspace } => {
                assert_eq!(workspace, "nope");
            },
            other => panic!("expected WorkspaceNotOpen, got: {other}"),
        }
    }
}
