//! Command module — the `Command` enum, `CommandResult` enum, dispatch
//! function, and `WorkspaceApi` trait.
//!
//! This module provides two complementary interfaces:
//!
//! 1. **`WorkspaceApi` trait** — a strongly-typed Rust trait that
//!    `WorkspaceManager` implements. Ideal for Rust consumers that want
//!    compile-time checked method calls.
//!
//! 2. **`Command` / `CommandResult` enums** — a serializable
//!    request/response pair suitable for adapter layers (CLI, MCP, HTTP).
//!    The `execute` function dispatches a `Command` to the appropriate
//!    `WorkspaceApi` method and wraps the result in a `CommandResult`.

pub mod atoms;
pub mod debug;
pub mod insert;
pub mod patterns;
pub mod read;
pub mod search;

use std::collections::HashSet;

use serde::{
    Deserialize,
    Serialize,
};

use context_trace::graph::snapshot::GraphSnapshot;

use crate::{
    error::{
        ApiError,
        AtomError,
        InsertError,
        PatternError,
        ReadError,
        SearchError,
        WorkspaceError,
    },
    types::{
        AtomInfo,
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

// ---------------------------------------------------------------------------
// WorkspaceApi trait
// ---------------------------------------------------------------------------

/// Strongly-typed API for all workspace operations.
///
/// `WorkspaceManager` implements this trait. Adapter layers (CLI, MCP, HTTP)
/// typically prefer the serializable `Command` / `CommandResult` path via
/// [`execute`], but Rust consumers can use this trait directly.
pub trait WorkspaceApi {
    // -- Workspace lifecycle ------------------------------------------------

    fn create_workspace(
        &mut self,
        name: &str,
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
}

// ---------------------------------------------------------------------------
// WorkspaceApi impl for WorkspaceManager
// ---------------------------------------------------------------------------

impl WorkspaceApi for WorkspaceManager {
    fn create_workspace(
        &mut self,
        name: &str,
    ) -> Result<WorkspaceInfo, WorkspaceError> {
        WorkspaceManager::create_workspace(self, name)
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
}

// ---------------------------------------------------------------------------
// Command enum (serializable)
// ---------------------------------------------------------------------------

/// A serializable command that can be dispatched to a `WorkspaceManager`.
///
/// Adapters (CLI, MCP, HTTP) deserialize incoming requests into this enum
/// and call [`execute`] to run the command and obtain a [`CommandResult`].
///
/// The enum is tagged with `#[serde(tag = "command", rename_all = "snake_case")]`
/// so that the JSON representation includes a `"command"` field identifying the
/// variant.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum Command {
    // -- Workspace lifecycle ------------------------------------------------
    CreateWorkspace {
        name: String,
    },
    OpenWorkspace {
        name: String,
    },
    CloseWorkspace {
        name: String,
    },
    SaveWorkspace {
        name: String,
    },
    ListWorkspaces,
    DeleteWorkspace {
        name: String,
    },

    // -- Atoms --------------------------------------------------------------
    AddAtom {
        workspace: String,
        ch: char,
    },
    AddAtoms {
        workspace: String,
        chars: Vec<char>,
    },
    GetAtom {
        workspace: String,
        ch: char,
    },
    ListAtoms {
        workspace: String,
    },

    // -- Patterns -----------------------------------------------------------
    AddSimplePattern {
        workspace: String,
        atoms: Vec<char>,
    },
    GetVertex {
        workspace: String,
        index: usize,
    },
    ListVertices {
        workspace: String,
    },

    // -- Search (Phase 2) ---------------------------------------------------
    SearchPattern {
        workspace: String,
        query: Vec<TokenRef>,
    },
    SearchSequence {
        workspace: String,
        text: String,
    },

    // -- Insert (Phase 2) ---------------------------------------------------
    InsertFirstMatch {
        workspace: String,
        query: Vec<TokenRef>,
    },
    InsertSequence {
        workspace: String,
        text: String,
    },
    InsertSequences {
        workspace: String,
        texts: HashSet<String>,
    },

    // -- Read (Phase 2) -----------------------------------------------------
    ReadPattern {
        workspace: String,
        index: usize,
    },
    ReadAsText {
        workspace: String,
        index: usize,
    },

    // -- Debug / Introspection ----------------------------------------------
    GetSnapshot {
        workspace: String,
    },
    GetStatistics {
        workspace: String,
    },
    ValidateGraph {
        workspace: String,
    },
    ShowGraph {
        workspace: String,
    },
    ShowVertex {
        workspace: String,
        index: usize,
    },
}

// ---------------------------------------------------------------------------
// CommandResult enum (serializable)
// ---------------------------------------------------------------------------

/// The result of executing a [`Command`].
///
/// Each variant wraps the return type of the corresponding `WorkspaceApi`
/// method. Adapters serialize this to JSON (or another format) for their
/// response.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CommandResult {
    /// Result of `create_workspace` or `open_workspace`.
    WorkspaceInfo(WorkspaceInfo),

    /// Result of `list_workspaces`.
    WorkspaceInfoList { workspaces: Vec<WorkspaceInfo> },

    /// Result of `add_atom`.
    AtomInfo(AtomInfo),

    /// Result of `add_atoms` or `list_atoms`.
    AtomInfoList { atoms: Vec<AtomInfo> },

    /// Result of `get_atom`.
    OptionalAtomInfo { atom: Option<AtomInfo> },

    /// Result of `add_simple_pattern`.
    PatternInfo(PatternInfo),

    /// Result of `get_vertex`.
    OptionalVertexInfo { vertex: Option<VertexInfo> },

    /// Result of `list_vertices`.
    TokenInfoList { tokens: Vec<TokenInfo> },

    /// Result of `search_pattern` or `search_sequence`.
    SearchResult(SearchResult),

    /// Result of `insert_first_match` or `insert_sequence`.
    InsertResult(InsertResult),

    /// Result of `insert_sequences`.
    InsertResultList { results: Vec<InsertResult> },

    /// Result of `read_pattern`.
    ReadResult(PatternReadResult),

    /// Result of `read_as_text`.
    Text { text: String },

    /// Result of `get_snapshot`.
    Snapshot(#[schemars(with = "serde_json::Value")] GraphSnapshot),

    /// Result of `get_statistics`.
    Statistics(GraphStatistics),

    /// Result of `validate_graph`.
    ValidationReport(ValidationReport),

    /// Result of `show_graph` or `show_vertex`.
    GraphDisplay { display: String },

    /// Result of commands that return `()` (close, save, delete).
    Ok,
}

// ---------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------

/// Execute a [`Command`] against a [`WorkspaceManager`] and return a
/// [`CommandResult`].
///
/// This is the primary entry point for adapter layers. Errors are mapped
/// into `ApiError` via the `From` impls on the per-domain error types.
pub fn execute(
    manager: &mut WorkspaceManager,
    cmd: Command,
) -> Result<CommandResult, ApiError> {
    match cmd {
        // -- Workspace lifecycle --------------------------------------------
        Command::CreateWorkspace { name } => {
            let info = manager.create_workspace(&name)?;
            Ok(CommandResult::WorkspaceInfo(info))
        },
        Command::OpenWorkspace { name } => {
            let info = manager.open_workspace(&name)?;
            Ok(CommandResult::WorkspaceInfo(info))
        },
        Command::CloseWorkspace { name } => {
            manager.close_workspace(&name)?;
            Ok(CommandResult::Ok)
        },
        Command::SaveWorkspace { name } => {
            manager.save_workspace(&name)?;
            Ok(CommandResult::Ok)
        },
        Command::ListWorkspaces => {
            let workspaces = manager.list_workspaces()?;
            Ok(CommandResult::WorkspaceInfoList { workspaces })
        },
        Command::DeleteWorkspace { name } => {
            manager.delete_workspace(&name)?;
            Ok(CommandResult::Ok)
        },

        // -- Atoms ----------------------------------------------------------
        Command::AddAtom { workspace, ch } => {
            let info = manager.add_atom(&workspace, ch)?;
            Ok(CommandResult::AtomInfo(info))
        },
        Command::AddAtoms { workspace, chars } => {
            let atoms = manager.add_atoms(&workspace, chars)?;
            Ok(CommandResult::AtomInfoList { atoms })
        },
        Command::GetAtom { workspace, ch } => {
            let atom = manager.get_atom(&workspace, ch)?;
            Ok(CommandResult::OptionalAtomInfo { atom })
        },
        Command::ListAtoms { workspace } => {
            let atoms = manager.list_atoms(&workspace)?;
            Ok(CommandResult::AtomInfoList { atoms })
        },

        // -- Patterns -------------------------------------------------------
        Command::AddSimplePattern { workspace, atoms } => {
            let info = manager.add_simple_pattern(&workspace, atoms)?;
            Ok(CommandResult::PatternInfo(info))
        },
        Command::GetVertex { workspace, index } => {
            let vertex = manager.get_vertex(&workspace, index)?;
            Ok(CommandResult::OptionalVertexInfo { vertex })
        },
        Command::ListVertices { workspace } => {
            let tokens = manager.list_vertices(&workspace)?;
            Ok(CommandResult::TokenInfoList { tokens })
        },

        // -- Search (Phase 2) -----------------------------------------------
        Command::SearchPattern { workspace, query } => {
            let result = manager.search_pattern(&workspace, query)?;
            Ok(CommandResult::SearchResult(result))
        },
        Command::SearchSequence { workspace, text } => {
            let result = manager.search_sequence(&workspace, &text)?;
            Ok(CommandResult::SearchResult(result))
        },

        // -- Insert (Phase 2) -----------------------------------------------
        Command::InsertFirstMatch { workspace, query } => {
            let result = manager.insert_first_match(&workspace, query)?;
            Ok(CommandResult::InsertResult(result))
        },
        Command::InsertSequence { workspace, text } => {
            let result = manager.insert_sequence(&workspace, &text)?;
            Ok(CommandResult::InsertResult(result))
        },
        Command::InsertSequences { workspace, texts } => {
            let results = manager.insert_sequences(&workspace, texts)?;
            Ok(CommandResult::InsertResultList { results })
        },

        // -- Read (Phase 2) -------------------------------------------------
        Command::ReadPattern { workspace, index } => {
            let result = manager.read_pattern(&workspace, index)?;
            Ok(CommandResult::ReadResult(result))
        },
        Command::ReadAsText { workspace, index } => {
            let text = manager.read_as_text(&workspace, index)?;
            Ok(CommandResult::Text { text })
        },

        // -- Debug / Introspection ------------------------------------------
        Command::GetSnapshot { workspace } => {
            let snapshot = manager.get_snapshot(&workspace)?;
            Ok(CommandResult::Snapshot(snapshot))
        },
        Command::GetStatistics { workspace } => {
            let statistics = manager.get_statistics(&workspace)?;
            Ok(CommandResult::Statistics(statistics))
        },
        Command::ValidateGraph { workspace } => {
            let report = manager.validate_graph(&workspace)?;
            Ok(CommandResult::ValidationReport(report))
        },
        Command::ShowGraph { workspace } => {
            let display = manager.show_graph(&workspace)?;
            Ok(CommandResult::GraphDisplay { display })
        },
        Command::ShowVertex { workspace, index } => {
            let display = manager.show_vertex(&workspace, index)?;
            Ok(CommandResult::GraphDisplay { display })
        },
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::manager::WorkspaceManager;

    /// Helper: create a `WorkspaceManager` backed by a temporary directory.
    fn tmp_manager() -> (tempfile::TempDir, WorkspaceManager) {
        let tmp = tempfile::tempdir().expect("failed to create temp dir");
        let mgr = WorkspaceManager::new(tmp.path().to_path_buf());
        (tmp, mgr)
    }

    // -- Command serde round-trip -------------------------------------------

    #[test]
    fn command_serde_create_workspace() {
        let cmd = Command::CreateWorkspace {
            name: "demo".into(),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("create_workspace"));
        assert!(json.contains("demo"));

        let deser: Command = serde_json::from_str(&json).unwrap();
        match deser {
            Command::CreateWorkspace { name } => assert_eq!(name, "demo"),
            other => panic!("expected CreateWorkspace, got: {other:?}"),
        }
    }

    #[test]
    fn command_serde_add_atom() {
        let cmd = Command::AddAtom {
            workspace: "ws".into(),
            ch: 'x',
        };
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("add_atom"));

        let deser: Command = serde_json::from_str(&json).unwrap();
        match deser {
            Command::AddAtom { workspace, ch } => {
                assert_eq!(workspace, "ws");
                assert_eq!(ch, 'x');
            },
            other => panic!("expected AddAtom, got: {other:?}"),
        }
    }

    #[test]
    fn command_serde_add_atoms() {
        let chars: Vec<char> = vec!['a', 'b', 'c'];
        let cmd = Command::AddAtoms {
            workspace: "ws".into(),
            chars: chars.clone(),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("add_atoms"));

        let deser: Command = serde_json::from_str(&json).unwrap();
        match deser {
            Command::AddAtoms {
                workspace,
                chars: deser_chars,
            } => {
                assert_eq!(workspace, "ws");
                assert_eq!(deser_chars, chars);
            },
            other => panic!("expected AddAtoms, got: {other:?}"),
        }
    }

    #[test]
    fn command_serde_add_simple_pattern() {
        let cmd = Command::AddSimplePattern {
            workspace: "ws".into(),
            atoms: vec!['a', 'b', 'c'],
        };
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("add_simple_pattern"));

        let deser: Command = serde_json::from_str(&json).unwrap();
        match deser {
            Command::AddSimplePattern { workspace, atoms } => {
                assert_eq!(workspace, "ws");
                assert_eq!(atoms, vec!['a', 'b', 'c']);
            },
            other => panic!("expected AddSimplePattern, got: {other:?}"),
        }
    }

    #[test]
    fn command_serde_list_workspaces() {
        let cmd = Command::ListWorkspaces;
        let json = serde_json::to_string(&cmd).unwrap();
        let deser: Command = serde_json::from_str(&json).unwrap();
        assert!(matches!(deser, Command::ListWorkspaces));
    }

    #[test]
    fn command_serde_search_sequence() {
        let cmd = Command::SearchSequence {
            workspace: "ws".into(),
            text: "hello".into(),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("search_sequence"));
        assert!(json.contains("hello"));

        let deser: Command = serde_json::from_str(&json).unwrap();
        match deser {
            Command::SearchSequence { workspace, text } => {
                assert_eq!(workspace, "ws");
                assert_eq!(text, "hello");
            },
            other => panic!("expected SearchSequence, got: {other:?}"),
        }
    }

    #[test]
    fn command_serde_search_pattern() {
        let cmd = Command::SearchPattern {
            workspace: "ws".into(),
            query: vec![TokenRef::Index(0), TokenRef::Label("ab".into())],
        };
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("search_pattern"));

        let deser: Command = serde_json::from_str(&json).unwrap();
        match deser {
            Command::SearchPattern { workspace, query } => {
                assert_eq!(workspace, "ws");
                assert_eq!(query.len(), 2);
                assert_eq!(query[0], TokenRef::Index(0));
                assert_eq!(query[1], TokenRef::Label("ab".into()));
            },
            other => panic!("expected SearchPattern, got: {other:?}"),
        }
    }

    #[test]
    fn command_serde_insert_sequence() {
        let cmd = Command::InsertSequence {
            workspace: "ws".into(),
            text: "world".into(),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("insert_sequence"));

        let deser: Command = serde_json::from_str(&json).unwrap();
        match deser {
            Command::InsertSequence { workspace, text } => {
                assert_eq!(workspace, "ws");
                assert_eq!(text, "world");
            },
            other => panic!("expected InsertSequence, got: {other:?}"),
        }
    }

    #[test]
    fn command_serde_insert_sequences() {
        let texts: HashSet<String> =
            ["abc", "def"].iter().map(|s| s.to_string()).collect();
        let cmd = Command::InsertSequences {
            workspace: "ws".into(),
            texts: texts.clone(),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("insert_sequences"));

        let deser: Command = serde_json::from_str(&json).unwrap();
        match deser {
            Command::InsertSequences {
                workspace,
                texts: deser_texts,
            } => {
                assert_eq!(workspace, "ws");
                assert_eq!(deser_texts, texts);
            },
            other => panic!("expected InsertSequences, got: {other:?}"),
        }
    }

    #[test]
    fn command_serde_read_pattern() {
        let cmd = Command::ReadPattern {
            workspace: "ws".into(),
            index: 42,
        };
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("read_pattern"));

        let deser: Command = serde_json::from_str(&json).unwrap();
        match deser {
            Command::ReadPattern { workspace, index } => {
                assert_eq!(workspace, "ws");
                assert_eq!(index, 42);
            },
            other => panic!("expected ReadPattern, got: {other:?}"),
        }
    }

    #[test]
    fn command_serde_read_as_text() {
        let cmd = Command::ReadAsText {
            workspace: "ws".into(),
            index: 7,
        };
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("read_as_text"));

        let deser: Command = serde_json::from_str(&json).unwrap();
        match deser {
            Command::ReadAsText { workspace, index } => {
                assert_eq!(workspace, "ws");
                assert_eq!(index, 7);
            },
            other => panic!("expected ReadAsText, got: {other:?}"),
        }
    }

    #[test]
    fn command_serde_validate_graph() {
        let cmd = Command::ValidateGraph {
            workspace: "ws".into(),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("validate_graph"));

        let deser: Command = serde_json::from_str(&json).unwrap();
        match deser {
            Command::ValidateGraph { workspace } => {
                assert_eq!(workspace, "ws");
            },
            other => panic!("expected ValidateGraph, got: {other:?}"),
        }
    }

    #[test]
    fn command_serde_all_variants_have_tag() {
        // Ensure every variant round-trips through JSON without error
        let commands = vec![
            Command::CreateWorkspace { name: "a".into() },
            Command::OpenWorkspace { name: "a".into() },
            Command::CloseWorkspace { name: "a".into() },
            Command::SaveWorkspace { name: "a".into() },
            Command::ListWorkspaces,
            Command::DeleteWorkspace { name: "a".into() },
            Command::AddAtom {
                workspace: "a".into(),
                ch: 'x',
            },
            Command::AddAtoms {
                workspace: "a".into(),
                chars: Vec::new(),
            },
            Command::GetAtom {
                workspace: "a".into(),
                ch: 'x',
            },
            Command::ListAtoms {
                workspace: "a".into(),
            },
            Command::AddSimplePattern {
                workspace: "a".into(),
                atoms: vec!['a', 'b'],
            },
            Command::GetVertex {
                workspace: "a".into(),
                index: 0,
            },
            Command::ListVertices {
                workspace: "a".into(),
            },
            Command::SearchPattern {
                workspace: "a".into(),
                query: vec![TokenRef::Index(0), TokenRef::Index(1)],
            },
            Command::SearchSequence {
                workspace: "a".into(),
                text: "ab".into(),
            },
            Command::InsertFirstMatch {
                workspace: "a".into(),
                query: vec![TokenRef::Index(0), TokenRef::Index(1)],
            },
            Command::InsertSequence {
                workspace: "a".into(),
                text: "ab".into(),
            },
            Command::InsertSequences {
                workspace: "a".into(),
                texts: HashSet::new(),
            },
            Command::ReadPattern {
                workspace: "a".into(),
                index: 0,
            },
            Command::ReadAsText {
                workspace: "a".into(),
                index: 0,
            },
            Command::GetSnapshot {
                workspace: "a".into(),
            },
            Command::GetStatistics {
                workspace: "a".into(),
            },
            Command::ValidateGraph {
                workspace: "a".into(),
            },
        ];

        for cmd in commands {
            let json = serde_json::to_string(&cmd)
                .unwrap_or_else(|e| panic!("failed to serialize {cmd:?}: {e}"));
            assert!(
                json.contains("\"command\""),
                "JSON should have a 'command' tag: {json}"
            );
            let _: Command = serde_json::from_str(&json).unwrap_or_else(|e| {
                panic!("failed to deserialize {json}: {e}")
            });
        }
    }

    // -- CommandResult serde ------------------------------------------------

    #[test]
    fn result_serde_ok() {
        let result = CommandResult::Ok;
        let json = serde_json::to_string(&result).unwrap();
        let deser: CommandResult = serde_json::from_str(&json).unwrap();
        assert!(matches!(deser, CommandResult::Ok));
    }

    #[test]
    fn result_serde_atom_info() {
        let result = CommandResult::AtomInfo(AtomInfo { index: 0, ch: 'a' });
        let json = serde_json::to_string(&result).unwrap();
        let deser: CommandResult = serde_json::from_str(&json).unwrap();
        match deser {
            CommandResult::AtomInfo(info) => {
                assert_eq!(info.index, 0);
                assert_eq!(info.ch, 'a');
            },
            other => panic!("expected AtomInfo, got: {other:?}"),
        }
    }

    #[test]
    fn result_serde_workspace_info() {
        let info = WorkspaceInfo {
            name: "demo".into(),
            vertex_count: 10,
            atom_count: 5,
            pattern_count: 5,
            created_at: "2025-01-01T00:00:00Z".into(),
            modified_at: "2025-01-02T00:00:00Z".into(),
        };
        let result = CommandResult::WorkspaceInfo(info);
        let json = serde_json::to_string(&result).unwrap();
        let deser: CommandResult = serde_json::from_str(&json).unwrap();
        match deser {
            CommandResult::WorkspaceInfo(i) => {
                assert_eq!(i.name, "demo");
                assert_eq!(i.vertex_count, 10);
            },
            other => panic!("expected WorkspaceInfo, got: {other:?}"),
        }
    }

    #[test]
    fn result_serde_search_result() {
        let result = CommandResult::SearchResult(SearchResult {
            complete: true,
            token: Some(TokenInfo {
                index: 5,
                label: "ab".into(),
                width: 2,
            }),
            query_exhausted: true,
            partial: None,
        });
        let json = serde_json::to_string(&result).unwrap();
        let deser: CommandResult = serde_json::from_str(&json).unwrap();
        match deser {
            CommandResult::SearchResult(sr) => {
                assert!(sr.complete);
                assert!(sr.token.is_some());
            },
            other => panic!("expected SearchResult, got: {other:?}"),
        }
    }

    #[test]
    fn result_serde_insert_result() {
        let result = CommandResult::InsertResult(InsertResult {
            token: TokenInfo {
                index: 7,
                label: "hello".into(),
                width: 5,
            },
            already_existed: false,
        });
        let json = serde_json::to_string(&result).unwrap();
        let deser: CommandResult = serde_json::from_str(&json).unwrap();
        match deser {
            CommandResult::InsertResult(ir) => {
                assert!(!ir.already_existed);
                assert_eq!(ir.token.label, "hello");
            },
            other => panic!("expected InsertResult, got: {other:?}"),
        }
    }

    #[test]
    fn result_serde_text() {
        let result = CommandResult::Text {
            text: "hello world".into(),
        };
        let json = serde_json::to_string(&result).unwrap();
        let deser: CommandResult = serde_json::from_str(&json).unwrap();
        match deser {
            CommandResult::Text { text } => {
                assert_eq!(text, "hello world");
            },
            other => panic!("expected Text, got: {other:?}"),
        }
    }

    #[test]
    fn result_serde_validation_report() {
        let result = CommandResult::ValidationReport(ValidationReport {
            valid: true,
            vertex_count: 5,
            issues: vec![],
        });
        let json = serde_json::to_string(&result).unwrap();
        let deser: CommandResult = serde_json::from_str(&json).unwrap();
        match deser {
            CommandResult::ValidationReport(vr) => {
                assert!(vr.valid);
                assert_eq!(vr.vertex_count, 5);
            },
            other => panic!("expected ValidationReport, got: {other:?}"),
        }
    }

    // -- execute() dispatch -------------------------------------------------

    #[test]
    fn execute_create_workspace() {
        let (_tmp, mut mgr) = tmp_manager();

        let result = execute(
            &mut mgr,
            Command::CreateWorkspace {
                name: "test".into(),
            },
        )
        .unwrap();

        match result {
            CommandResult::WorkspaceInfo(info) => {
                assert_eq!(info.name, "test");
                assert_eq!(info.vertex_count, 0);
            },
            other => panic!("expected WorkspaceInfo, got: {other:?}"),
        }
    }

    #[test]
    fn execute_add_atom() {
        let (_tmp, mut mgr) = tmp_manager();
        execute(&mut mgr, Command::CreateWorkspace { name: "ws".into() })
            .unwrap();

        let result = execute(
            &mut mgr,
            Command::AddAtom {
                workspace: "ws".into(),
                ch: 'a',
            },
        )
        .unwrap();

        match result {
            CommandResult::AtomInfo(info) => assert_eq!(info.ch, 'a'),
            other => panic!("expected AtomInfo, got: {other:?}"),
        }
    }

    #[test]
    fn execute_full_workflow() {
        let (_tmp, mut mgr) = tmp_manager();

        // Create workspace
        execute(&mut mgr, Command::CreateWorkspace { name: "ws".into() })
            .unwrap();

        // Add atoms
        let chars: Vec<char> = vec!['a', 'b', 'c'];
        execute(
            &mut mgr,
            Command::AddAtoms {
                workspace: "ws".into(),
                chars,
            },
        )
        .unwrap();

        // Create simple pattern
        execute(
            &mut mgr,
            Command::AddSimplePattern {
                workspace: "ws".into(),
                atoms: vec!['a', 'b'],
            },
        )
        .unwrap();

        // Search for it
        let search = execute(
            &mut mgr,
            Command::SearchSequence {
                workspace: "ws".into(),
                text: "ab".into(),
            },
        )
        .unwrap();
        match &search {
            CommandResult::SearchResult(sr) => {
                assert!(sr.complete, "should find 'ab'");
            },
            other => panic!("expected SearchResult, got: {other:?}"),
        }

        // List vertices
        let vertices = execute(
            &mut mgr,
            Command::ListVertices {
                workspace: "ws".into(),
            },
        )
        .unwrap();
        match &vertices {
            CommandResult::TokenInfoList { tokens } => {
                assert_eq!(tokens.len(), 4); // a, b, c, ab (3 atoms + 1 pattern)
            },
            other => panic!("expected TokenInfoList, got: {other:?}"),
        }

        // Get snapshot
        let snap = execute(
            &mut mgr,
            Command::GetSnapshot {
                workspace: "ws".into(),
            },
        )
        .unwrap();
        assert!(matches!(snap, CommandResult::Snapshot(_)));

        // Get statistics
        let stats = execute(
            &mut mgr,
            Command::GetStatistics {
                workspace: "ws".into(),
            },
        )
        .unwrap();
        match &stats {
            CommandResult::Statistics(s) => {
                assert_eq!(s.vertex_count, 4);
                assert_eq!(s.atom_count, 3);
                assert_eq!(s.pattern_count, 1);
            },
            other => panic!("expected Statistics, got: {other:?}"),
        }

        // Validate
        let validation = execute(
            &mut mgr,
            Command::ValidateGraph {
                workspace: "ws".into(),
            },
        )
        .unwrap();
        match &validation {
            CommandResult::ValidationReport(vr) => {
                assert!(vr.valid);
            },
            other => panic!("expected ValidationReport, got: {other:?}"),
        }

        // Save
        execute(&mut mgr, Command::SaveWorkspace { name: "ws".into() })
            .unwrap();

        // Close
        execute(&mut mgr, Command::CloseWorkspace { name: "ws".into() })
            .unwrap();
    }

    #[test]
    fn execute_insert_and_read_workflow() {
        let (_tmp, mut mgr) = tmp_manager();

        // Create workspace
        execute(&mut mgr, Command::CreateWorkspace { name: "ws".into() })
            .unwrap();

        // Insert a sequence (use a word without duplicate chars so search works)
        let insert = execute(
            &mut mgr,
            Command::InsertSequence {
                workspace: "ws".into(),
                text: "world".into(),
            },
        )
        .unwrap();
        let insert_index = match &insert {
            CommandResult::InsertResult(ir) => {
                assert!(!ir.already_existed);
                assert_eq!(ir.token.width, 5);
                ir.token.index
            },
            other => panic!("expected InsertResult, got: {other:?}"),
        };

        // Read as text
        let text = execute(
            &mut mgr,
            Command::ReadAsText {
                workspace: "ws".into(),
                index: insert_index,
            },
        )
        .unwrap();
        match &text {
            CommandResult::Text { text } => {
                assert_eq!(text, "world");
            },
            other => panic!("expected Text, got: {other:?}"),
        }

        // Read pattern
        let read = execute(
            &mut mgr,
            Command::ReadPattern {
                workspace: "ws".into(),
                index: insert_index,
            },
        )
        .unwrap();
        match &read {
            CommandResult::ReadResult(rr) => {
                assert_eq!(rr.text, "world");
                assert_eq!(rr.root.width, 5);
            },
            other => panic!("expected ReadResult, got: {other:?}"),
        }

        // Search for the same sequence
        let search = execute(
            &mut mgr,
            Command::SearchSequence {
                workspace: "ws".into(),
                text: "world".into(),
            },
        )
        .unwrap();
        match &search {
            CommandResult::SearchResult(sr) => {
                assert!(sr.complete, "should find inserted 'world'");
                assert_eq!(sr.token.as_ref().unwrap().index, insert_index);
            },
            other => panic!("expected SearchResult, got: {other:?}"),
        }
    }

    #[test]
    fn execute_error_propagation() {
        let (_tmp, mut mgr) = tmp_manager();

        // Trying to add an atom to a nonexistent workspace
        let result = execute(
            &mut mgr,
            Command::AddAtom {
                workspace: "nope".into(),
                ch: 'x',
            },
        );
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), "atom");
    }

    #[test]
    fn execute_list_workspaces_empty() {
        let (_tmp, mut mgr) = tmp_manager();
        let result = execute(&mut mgr, Command::ListWorkspaces).unwrap();
        match result {
            CommandResult::WorkspaceInfoList { workspaces } => {
                assert!(workspaces.is_empty());
            },
            other => panic!("expected WorkspaceInfoList, got: {other:?}"),
        }
    }

    #[test]
    fn execute_get_atom() {
        let (_tmp, mut mgr) = tmp_manager();
        execute(&mut mgr, Command::CreateWorkspace { name: "ws".into() })
            .unwrap();

        // Get non-existent atom
        let result = execute(
            &mut mgr,
            Command::GetAtom {
                workspace: "ws".into(),
                ch: 'z',
            },
        )
        .unwrap();
        match &result {
            CommandResult::OptionalAtomInfo { atom } => {
                assert!(atom.is_none());
            },
            other => panic!("expected OptionalAtomInfo, got: {other:?}"),
        }

        // Add atom, then get it
        execute(
            &mut mgr,
            Command::AddAtom {
                workspace: "ws".into(),
                ch: 'z',
            },
        )
        .unwrap();

        let result = execute(
            &mut mgr,
            Command::GetAtom {
                workspace: "ws".into(),
                ch: 'z',
            },
        )
        .unwrap();
        match &result {
            CommandResult::OptionalAtomInfo { atom } => {
                assert!(atom.is_some());
                assert_eq!(atom.as_ref().unwrap().ch, 'z');
            },
            other => panic!("expected OptionalAtomInfo, got: {other:?}"),
        }
    }

    #[test]
    fn execute_get_vertex() {
        let (_tmp, mut mgr) = tmp_manager();
        execute(&mut mgr, Command::CreateWorkspace { name: "ws".into() })
            .unwrap();
        execute(
            &mut mgr,
            Command::AddAtom {
                workspace: "ws".into(),
                ch: 'a',
            },
        )
        .unwrap();

        // Get vertex at index 0
        let result = execute(
            &mut mgr,
            Command::GetVertex {
                workspace: "ws".into(),
                index: 0,
            },
        )
        .unwrap();
        match result {
            CommandResult::OptionalVertexInfo { vertex } => {
                assert!(vertex.is_some());
            },
            other => panic!("expected OptionalVertexInfo, got: {other:?}"),
        }
    }

    #[test]
    fn execute_delete_workspace() {
        let (_tmp, mut mgr) = tmp_manager();
        execute(&mut mgr, Command::CreateWorkspace { name: "ws".into() })
            .unwrap();
        let result =
            execute(&mut mgr, Command::DeleteWorkspace { name: "ws".into() })
                .unwrap();
        assert!(matches!(result, CommandResult::Ok));
    }

    #[test]
    fn execute_snapshot() {
        let (_tmp, mut mgr) = tmp_manager();
        execute(&mut mgr, Command::CreateWorkspace { name: "ws".into() })
            .unwrap();
        execute(
            &mut mgr,
            Command::AddAtom {
                workspace: "ws".into(),
                ch: 'a',
            },
        )
        .unwrap();

        let result = execute(
            &mut mgr,
            Command::GetSnapshot {
                workspace: "ws".into(),
            },
        )
        .unwrap();
        match result {
            CommandResult::Snapshot(snapshot) => {
                assert_eq!(snapshot.nodes.len(), 1);
            },
            other => panic!("expected Snapshot, got: {other:?}"),
        }
    }

    #[test]
    fn execute_insert_first_match_via_command() {
        let (_tmp, mut mgr) = tmp_manager();
        execute(&mut mgr, Command::CreateWorkspace { name: "ws".into() })
            .unwrap();

        // Add atoms
        let chars: Vec<char> = vec!['a', 'b'];
        execute(
            &mut mgr,
            Command::AddAtoms {
                workspace: "ws".into(),
                chars,
            },
        )
        .unwrap();

        // Get atom indices
        let a_result = execute(
            &mut mgr,
            Command::GetAtom {
                workspace: "ws".into(),
                ch: 'a',
            },
        )
        .unwrap();
        let a_index = match &a_result {
            CommandResult::OptionalAtomInfo { atom: Some(a) } => a.index,
            other => panic!("expected atom a, got: {other:?}"),
        };

        let b_result = execute(
            &mut mgr,
            Command::GetAtom {
                workspace: "ws".into(),
                ch: 'b',
            },
        )
        .unwrap();
        let b_index = match &b_result {
            CommandResult::OptionalAtomInfo { atom: Some(b) } => b.index,
            other => panic!("expected atom b, got: {other:?}"),
        };

        // Insert first match by index
        let result = execute(
            &mut mgr,
            Command::InsertFirstMatch {
                workspace: "ws".into(),
                query: vec![TokenRef::Index(a_index), TokenRef::Index(b_index)],
            },
        )
        .unwrap();
        match &result {
            CommandResult::InsertResult(ir) => {
                assert!(!ir.already_existed);
                assert_eq!(ir.token.label, "ab");
            },
            other => panic!("expected InsertResult, got: {other:?}"),
        }
    }

    #[test]
    fn execute_insert_sequences_via_command() {
        let (_tmp, mut mgr) = tmp_manager();
        execute(&mut mgr, Command::CreateWorkspace { name: "ws".into() })
            .unwrap();

        let texts: HashSet<String> =
            ["abc", "def"].iter().map(|s| s.to_string()).collect();

        let result = execute(
            &mut mgr,
            Command::InsertSequences {
                workspace: "ws".into(),
                texts,
            },
        )
        .unwrap();
        match &result {
            CommandResult::InsertResultList { results } => {
                assert_eq!(results.len(), 2);
            },
            other => panic!("expected InsertResultList, got: {other:?}"),
        }
    }

    #[test]
    fn workspace_api_trait_is_object_safe_enough() {
        // Verify that WorkspaceApi can be used as a trait object
        // (we don't use dyn dispatch currently, but this guards against
        // accidental breaking of object safety).
        fn use_api(
            api: &mut dyn WorkspaceApi,
            name: &str,
        ) -> Result<WorkspaceInfo, WorkspaceError> {
            let info = api.create_workspace(name)?;
            let _ = api.list_workspaces()?;
            let _ = api.save_workspace(name)?;
            Ok(info)
        }

        let (_tmp, mut mgr) = tmp_manager();
        let result = use_api(&mut mgr, "trait-test");
        assert!(result.is_ok());
    }

    #[test]
    fn api_error_to_error_response_round_trip() {
        use crate::error::ErrorResponse;

        let api_err = ApiError::Search(SearchError::QueryTooShort);
        let resp = ErrorResponse::from(&api_err);
        assert_eq!(resp.kind, "search");
        assert!(resp.message.contains("too short"));

        let json = serde_json::to_string(&resp).unwrap();
        let deser: ErrorResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.kind, resp.kind);
    }
}
