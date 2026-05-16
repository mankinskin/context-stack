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
    workspace::manager::WorkspaceManager,
};

use super::{
    api::WorkspaceApi,
    export_import::{
        self,
        ExportFormat,
    },
};

impl WorkspaceApi for WorkspaceManager {
    fn create_workspace(
        &mut self,
        name: &str,
    ) -> Result<WorkspaceInfo, WorkspaceError> {
        WorkspaceManager::create_workspace(self, name)
    }

    fn create_workspace_from_ngrams_text(
        &mut self,
        name: &str,
        text: &str,
        timeout_secs: u64,
    ) -> Result<WorkspaceInfo, WorkspaceError> {
        WorkspaceManager::create_workspace_from_ngrams_text(
            self,
            name,
            text,
            timeout_secs,
        )
    }

    fn open_workspace(
        &mut self,
        name: &str,
    ) -> Result<WorkspaceInfo, WorkspaceError> {
        WorkspaceManager::open_workspace(self, name)
    }

    fn close_workspace(
        &mut self,
        name: &str,
    ) -> Result<(), WorkspaceError> {
        WorkspaceManager::close_workspace(self, name)
    }

    fn save_workspace(
        &mut self,
        name: &str,
    ) -> Result<(), WorkspaceError> {
        WorkspaceManager::save_workspace(self, name)
    }

    fn list_workspaces(&self) -> Result<Vec<WorkspaceInfo>, WorkspaceError> {
        WorkspaceManager::list_workspaces(self)
    }

    fn delete_workspace(
        &mut self,
        name: &str,
    ) -> Result<(), WorkspaceError> {
        WorkspaceManager::delete_workspace(self, name)
    }

    fn add_atom(
        &mut self,
        ws: &str,
        ch: char,
    ) -> Result<AtomInfo, AtomError> {
        WorkspaceManager::add_atom(self, ws, ch)
    }

    fn add_atoms(
        &mut self,
        ws: &str,
        chars: Vec<char>,
    ) -> Result<Vec<AtomInfo>, AtomError> {
        WorkspaceManager::add_atoms(self, ws, chars)
    }

    fn get_atom(
        &self,
        ws: &str,
        ch: char,
    ) -> Result<Option<AtomInfo>, ApiError> {
        WorkspaceManager::get_atom(self, ws, ch)
    }

    fn list_atoms(
        &self,
        ws: &str,
    ) -> Result<Vec<AtomInfo>, ApiError> {
        WorkspaceManager::list_atoms(self, ws)
    }

    fn add_simple_pattern(
        &mut self,
        ws: &str,
        atoms: Vec<char>,
    ) -> Result<PatternInfo, PatternError> {
        WorkspaceManager::add_simple_pattern(self, ws, atoms)
    }

    fn get_vertex(
        &self,
        ws: &str,
        index: usize,
    ) -> Result<Option<VertexInfo>, ApiError> {
        WorkspaceManager::get_vertex(self, ws, index)
    }

    fn list_vertices(
        &self,
        ws: &str,
    ) -> Result<Vec<TokenInfo>, ApiError> {
        WorkspaceManager::list_vertices(self, ws)
    }

    fn search_pattern(
        &self,
        ws: &str,
        query: Vec<TokenRef>,
    ) -> Result<SearchResult, SearchError> {
        WorkspaceManager::search_pattern(self, ws, query)
    }

    fn search_sequence(
        &self,
        ws: &str,
        text: &str,
    ) -> Result<SearchResult, SearchError> {
        WorkspaceManager::search_sequence(self, ws, text)
    }

    fn insert_first_match(
        &mut self,
        ws: &str,
        query: Vec<TokenRef>,
    ) -> Result<InsertResult, InsertError> {
        WorkspaceManager::insert_first_match(self, ws, query)
    }

    fn insert_sequence(
        &mut self,
        ws: &str,
        text: &str,
    ) -> Result<InsertResult, InsertError> {
        WorkspaceManager::insert_sequence(self, ws, text)
    }

    fn insert_sequences(
        &mut self,
        ws: &str,
        texts: HashSet<String>,
    ) -> Result<Vec<InsertResult>, InsertError> {
        WorkspaceManager::insert_sequences(self, ws, texts)
    }

    fn read_pattern(
        &self,
        ws: &str,
        index: usize,
    ) -> Result<PatternReadResult, ReadError> {
        WorkspaceManager::read_pattern(self, ws, index)
    }

    fn read_as_text(
        &self,
        ws: &str,
        index: usize,
    ) -> Result<String, ReadError> {
        WorkspaceManager::read_as_text(self, ws, index)
    }

    fn read_sequence(
        &mut self,
        ws: &str,
        text: &str,
    ) -> Result<PatternReadResult, ReadError> {
        WorkspaceManager::read_sequence(self, ws, text)
    }

    fn read_file(
        &mut self,
        ws: &str,
        path: &str,
    ) -> Result<PatternReadResult, ReadError> {
        WorkspaceManager::read_file(self, ws, path)
    }

    fn get_snapshot(
        &self,
        ws: &str,
    ) -> Result<GraphSnapshot, ApiError> {
        WorkspaceManager::get_snapshot(self, ws)
    }

    fn get_statistics(
        &self,
        ws: &str,
    ) -> Result<GraphStatistics, ApiError> {
        WorkspaceManager::get_statistics(self, ws)
    }

    fn validate_graph(
        &self,
        ws: &str,
    ) -> Result<ValidationReport, ApiError> {
        WorkspaceManager::validate_graph(self, ws)
    }

    fn show_graph(
        &self,
        ws: &str,
    ) -> Result<String, ApiError> {
        WorkspaceManager::show_graph(self, ws)
    }

    fn show_vertex(
        &self,
        ws: &str,
        index: usize,
    ) -> Result<String, ApiError> {
        WorkspaceManager::show_vertex(self, ws, index)
    }

    fn render_ascii_graph(
        &self,
        ws: &str,
        ascii_dag: bool,
    ) -> Result<String, ApiError> {
        WorkspaceManager::render_ascii_graph(self, ws, ascii_dag)
    }

    fn compare_workspaces(
        &self,
        workspace_a: &str,
        workspace_b: &str,
        mode: CompareMode,
    ) -> Result<GraphDiffResult, CompareError> {
        WorkspaceManager::compare_workspaces(
            self,
            workspace_a,
            workspace_b,
            mode,
        )
    }

    fn compare_vertices(
        &self,
        workspace_a: &str,
        index_a: usize,
        workspace_b: &str,
        index_b: usize,
    ) -> Result<GraphDiffResult, CompareError> {
        WorkspaceManager::compare_vertices(
            self,
            workspace_a,
            index_a,
            workspace_b,
            index_b,
        )
    }

    fn export_workspace(
        &self,
        ws: &str,
        format: ExportFormat,
        path: Option<&str>,
    ) -> Result<Option<Vec<u8>>, WorkspaceError> {
        export_import::export_workspace(self, ws, format, path)
    }

    fn import_workspace(
        &mut self,
        name: &str,
        path: &str,
        overwrite: bool,
    ) -> Result<WorkspaceInfo, WorkspaceError> {
        export_import::import_workspace(self, name, path, overwrite)
    }
}