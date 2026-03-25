//! Shared test utilities for context-cli integration tests.
//!
//! Provides workspace lifecycle helpers, command shorthands, and assertion
//! utilities. Every test gets an isolated temporary directory that is
//! cleaned up automatically when the `TestWorkspace` guard is dropped.

use context_api::{
    commands::{
        Command,
        CommandResult,
        execute,
    },
    types::{
        AtomInfo,
        InsertResult,
        PatternReadResult,
    },
    workspace::manager::WorkspaceManager,
};
use std::path::PathBuf;
use tempfile::TempDir;

/// A self-contained test workspace with automatic cleanup.
pub struct TestWorkspace {
    pub manager: WorkspaceManager,
    pub name: String,
    _temp_dir: TempDir,
}

impl TestWorkspace {
    /// Create a new isolated test workspace.
    pub fn new(name: &str) -> Self {
        let temp_dir = TempDir::new().expect("failed to create temp directory");
        let mut manager = WorkspaceManager::new(temp_dir.path().to_path_buf());

        let cmd = Command::CreateWorkspace {
            name: name.to_string(),
        };
        execute(&mut manager, cmd).expect("failed to create test workspace");

        Self {
            manager,
            name: name.to_string(),
            _temp_dir: temp_dir,
        }
    }

    /// Execute a command against this workspace's manager.
    pub fn exec(
        &mut self,
        cmd: Command,
    ) -> Result<CommandResult, context_api::error::ApiError> {
        execute(&mut self.manager, cmd)
    }

    /// Shorthand: insert a text sequence into this workspace.
    pub fn insert_text(
        &mut self,
        text: &str,
    ) -> CommandResult {
        let cmd = Command::InsertSequence {
            workspace: self.name.clone(),
            text: text.to_string(),
        };
        self.exec(cmd)
            .unwrap_or_else(|e| panic!("insert_text({text:?}) failed: {e}"))
    }

    /// Shorthand: read a text sequence through the graph.
    pub fn read_sequence(
        &mut self,
        text: &str,
    ) -> CommandResult {
        let cmd = Command::ReadSequence {
            workspace: self.name.clone(),
            text: text.to_string(),
        };
        self.exec(cmd)
            .unwrap_or_else(|e| panic!("read_sequence({text:?}) failed: {e}"))
    }

    /// Shorthand: read a file through the graph.
    pub fn read_file(
        &mut self,
        path: &str,
    ) -> Result<CommandResult, context_api::error::ApiError> {
        let cmd = Command::ReadFile {
            workspace: self.name.clone(),
            path: path.to_string(),
        };
        self.exec(cmd)
    }

    /// Shorthand: read a pattern by vertex index.
    pub fn read_pattern(
        &mut self,
        index: usize,
    ) -> CommandResult {
        let cmd = Command::ReadPattern {
            workspace: self.name.clone(),
            index,
        };
        self.exec(cmd)
            .unwrap_or_else(|e| panic!("read_pattern({index}) failed: {e}"))
    }

    /// Shorthand: read as text by vertex index.
    pub fn read_as_text(
        &mut self,
        index: usize,
    ) -> CommandResult {
        let cmd = Command::ReadAsText {
            workspace: self.name.clone(),
            index,
        };
        self.exec(cmd)
            .unwrap_or_else(|e| panic!("read_as_text({index}) failed: {e}"))
    }

    #[allow(dead_code)]
    /// Shorthand: search for a text sequence.
    pub fn search_text(
        &mut self,
        text: &str,
    ) -> CommandResult {
        let cmd = Command::SearchSequence {
            workspace: self.name.clone(),
            text: text.to_string(),
        };
        self.exec(cmd)
            .unwrap_or_else(|e| panic!("search_text({text:?}) failed: {e}"))
    }

    /// Shorthand: add a single atom.
    pub fn add_atom(
        &mut self,
        ch: char,
    ) -> CommandResult {
        let cmd = Command::AddAtom {
            workspace: self.name.clone(),
            ch,
        };
        self.exec(cmd)
            .unwrap_or_else(|e| panic!("add_atom({ch:?}) failed: {e}"))
    }

    #[allow(dead_code)]
    /// Shorthand: add multiple atoms from a string.
    pub fn add_atoms(
        &mut self,
        chars: &str,
    ) -> CommandResult {
        let cmd = Command::AddAtoms {
            workspace: self.name.clone(),
            chars: chars.chars().collect(),
        };
        self.exec(cmd)
            .unwrap_or_else(|e| panic!("add_atoms({chars:?}) failed: {e}"))
    }

    /// Shorthand: list all atoms.
    pub fn list_atoms(&mut self) -> CommandResult {
        let cmd = Command::ListAtoms {
            workspace: self.name.clone(),
        };
        self.exec(cmd)
            .unwrap_or_else(|e| panic!("list_atoms failed: {e}"))
    }

    /// Shorthand: get a vertex by index.
    pub fn get_vertex(
        &mut self,
        index: usize,
    ) -> CommandResult {
        let cmd = Command::GetVertex {
            workspace: self.name.clone(),
            index,
        };
        self.exec(cmd)
            .unwrap_or_else(|e| panic!("get_vertex({index}) failed: {e}"))
    }

    #[allow(dead_code)]
    /// Shorthand: list all vertices.
    pub fn list_vertices(&mut self) -> CommandResult {
        let cmd = Command::ListVertices {
            workspace: self.name.clone(),
        };
        self.exec(cmd)
            .unwrap_or_else(|e| panic!("list_vertices failed: {e}"))
    }

    /// Shorthand: validate graph integrity.
    pub fn validate_graph(&mut self) -> CommandResult {
        let cmd = Command::ValidateGraph {
            workspace: self.name.clone(),
        };
        self.exec(cmd)
            .unwrap_or_else(|e| panic!("validate_graph failed: {e}"))
    }

    /// Shorthand: get workspace statistics.
    pub fn get_statistics(&mut self) -> CommandResult {
        let cmd = Command::GetStatistics {
            workspace: self.name.clone(),
        };
        self.exec(cmd)
            .unwrap_or_else(|e| panic!("get_statistics failed: {e}"))
    }

    /// Shorthand: get a full graph snapshot for this workspace.
    pub fn get_snapshot(&mut self) -> context_api::types::Snapshot {
        use context_api::commands::{
            Command,
            CommandResult,
        };
        let cmd = Command::GetSnapshot {
            workspace: self.name.clone(),
        };
        match self
            .exec(cmd)
            .unwrap_or_else(|e| panic!("get_snapshot failed: {e}"))
        {
            CommandResult::Snapshot(snap) => snap,
            other => panic!("expected CommandResult::Snapshot, got {other:?}"),
        }
    }

    /// Return the base directory for this workspace.
    pub fn base_dir(&self) -> PathBuf {
        self._temp_dir.path().to_path_buf()
    }
}

// -----------------------------------------------------------------------
// Assertion helpers
// -----------------------------------------------------------------------

/// Extract the text string from a `CommandResult::Text` variant.
pub fn unwrap_text(result: &CommandResult) -> &str {
    match result {
        CommandResult::Text { text } => text.as_str(),
        other => panic!("expected CommandResult::Text, got {other:?}"),
    }
}

/// Extract atom info from a `CommandResult::AtomInfo` variant.
pub fn unwrap_atom_info(result: &CommandResult) -> &AtomInfo {
    match result {
        CommandResult::AtomInfo(info) => info,
        other => panic!("expected CommandResult::AtomInfo, got {other:?}"),
    }
}

/// Extract the atom list from a `CommandResult::AtomInfoList` variant.
pub fn unwrap_atom_list(result: &CommandResult) -> &[AtomInfo] {
    match result {
        CommandResult::AtomInfoList { atoms } => atoms.as_slice(),
        other => panic!("expected CommandResult::AtomInfoList, got {other:?}"),
    }
}

/// Extract insert result from a `CommandResult::InsertResult` variant.
pub fn unwrap_insert_result(result: &CommandResult) -> &InsertResult {
    match result {
        CommandResult::InsertResult(info) => info,
        other => panic!("expected CommandResult::InsertResult, got {other:?}"),
    }
}

/// Extract read result from a `CommandResult::ReadResult` variant.
pub fn unwrap_read_result(result: &CommandResult) -> &PatternReadResult {
    match result {
        CommandResult::ReadResult(result) => result,
        other => panic!("expected CommandResult::ReadResult, got {other:?}"),
    }
}

/// Extract statistics from a `CommandResult::Statistics` variant.
pub fn unwrap_statistics(
    result: &CommandResult
) -> &context_api::types::GraphStatistics {
    match result {
        CommandResult::Statistics(stats) => stats,
        other => panic!("expected CommandResult::Statistics, got {other:?}"),
    }
}
