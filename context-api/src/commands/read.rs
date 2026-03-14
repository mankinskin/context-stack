//! Read commands — read (decompose) vertices in the hypergraph.
//!
//! Provides two read operations:
//!
//! - `read_pattern` — returns a full recursive decomposition tree of a vertex,
//!   including the concatenated leaf text and a `ReadNode` tree.
//! - `read_as_text` — returns just the concatenated leaf text for a vertex.
//!
//! Both operations traverse the graph recursively using `context-trace`
//! primitives. For each vertex, the first child pattern is followed (vertices
//! may have multiple decompositions; we pick the first for determinism).

use context_trace::{
    VertexSet,
    graph::{
        Hypergraph,
        kind::BaseGraphKind,
        vertex::{
            VertexIndex,
            token::Token,
        },
    },
};

use crate::{
    error::ReadError,
    types::{
        PatternReadResult,
        ReadNode,
        TokenInfo,
    },
    workspace::manager::WorkspaceManager,
};

impl WorkspaceManager {
    /// Read a vertex as a full decomposition tree.
    ///
    /// Returns the root vertex info, the concatenated leaf text, and a
    /// recursive `ReadNode` tree showing how the vertex decomposes into
    /// children (and their children, recursively down to atoms).
    ///
    /// # Arguments
    ///
    /// * `ws_name` — name of the open workspace.
    /// * `index` — vertex index to read.
    ///
    /// # Errors
    ///
    /// - `ReadError::WorkspaceNotOpen` if the workspace is not currently open.
    /// - `ReadError::VertexNotFound` if no vertex exists at the given index.
    pub fn read_pattern(
        &self,
        ws_name: &str,
        index: usize,
    ) -> Result<PatternReadResult, ReadError> {
        let ws = self.get_workspace(ws_name).map_err(|_| {
            ReadError::WorkspaceNotOpen {
                workspace: ws_name.to_string(),
            }
        })?;
        let graph: &Hypergraph<BaseGraphKind> = ws.graph();

        let vi = VertexIndex(index);
        let data = graph
            .get_vertex_data(vi)
            .map_err(|_| ReadError::VertexNotFound { index })?;

        let root_token = data.to_token();
        let root_info = TokenInfo::from_graph(graph, root_token)
            .ok_or_else(|| ReadError::VertexNotFound { index })?;

        // Build recursive tree
        let tree = build_read_tree(graph, root_token);

        // Collect leaf text
        let text = collect_leaf_text(graph, root_token);

        Ok(PatternReadResult {
            root: root_info,
            text,
            tree,
        })
    }

    /// Read a vertex as concatenated leaf text.
    ///
    /// Recursively traverses the vertex's decomposition tree and concatenates
    /// all leaf atom characters into a single string.
    ///
    /// # Arguments
    ///
    /// * `ws_name` — name of the open workspace.
    /// * `index` — vertex index to read.
    ///
    /// # Errors
    ///
    /// - `ReadError::WorkspaceNotOpen` if the workspace is not currently open.
    /// - `ReadError::VertexNotFound` if no vertex exists at the given index.
    pub fn read_as_text(
        &self,
        ws_name: &str,
        index: usize,
    ) -> Result<String, ReadError> {
        let ws = self.get_workspace(ws_name).map_err(|_| {
            ReadError::WorkspaceNotOpen {
                workspace: ws_name.to_string(),
            }
        })?;
        let graph: &Hypergraph<BaseGraphKind> = ws.graph();

        let vi = VertexIndex(index);
        let data = graph
            .get_vertex_data(vi)
            .map_err(|_| ReadError::VertexNotFound { index })?;

        Ok(collect_leaf_text(graph, data.to_token()))
    }

    /// Read a text sequence through the graph.
    ///
    /// Each character in the text is ensured to exist as an atom (auto-created
    /// if missing). The atom sequence is then passed to `context-read`'s
    /// `ReadCtx::read_sequence` to find the largest-match decomposition.
    ///
    /// # Errors
    ///
    /// - `ReadError::WorkspaceNotOpen` if the workspace is not currently open.
    /// - `ReadError::SequenceTooShort` if the text is empty.
    /// - `ReadError::InternalError` on unexpected failures from the read algorithm.
    pub fn read_sequence(
        &mut self,
        ws_name: &str,
        text: &str,
    ) -> Result<PatternReadResult, ReadError> {
        let char_count = text.chars().count();
        if char_count == 0 {
            return Err(ReadError::SequenceTooShort { len: 0 });
        }

        // For single characters, just ensure the atom exists and read it
        if char_count == 1 {
            let ch = text.chars().next().unwrap();
            let ws = self.get_workspace(ws_name).map_err(|_| {
                ReadError::WorkspaceNotOpen {
                    workspace: ws_name.to_string(),
                }
            })?;
            let graph = ws.graph();

            // Ensure atom exists
            let atom = context_trace::graph::vertex::atom::Atom::Element(ch);
            let token = match graph.get_atom_index(atom) {
                Ok(idx) =>
                    context_trace::graph::vertex::token::Token::new(idx, 1),
                Err(_) => graph.insert_atom(atom),
            };

            // Read the single atom as a pattern
            return self.read_pattern(ws_name, token.index.0);
        }

        // Multi-character path: use context-read's ReadCtx
        {
            let ws = self.get_workspace(ws_name).map_err(|_| {
                ReadError::WorkspaceNotOpen {
                    workspace: ws_name.to_string(),
                }
            })?;
            let graph_ref = ws.graph_ref();

            let mut read_ctx =
                context_read::context::ReadCtx::new(graph_ref, text.chars());

            let root_token = read_ctx.read_sequence();

            match root_token {
                Some(token) => {
                    // Mark workspace dirty (atoms may have been created)
                    drop(read_ctx);
                    let ws = self.get_workspace_mut(ws_name).map_err(|_| {
                        ReadError::WorkspaceNotOpen {
                            workspace: ws_name.to_string(),
                        }
                    })?;
                    ws.mark_dirty();

                    // Build PatternReadResult from the root token
                    self.read_pattern(ws_name, token.index.0)
                },
                None => Err(ReadError::InternalError(format!(
                    "read_sequence returned None for text of length \
                     {char_count}"
                ))),
            }
        }
    }

    /// Read a file's contents through the graph.
    ///
    /// Reads the file at `path` to a string, then delegates to
    /// `read_sequence`.
    ///
    /// # Errors
    ///
    /// - `ReadError::FileReadError` if the file cannot be read.
    /// - All errors from `read_sequence`.
    pub fn read_file(
        &mut self,
        ws_name: &str,
        path: &str,
    ) -> Result<PatternReadResult, ReadError> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            ReadError::FileReadError {
                path: path.to_string(),
                reason: e.to_string(),
            }
        })?;

        self.read_sequence(ws_name, &content)
    }
}

/// Recursively build a `ReadNode` tree by expanding child patterns.
///
/// For each vertex:
/// - If it's an atom (no child patterns) → leaf node (no children).
/// - If it has child patterns → pick the first child pattern and recurse
///   into each child token.
///
/// Note: A vertex may have multiple child patterns (different decompositions).
/// For the read tree, we use the first pattern (sorted by `PatternId` for
/// determinism). A future enhancement could expose all decompositions.
fn build_read_tree(
    graph: &Hypergraph<BaseGraphKind>,
    token: Token,
) -> ReadNode {
    let token_info =
        TokenInfo::from_graph(graph, token).unwrap_or_else(|| TokenInfo {
            index: token.index.0,
            label: format!("?{}", token.index.0),
            width: token.width.0,
        });

    let data = match graph.get_vertex_data(token.index) {
        Ok(d) => d,
        Err(_) => {
            return ReadNode {
                token: token_info,
                children: vec![],
            };
        },
    };

    // Check if this is an atom (has no child patterns)
    if data.child_patterns().is_empty() {
        return ReadNode {
            token: token_info,
            children: vec![],
        };
    }

    // Get the first child pattern (sorted by PatternId for determinism)
    let mut sorted_patterns: Vec<_> = data.child_patterns().iter().collect();
    sorted_patterns.sort_by_key(|(pid, _)| *pid);

    let children = if let Some((_pid, pattern)) = sorted_patterns.first() {
        pattern
            .iter()
            .map(|&child_token| build_read_tree(graph, child_token))
            .collect()
    } else {
        vec![]
    };

    ReadNode {
        token: token_info,
        children,
    }
}

/// Recursively collect leaf text by traversing to atoms and concatenating
/// their string representations.
///
/// Uses `graph.vertex_data_string()` for atoms to get the character value,
/// and recurses through the first child pattern for non-atom vertices.
fn collect_leaf_text(
    graph: &Hypergraph<BaseGraphKind>,
    token: Token,
) -> String {
    let data = match graph.get_vertex_data(token.index) {
        Ok(d) => d,
        Err(_) => return String::new(),
    };

    // If this is an atom (no child patterns), return its string representation
    if data.child_patterns().is_empty() {
        return graph.vertex_data_string(data);
    }

    // Recurse through first child pattern (sorted for determinism)
    let mut sorted_patterns: Vec<_> = data.child_patterns().iter().collect();
    sorted_patterns.sort_by_key(|(pid, _)| *pid);

    match sorted_patterns.first() {
        Some((_pid, pattern)) => pattern
            .iter()
            .map(|&child_token| collect_leaf_text(graph, child_token))
            .collect(),
        None => String::new(),
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

    // -- read_pattern --------------------------------------------------------

    #[test]
    fn read_atom() {
        let (_tmp, mut mgr) = setup("ws");
        let atom = mgr.add_atom("ws", 'a').unwrap();

        let result = mgr.read_pattern("ws", atom.index).unwrap();
        assert_eq!(result.root.label, "a");
        assert_eq!(result.text, "a");
        assert!(
            result.tree.children.is_empty(),
            "atom should have no children"
        );
    }

    #[test]
    fn read_simple_pattern() {
        let (_tmp, mut mgr) = setup("ws");
        add_atoms(&mut mgr, "ws", "ab");
        let pat = mgr.add_simple_pattern("ws", vec!['a', 'b']).unwrap();

        let result = mgr.read_pattern("ws", pat.index).unwrap();
        assert_eq!(result.root.label, "ab");
        assert_eq!(result.text, "ab");
        assert_eq!(result.tree.children.len(), 2);
        assert_eq!(result.tree.children[0].token.label, "a");
        assert_eq!(result.tree.children[1].token.label, "b");
        // Children are atoms — no further nesting
        assert!(result.tree.children[0].children.is_empty());
        assert!(result.tree.children[1].children.is_empty());
    }

    #[test]
    fn read_as_text_atom() {
        let (_tmp, mut mgr) = setup("ws");
        let atom = mgr.add_atom("ws", 'x').unwrap();

        let text = mgr.read_as_text("ws", atom.index).unwrap();
        assert_eq!(text, "x");
    }

    #[test]
    fn read_as_text_pattern() {
        let (_tmp, mut mgr) = setup("ws");
        add_atoms(&mut mgr, "ws", "abc");
        let pat = mgr.add_simple_pattern("ws", vec!['a', 'b', 'c']).unwrap();

        let text = mgr.read_as_text("ws", pat.index).unwrap();
        assert_eq!(text, "abc");
    }

    #[test]
    fn read_nonexistent_vertex() {
        let (_tmp, mgr) = setup("ws");
        let err = mgr.read_pattern("ws", 99999).unwrap_err();
        match err {
            crate::error::ReadError::VertexNotFound { index } => {
                assert_eq!(index, 99999);
            },
            other => panic!("expected VertexNotFound, got: {other}"),
        }
    }

    #[test]
    fn read_as_text_nonexistent() {
        let (_tmp, mgr) = setup("ws");
        let err = mgr.read_as_text("ws", 99999).unwrap_err();
        match err {
            crate::error::ReadError::VertexNotFound { index } => {
                assert_eq!(index, 99999);
            },
            other => panic!("expected VertexNotFound, got: {other}"),
        }
    }

    #[test]
    fn read_workspace_not_open() {
        let (_tmp, mgr) = setup("ws");
        let err = mgr.read_pattern("nope", 0).unwrap_err();
        match err {
            crate::error::ReadError::WorkspaceNotOpen { workspace } => {
                assert_eq!(workspace, "nope");
            },
            other => panic!("expected WorkspaceNotOpen, got: {other}"),
        }
    }

    #[test]
    fn read_as_text_workspace_not_open() {
        let (_tmp, mgr) = setup("ws");
        let err = mgr.read_as_text("nope", 0).unwrap_err();
        match err {
            crate::error::ReadError::WorkspaceNotOpen { workspace } => {
                assert_eq!(workspace, "nope");
            },
            other => panic!("expected WorkspaceNotOpen, got: {other}"),
        }
    }

    #[test]
    fn read_inserted_sequence() {
        let (_tmp, mut mgr) = setup("ws");

        // Insert a sequence and then read it back
        let insert_result = mgr.insert_sequence("ws", "abcde").unwrap();

        let read_result =
            mgr.read_pattern("ws", insert_result.token.index).unwrap();
        assert_eq!(read_result.text, "abcde");
        assert_eq!(read_result.root.width, 5);

        let text = mgr.read_as_text("ws", insert_result.token.index).unwrap();
        assert_eq!(text, "abcde");
    }

    #[test]
    fn read_sequence_basic() {
        let (_tmp, mut mgr) = setup("ws");
        let result = mgr.read_sequence("ws", "ab").unwrap();
        assert_eq!(result.text, "ab");
        assert_eq!(result.root.width, 2);
    }

    #[test]
    fn read_sequence_single_char() {
        let (_tmp, mut mgr) = setup("ws");
        let result = mgr.read_sequence("ws", "x").unwrap();
        assert_eq!(result.text, "x");
        assert_eq!(result.root.width, 1);
        assert!(
            result.tree.children.is_empty(),
            "atom should have no children"
        );
    }

    #[test]
    fn read_sequence_empty_returns_error() {
        let (_tmp, mut mgr) = setup("ws");
        let err = mgr.read_sequence("ws", "").unwrap_err();
        match err {
            crate::error::ReadError::SequenceTooShort { len } => {
                assert_eq!(len, 0);
            },
            other => panic!("expected SequenceTooShort, got: {other}"),
        }
    }

    #[test]
    fn read_sequence_workspace_not_open() {
        let (_tmp, mut mgr) = setup("ws");
        let err = mgr.read_sequence("nonexistent", "hello").unwrap_err();
        match err {
            crate::error::ReadError::WorkspaceNotOpen { workspace } => {
                assert_eq!(workspace, "nonexistent");
            },
            other => panic!("expected WorkspaceNotOpen, got: {other}"),
        }
    }

    #[test]
    fn read_file_not_found() {
        let (_tmp, mut mgr) = setup("ws");
        let err = mgr.read_file("ws", "/nonexistent/path.txt").unwrap_err();
        match err {
            crate::error::ReadError::FileReadError { path, .. } => {
                assert_eq!(path, "/nonexistent/path.txt");
            },
            other => panic!("expected FileReadError, got: {other}"),
        }
    }
}
