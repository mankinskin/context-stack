use std::collections::HashSet;

use context_trace::graph::snapshot::GraphSnapshot;

use crate::{
    error::{
        ApiError,
        AtomError,
        CompareError,
        InsertError,
        PatternError,
        ReadError,
        SearchError,
        WorkspaceError,
    },
    types::{
        AtomInfo,
        CompareMode,
        GraphDiffResult,
        GraphStatistics,
        InsertResult,
        PatternInfo,
        PatternReadResult,
        SearchResult,
        TokenInfo,
        TokenRef,
        ValidationReport,
        VertexInfo,
        WorkspaceInfo,
    },
};

use super::export_import::ExportFormat;

/// Strongly-typed API for all workspace operations.
///
/// `WorkspaceManager` implements this trait. Adapter layers (CLI, MCP, HTTP)
/// typically prefer the serializable `Command` / `CommandResult` path via
/// [`super::execute`], but Rust consumers can use this trait directly.
pub trait WorkspaceApi {
    // -- Workspace lifecycle ------------------------------------------------

    fn create_workspace(
        &mut self,
        name: &str,
    ) -> Result<WorkspaceInfo, WorkspaceError>;

    fn create_workspace_from_ngrams_text(
        &mut self,
        name: &str,
        text: &str,
        timeout_secs: u64,
    ) -> Result<WorkspaceInfo, WorkspaceError>;

    fn open_workspace(
        &mut self,
        name: &str,
    ) -> Result<WorkspaceInfo, WorkspaceError>;

    fn close_workspace(
        &mut self,
        name: &str,
    ) -> Result<(), WorkspaceError>;

    fn save_workspace(
        &mut self,
        name: &str,
    ) -> Result<(), WorkspaceError>;

    fn list_workspaces(&self) -> Result<Vec<WorkspaceInfo>, WorkspaceError>;

    fn delete_workspace(
        &mut self,
        name: &str,
    ) -> Result<(), WorkspaceError>;

    // -- Atoms --------------------------------------------------------------

    fn add_atom(
        &mut self,
        ws: &str,
        ch: char,
    ) -> Result<AtomInfo, AtomError>;

    fn add_atoms(
        &mut self,
        ws: &str,
        chars: Vec<char>,
    ) -> Result<Vec<AtomInfo>, AtomError>;

    fn get_atom(
        &self,
        ws: &str,
        ch: char,
    ) -> Result<Option<AtomInfo>, ApiError>;

    fn list_atoms(
        &self,
        ws: &str,
    ) -> Result<Vec<AtomInfo>, ApiError>;

    // -- Patterns -----------------------------------------------------------

    fn add_simple_pattern(
        &mut self,
        ws: &str,
        atoms: Vec<char>,
    ) -> Result<PatternInfo, PatternError>;

    fn get_vertex(
        &self,
        ws: &str,
        index: usize,
    ) -> Result<Option<VertexInfo>, ApiError>;

    fn list_vertices(
        &self,
        ws: &str,
    ) -> Result<Vec<TokenInfo>, ApiError>;

    // -- Search (Phase 2) ---------------------------------------------------

    fn search_pattern(
        &self,
        ws: &str,
        query: Vec<TokenRef>,
    ) -> Result<SearchResult, SearchError>;

    fn search_sequence(
        &self,
        ws: &str,
        text: &str,
    ) -> Result<SearchResult, SearchError>;

    // -- Insert (Phase 2) ---------------------------------------------------

    fn insert_first_match(
        &mut self,
        ws: &str,
        query: Vec<TokenRef>,
    ) -> Result<InsertResult, InsertError>;

    fn insert_sequence(
        &mut self,
        ws: &str,
        text: &str,
    ) -> Result<InsertResult, InsertError>;

    fn insert_sequences(
        &mut self,
        ws: &str,
        texts: HashSet<String>,
    ) -> Result<Vec<InsertResult>, InsertError>;

    // -- Read (Phase 2) -----------------------------------------------------

    fn read_pattern(
        &self,
        ws: &str,
        index: usize,
    ) -> Result<PatternReadResult, ReadError>;

    fn read_as_text(
        &self,
        ws: &str,
        index: usize,
    ) -> Result<String, ReadError>;

    fn read_sequence(
        &mut self,
        ws: &str,
        text: &str,
    ) -> Result<PatternReadResult, ReadError>;

    fn read_file(
        &mut self,
        ws: &str,
        path: &str,
    ) -> Result<PatternReadResult, ReadError>;

    // -- Debug / Introspection ----------------------------------------------

    fn get_snapshot(
        &self,
        ws: &str,
    ) -> Result<GraphSnapshot, ApiError>;

    fn get_statistics(
        &self,
        ws: &str,
    ) -> Result<GraphStatistics, ApiError>;

    fn validate_graph(
        &self,
        ws: &str,
    ) -> Result<ValidationReport, ApiError>;

    fn show_graph(
        &self,
        ws: &str,
    ) -> Result<String, ApiError>;

    fn show_vertex(
        &self,
        ws: &str,
        index: usize,
    ) -> Result<String, ApiError>;

    fn render_ascii_graph(
        &self,
        ws: &str,
        ascii_dag: bool,
    ) -> Result<String, ApiError>;

    // -- Compare (Phase 3.2) ------------------------------------------------

    fn compare_workspaces(
        &self,
        workspace_a: &str,
        workspace_b: &str,
        mode: CompareMode,
    ) -> Result<GraphDiffResult, CompareError>;

    fn compare_vertices(
        &self,
        workspace_a: &str,
        index_a: usize,
        workspace_b: &str,
        index_b: usize,
    ) -> Result<GraphDiffResult, CompareError>;

    // -- Export / Import (Phase 5) ------------------------------------------

    /// Export a workspace to JSON or bincode format.
    ///
    /// If `path` is `Some`, writes to the given file and returns `None`.
    /// If `path` is `None`, returns the raw export bytes as `Some(bytes)`.
    fn export_workspace(
        &self,
        ws: &str,
        format: ExportFormat,
        path: Option<&str>,
    ) -> Result<Option<Vec<u8>>, WorkspaceError>;

    /// Import a workspace from a previously exported file.
    ///
    /// If `overwrite` is true, an existing workspace with the same name
    /// will be replaced.
    fn import_workspace(
        &mut self,
        name: &str,
        path: &str,
        overwrite: bool,
    ) -> Result<WorkspaceInfo, WorkspaceError>;
}