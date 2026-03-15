//! Insert commands — thin wrappers that forward to the read/insert pipelines.
//!
//! Provides three entry points for inserting data into the graph:
//!
//! - `insert_first_match` — insert by a list of `TokenRef` values (resolved to
//!   concrete tokens first).
//! - `insert_sequence` — insert a text string via `ReadCtx::read_sequence`,
//!   which drives the full expansion-loop pipeline (auto-creates atoms as
//!   needed, detects overlaps, builds tightest decompositions).
//! - `insert_sequences` — bulk insert a set of text strings.
//!
//! ## Insertion strategy
//!
//! `insert_sequence` delegates to `context-read`'s `ReadCtx::read_sequence`
//! (the full expansion-loop pipeline).  Single-character inputs are accepted
//! and return the atom token directly.

use std::collections::HashSet;

use context_read::context::ReadCtx;
use context_trace::graph::vertex::{
    atom::Atom,
    pattern::Pattern,
    token::Token,
};
use tracing::debug;

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
    /// index or label). The resolved tokens are passed directly to
    /// `context-insert`'s `insert_next_match()` pipeline.
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

        debug!(
            token_count = tokens.len(),
            tokens = ?tokens,
            "insert_first_match: resolved tokens"
        );

        // Capture a snapshot of all known vertex indices before the read so
        // we can determine whether anything new was created.
        let pre_vertex_count = graph_ref.vertex_count();

        // Delegate to the full ReadCtx pipeline via a Pattern of resolved tokens.
        // This correctly handles the case where no composite exists yet (e.g.
        // atoms [a, b] → creates new ab token) as well as idempotent re-insertion
        // (returns the existing token when already present).
        let pattern = Pattern::from(tokens);
        let token = ReadCtx::new(graph_ref.clone(), pattern)
            .read_sequence()
            .ok_or_else(|| {
                InsertError::InternalError(
                    "read_sequence returned no token for non-empty input"
                        .to_string(),
                )
            })?;

        let post_vertex_count = graph_ref.vertex_count();
        let already_existed = post_vertex_count == pre_vertex_count;

        debug!(
            token = ?token,
            already_existed,
            "insert_first_match: read_sequence complete"
        );

        // Mark workspace dirty if new vertices were created.
        if !already_existed {
            let ws = self.get_workspace_mut(ws_name).map_err(|_| {
                InsertError::WorkspaceNotOpen {
                    workspace: ws_name.to_string(),
                }
            })?;
            ws.mark_dirty();
        }

        let token_info = TokenInfo::from_graph(&graph_ref, token)
            .unwrap_or_else(|| TokenInfo {
                index: token.index.0,
                label: String::new(),
                width: token.width.0,
            });

        Ok(InsertResult {
            token: token_info,
            already_existed,
        })
    }

    /// Insert a text sequence into the graph via the full read pipeline.
    ///
    /// Delegates to `ReadCtx::read_sequence` which drives the expansion-loop
    /// pipeline: atom auto-creation, segmentation, overlap detection, and
    /// tightest-decomposition building.
    ///
    /// Single-character inputs are accepted (they return the atom token
    /// directly).  Empty strings return `InsertError::QueryTooShort`.
    ///
    /// # Errors
    ///
    /// - `InsertError::WorkspaceNotOpen` if the workspace is not currently open.
    /// - `InsertError::QueryTooShort` if the text is empty.
    /// - `InsertError::InternalError` if `read_sequence` returns no token.
    pub fn insert_sequence(
        &mut self,
        ws_name: &str,
        text: &str,
    ) -> Result<InsertResult, InsertError> {
        if text.is_empty() {
            return Err(InsertError::QueryTooShort);
        }

        let ws = self.get_workspace(ws_name).map_err(|_| {
            InsertError::WorkspaceNotOpen {
                workspace: ws_name.to_string(),
            }
        })?;
        let graph_ref = ws.graph_ref();

        debug!(
            text,
            "insert_sequence: delegating to ReadCtx::read_sequence"
        );

        // Capture a snapshot of all known vertex indices before the read so
        // we can determine whether anything new was created.
        let pre_vertex_count = graph_ref.vertex_count();

        // Delegate to the full read pipeline via ReadCtx.
        // Collect chars to avoid lifetime issues with the &str borrow.
        let token = ReadCtx::new(graph_ref.clone(), text.chars())
            .read_sequence()
            .ok_or_else(|| {
                InsertError::InternalError(
                    "read_sequence returned no token for non-empty input"
                        .to_string(),
                )
            })?;

        let post_vertex_count = graph_ref.vertex_count();
        let already_existed = post_vertex_count == pre_vertex_count;

        debug!(
            token = ?token,
            already_existed,
            "insert_sequence: read_sequence complete"
        );

        // Mark workspace dirty if new vertices were created.
        if !already_existed {
            let ws = self.get_workspace_mut(ws_name).map_err(|_| {
                InsertError::WorkspaceNotOpen {
                    workspace: ws_name.to_string(),
                }
            })?;
            ws.mark_dirty();
        }

        let token_info = TokenInfo::from_graph(&graph_ref, token)
            .unwrap_or_else(|| TokenInfo {
                index: token.index.0,
                label: String::new(),
                width: token.width.0,
            });

        Ok(InsertResult {
            token: token_info,
            already_existed,
        })
    }

    /// Bulk insert multiple text sequences into the graph.
    ///
    /// Each sequence is inserted independently via `insert_sequence`. The
    /// order of processing is not guaranteed (the input is a `HashSet`).
    /// Results are returned in the order they were processed.
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
        let char_vec: Vec<char> = chars.chars().collect();
        mgr.add_atoms(ws, char_vec).unwrap();
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

        let result = mgr.insert_sequence("ws", "abcde").unwrap();
        assert!(!result.already_existed);

        // Verify atoms were created
        let atoms = mgr.list_atoms("ws").unwrap();
        let atom_chars: HashSet<char> = atoms.iter().map(|a| a.ch).collect();
        for ch in "abcde".chars() {
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
        // Single-character sequences are valid — they return the atom token
        // directly. Only empty strings are rejected with QueryTooShort.
        let result = mgr.insert_sequence("ws", "a").unwrap();
        assert_eq!(result.token.label, "a");
        assert_eq!(result.token.width, 1);

        // Empty string is still too short
        let err = mgr.insert_sequence("ws", "").unwrap_err();
        match err {
            crate::error::InsertError::QueryTooShort => {},
            other =>
                panic!("expected QueryTooShort for empty string, got: {other}"),
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

    // -- overlap / extension tests ------------------------------------------

    #[test]
    fn insert_overlapping_sequences_shares_subpatterns() {
        let (_tmp, mut mgr) = setup("ws");

        // Insert "ab" then "bc" — they overlap on 'b'
        let ab = mgr.insert_sequence("ws", "ab").unwrap();
        let _bc = mgr.insert_sequence("ws", "bc").unwrap();

        assert!(!ab.already_existed);

        // "ab" should still be searchable after inserting "bc"
        let r1 = mgr.search_sequence("ws", "ab").unwrap();
        assert!(r1.complete, "ab should be found");

        // Graph should be valid
        let report = mgr.validate_graph("ws").unwrap();
        assert!(report.valid, "graph should be valid: {:?}", report.issues);
    }

    #[test]
    fn insert_supersequence_of_existing() {
        let (_tmp, mut mgr) = setup("ws");

        // Insert "ab" first, then "abc" which extends it
        mgr.insert_sequence("ws", "ab").unwrap();
        let abc = mgr.insert_sequence("ws", "abc").unwrap();

        assert!(!abc.already_existed);
        assert_eq!(abc.token.width, 3);

        // "ab" should still be searchable
        let r1 = mgr.search_sequence("ws", "ab").unwrap();
        assert!(r1.complete, "ab should still be found");

        let report = mgr.validate_graph("ws").unwrap();
        assert!(report.valid, "graph should be valid: {:?}", report.issues);
    }

    #[test]
    fn insert_subsequence_of_existing() {
        let (_tmp, mut mgr) = setup("ws");

        // Insert "abcd" first, then "bc" which is a subsequence
        mgr.insert_sequence("ws", "abcd").unwrap();
        let _bc = mgr.insert_sequence("ws", "bc").unwrap();

        let report = mgr.validate_graph("ws").unwrap();
        assert!(report.valid, "graph should be valid: {:?}", report.issues);
    }

    #[test]
    fn insert_sequence_with_repeated_chars() {
        let (_tmp, mut mgr) = setup("ws");

        // "hello" has repeated 'l'
        let result = mgr.insert_sequence("ws", "hello").unwrap();
        assert!(!result.already_existed);
        assert_eq!(result.token.width, 5);
        assert_eq!(result.token.label, "hello");

        // Read back as text to verify the graph structure is correct
        let text_result = mgr.read_as_text("ws", result.token.index).unwrap();
        assert_eq!(text_result, "hello");

        let report = mgr.validate_graph("ws").unwrap();
        assert!(report.valid, "graph should be valid: {:?}", report.issues);
    }

    #[test]
    fn insert_sequence_correct_label() {
        let (_tmp, mut mgr) = setup("ws");

        let result = mgr.insert_sequence("ws", "xyz").unwrap();
        assert_eq!(result.token.label, "xyz");
        assert_eq!(result.token.width, 3);
    }

    #[test]
    fn insert_disjoint_sequences_are_independent() {
        let (_tmp, mut mgr) = setup("ws");

        // Insert two sequences with no shared characters
        let r1 = mgr.insert_sequence("ws", "ab").unwrap();
        let r2 = mgr.insert_sequence("ws", "cd").unwrap();

        assert_ne!(r1.token.index, r2.token.index);

        let s1 = mgr.search_sequence("ws", "ab").unwrap();
        assert!(s1.complete, "ab should be found");

        let s2 = mgr.search_sequence("ws", "cd").unwrap();
        assert!(s2.complete, "cd should be found");

        let report = mgr.validate_graph("ws").unwrap();
        assert!(report.valid, "graph should be valid: {:?}", report.issues);
    }
}
