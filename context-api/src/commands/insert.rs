//! Insert commands — insert patterns and sequences into the hypergraph.
//!
//! Provides three entry points for inserting data into the graph:
//!
//! - `insert_first_match` — insert by a list of `TokenRef` values (resolved to
//!   concrete tokens first).
//! - `insert_sequence` — insert a text string (auto-creates atoms as needed).
//! - `insert_sequences` — bulk insert a set of text strings.
//!
//! Insertions use the direct `graph.insert_pattern()` API which correctly
//! handles building new vertices from token sequences. The search crate is
//! used to check for existing sequences before inserting.
//!
//! All return [`InsertResult`] indicating the resulting token and whether it
//! already existed.

use std::collections::HashSet;

use context_search::Find;
use context_trace::graph::vertex::{
    atom::Atom,
    token::Token,
};

use crate::{
    error::InsertError,
    resolve::resolve_token_refs,
    types::{
        InsertResult,
        TokenInfo,
        TokenRef,
    },
    workspace::manager::WorkspaceManager,
};

impl WorkspaceManager {
    /// Insert a pattern specified by a list of token references.
    ///
    /// Each `TokenRef` is resolved to a concrete `Token` in the graph (by
    /// index or label). If the resolved token sequence already exists as a
    /// single vertex, the existing vertex is returned with
    /// `already_existed: true`. Otherwise, the sequence is inserted into
    /// the graph, splitting and joining as needed.
    ///
    /// # Arguments
    ///
    /// * `ws_name` — name of the open workspace.
    /// * `query` — ordered list of token references forming the pattern to insert.
    ///
    /// # Errors
    ///
    /// - `InsertError::WorkspaceNotOpen` if the workspace is not currently open.
    /// - `InsertError::QueryTooShort` if fewer than 2 token refs are given.
    /// - `InsertError::TokenNotFound` if a token ref cannot be resolved.
    /// - `InsertError::InternalError` on unexpected insert failures.
    pub fn insert_first_match(
        &mut self,
        ws_name: &str,
        query: Vec<TokenRef>,
    ) -> Result<InsertResult, InsertError> {
        if query.len() < 2 {
            return Err(InsertError::QueryTooShort);
        }

        let ws = self.get_workspace(ws_name).map_err(|_| {
            InsertError::WorkspaceNotOpen {
                workspace: ws_name.to_string(),
            }
        })?;
        let graph_ref = ws.graph_ref();

        // Resolve TokenRefs to Tokens
        let tokens = resolve_token_refs(&graph_ref, &query)?;

        // First, check if this exact sequence already exists
        match graph_ref.find_ancestor(tokens.as_slice()) {
            Ok(response)
                if response.is_entire_root() && response.query_exhausted() =>
            {
                let root = response.root_token();
                let token_info = TokenInfo::from_graph(&graph_ref, root)
                    .unwrap_or_else(|| TokenInfo {
                        index: root.index.0,
                        label: String::new(),
                        width: root.width.0,
                    });
                return Ok(InsertResult {
                    token: token_info,
                    already_existed: true,
                });
            },
            _ => {
                // Not found or partial — proceed to insert
            },
        }

        // Insert the pattern directly using the graph API.
        // The resolved tokens are already concrete vertices in the graph,
        // so we can call insert_pattern directly.
        let result_token = graph_ref.insert_pattern(tokens);

        // Mark workspace dirty
        let ws = self.get_workspace_mut(ws_name).map_err(|_| {
            InsertError::WorkspaceNotOpen {
                workspace: ws_name.to_string(),
            }
        })?;
        ws.mark_dirty();

        let token_info = TokenInfo::from_graph(&graph_ref, result_token)
            .unwrap_or_else(|| TokenInfo {
                index: result_token.index.0,
                label: String::new(),
                width: result_token.width.0,
            });

        Ok(InsertResult {
            token: token_info,
            already_existed: false,
        })
    }

    /// Insert a text sequence into the graph.
    ///
    /// Each character in the text is ensured to exist as an atom (auto-created
    /// if missing). If the full sequence already exists as a single vertex,
    /// the existing vertex is returned with `already_existed: true`. Otherwise,
    /// atoms are created for each character and the full token sequence is
    /// inserted via `graph.insert_pattern()`.
    ///
    /// # Arguments
    ///
    /// * `ws_name` — name of the open workspace.
    /// * `text` — the text string to insert (must be at least 2 characters).
    ///
    /// # Errors
    ///
    /// - `InsertError::WorkspaceNotOpen` if the workspace is not currently open.
    /// - `InsertError::QueryTooShort` if the text is shorter than 2 characters.
    /// - `InsertError::InternalError` on unexpected insert failures.
    pub fn insert_sequence(
        &mut self,
        ws_name: &str,
        text: &str,
    ) -> Result<InsertResult, InsertError> {
        if text.chars().count() < 2 {
            return Err(InsertError::QueryTooShort);
        }

        let ws = self.get_workspace(ws_name).map_err(|_| {
            InsertError::WorkspaceNotOpen {
                workspace: ws_name.to_string(),
            }
        })?;
        let graph_ref = ws.graph_ref();

        // First check if the sequence already exists in the graph.
        // We need all atoms to exist for the search to work, so we check
        // whether all chars are known atoms first.
        let all_atoms_exist = text
            .chars()
            .all(|ch| graph_ref.get_atom_index(Atom::Element(ch)).is_ok());

        if all_atoms_exist {
            match graph_ref.find_ancestor(text.chars()) {
                Ok(response)
                    if response.is_entire_root()
                        && response.query_exhausted() =>
                {
                    let root = response.root_token();
                    let token_info = TokenInfo::from_graph(&graph_ref, root)
                        .unwrap_or_else(|| TokenInfo {
                            index: root.index.0,
                            label: String::new(),
                            width: root.width.0,
                        });

                    // Mark dirty in case we want to track reads
                    return Ok(InsertResult {
                        token: token_info,
                        already_existed: true,
                    });
                },
                _ => {
                    // Not found or partial — proceed to insert
                },
            }
        }

        // Ensure all atoms exist (auto-create missing ones) and build
        // the token sequence preserving the original character order
        // (including duplicate characters).
        let tokens: Vec<_> = text
            .chars()
            .map(|ch| {
                let atom = Atom::Element(ch);
                match graph_ref.get_atom_index(atom) {
                    Ok(idx) => Token::new(idx, 1),
                    Err(_) => graph_ref.insert_atom(atom),
                }
            })
            .collect();

        // Insert the full pattern directly.
        let result_token = graph_ref.insert_pattern(tokens);

        // Mark workspace dirty
        let ws = self.get_workspace_mut(ws_name).map_err(|_| {
            InsertError::WorkspaceNotOpen {
                workspace: ws_name.to_string(),
            }
        })?;
        ws.mark_dirty();

        let token_info = TokenInfo::from_graph(&graph_ref, result_token)
            .unwrap_or_else(|| TokenInfo {
                index: result_token.index.0,
                label: String::new(),
                width: result_token.width.0,
            });

        Ok(InsertResult {
            token: token_info,
            already_existed: false,
        })
    }

    /// Bulk insert multiple text sequences into the graph.
    ///
    /// Each sequence is inserted independently. The order of processing is
    /// not guaranteed (the input is a `HashSet`). Results are returned in
    /// the order they were processed.
    ///
    /// # Arguments
    ///
    /// * `ws_name` — name of the open workspace.
    /// * `texts` — set of text strings to insert (each must be at least 2 chars).
    ///
    /// # Errors
    ///
    /// Stops at the first error and returns it. Successfully inserted
    /// sequences before the error are committed to the graph.
    pub fn insert_sequences(
        &mut self,
        ws_name: &str,
        texts: HashSet<String>,
    ) -> Result<Vec<InsertResult>, InsertError> {
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            results.push(self.insert_sequence(ws_name, &text)?);
        }
        Ok(results)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::workspace::manager::WorkspaceManager;
    use std::collections::HashSet;

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
        let char_set: HashSet<char> = chars.chars().collect();
        mgr.add_atoms(ws, char_set).unwrap();
    }

    // -- insert_sequence -----------------------------------------------------

    #[test]
    fn insert_sequence_creates_new() {
        let (_tmp, mut mgr) = setup("ws");

        let result = mgr.insert_sequence("ws", "abc").unwrap();
        assert!(!result.already_existed);
        assert_eq!(result.token.width, 3);
    }

    #[test]
    fn insert_sequence_auto_creates_atoms() {
        let (_tmp, mut mgr) = setup("ws");
        // No atoms added yet — insert_sequence should auto-create them

        let result = mgr.insert_sequence("ws", "hello").unwrap();
        assert!(!result.already_existed);

        // Verify atoms were created
        let atoms = mgr.list_atoms("ws").unwrap();
        let atom_chars: HashSet<char> = atoms.iter().map(|a| a.ch).collect();
        for ch in "helo".chars() {
            assert!(
                atom_chars.contains(&ch),
                "atom '{ch}' should have been auto-created"
            );
        }
    }

    #[test]
    fn insert_sequence_idempotent() {
        let (_tmp, mut mgr) = setup("ws");

        let first = mgr.insert_sequence("ws", "ab").unwrap();
        let second = mgr.insert_sequence("ws", "ab").unwrap();

        assert_eq!(first.token.index, second.token.index);
        assert!(second.already_existed);
    }

    #[test]
    fn insert_sequence_too_short() {
        let (_tmp, mut mgr) = setup("ws");
        let err = mgr.insert_sequence("ws", "a").unwrap_err();
        match err {
            crate::error::InsertError::QueryTooShort => {},
            other => panic!("expected QueryTooShort, got: {other}"),
        }
    }

    #[test]
    fn insert_sequence_workspace_not_open() {
        let (_tmp, mut mgr) = setup("ws");
        let err = mgr.insert_sequence("nope", "ab").unwrap_err();
        match err {
            crate::error::InsertError::WorkspaceNotOpen { workspace } => {
                assert_eq!(workspace, "nope");
            },
            other => panic!("expected WorkspaceNotOpen, got: {other}"),
        }
    }

    #[test]
    fn insert_then_search_finds_it() {
        let (_tmp, mut mgr) = setup("ws");

        let insert_result = mgr.insert_sequence("ws", "abc").unwrap();
        assert!(!insert_result.already_existed);

        // Now search for the same sequence — should be found
        let search_result = mgr.search_sequence("ws", "abc").unwrap();
        assert!(search_result.complete);
        assert_eq!(
            search_result.token.as_ref().unwrap().index,
            insert_result.token.index
        );
    }

    #[test]
    fn insert_sequence_marks_workspace_dirty() {
        let (_tmp, mut mgr) = setup("ws");

        // Save to clear dirty flag
        mgr.save_workspace("ws").unwrap();
        assert!(!mgr.get_workspace("ws").unwrap().is_dirty());

        mgr.insert_sequence("ws", "ab").unwrap();
        assert!(mgr.get_workspace("ws").unwrap().is_dirty());
    }

    #[test]
    fn insert_multiple_sequences_builds_graph() {
        let (_tmp, mut mgr) = setup("ws");

        mgr.insert_sequence("ws", "abc").unwrap();
        mgr.insert_sequence("ws", "def").unwrap();

        // Both should be searchable
        let r1 = mgr.search_sequence("ws", "abc").unwrap();
        assert!(r1.complete, "abc should be found");

        let r2 = mgr.search_sequence("ws", "def").unwrap();
        assert!(r2.complete, "def should be found");
    }

    // -- insert_first_match --------------------------------------------------

    #[test]
    fn insert_first_match_by_index() {
        let (_tmp, mut mgr) = setup("ws");
        add_atoms(&mut mgr, "ws", "ab");

        let a = mgr.get_atom("ws", 'a').unwrap().unwrap();
        let b = mgr.get_atom("ws", 'b').unwrap().unwrap();

        let result = mgr
            .insert_first_match(
                "ws",
                vec![
                    crate::types::TokenRef::Index(a.index),
                    crate::types::TokenRef::Index(b.index),
                ],
            )
            .unwrap();

        assert!(!result.already_existed);
        assert_eq!(result.token.label, "ab");
        assert_eq!(result.token.width, 2);
    }

    #[test]
    fn insert_first_match_existing() {
        let (_tmp, mut mgr) = setup("ws");
        add_atoms(&mut mgr, "ws", "ab");
        mgr.add_simple_pattern("ws", vec!['a', 'b']).unwrap();

        let a = mgr.get_atom("ws", 'a').unwrap().unwrap();
        let b = mgr.get_atom("ws", 'b').unwrap().unwrap();

        let result = mgr
            .insert_first_match(
                "ws",
                vec![
                    crate::types::TokenRef::Index(a.index),
                    crate::types::TokenRef::Index(b.index),
                ],
            )
            .unwrap();

        assert!(result.already_existed);
    }

    #[test]
    fn insert_first_match_too_short() {
        let (_tmp, mut mgr) = setup("ws");
        let err = mgr
            .insert_first_match("ws", vec![crate::types::TokenRef::Index(0)])
            .unwrap_err();
        match err {
            crate::error::InsertError::QueryTooShort => {},
            other => panic!("expected QueryTooShort, got: {other}"),
        }
    }

    // -- insert_sequences (bulk) ---------------------------------------------

    #[test]
    fn insert_sequences_bulk() {
        let (_tmp, mut mgr) = setup("ws");

        let texts: HashSet<String> =
            ["abc", "def"].iter().map(|s| s.to_string()).collect();
        let results = mgr.insert_sequences("ws", texts).unwrap();
        assert_eq!(results.len(), 2);

        // Both should be new
        for result in &results {
            assert!(!result.already_existed);
        }
    }

    #[test]
    fn insert_sequences_empty_set() {
        let (_tmp, mut mgr) = setup("ws");
        let texts: HashSet<String> = HashSet::new();
        let results = mgr.insert_sequences("ws", texts).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn insert_preserves_graph_integrity() {
        let (_tmp, mut mgr) = setup("ws");

        // Insert several overlapping sequences
        mgr.insert_sequence("ws", "abc").unwrap();
        mgr.insert_sequence("ws", "bcd").unwrap();
        mgr.insert_sequence("ws", "abcd").unwrap();

        // Validate graph integrity
        let report = mgr.validate_graph("ws").unwrap();
        assert!(report.valid, "graph should be valid: {:?}", report.issues);
    }
}
