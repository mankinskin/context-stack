use std::collections::HashSet;

use serde::{
    Deserialize,
    Serialize,
};

use crate::{
    ts_export,
    types::{
        CompareMode,
        TokenRef,
    },
};

use super::export_import::ExportFormat;

ts_export! {
    /// A serializable command that can be dispatched to a `WorkspaceManager`.
    ///
    /// Adapters (CLI, MCP, HTTP) deserialize incoming requests into this enum
    /// and call [`super::execute`] to run the command and obtain a [`super::CommandResult`].
    ///
    /// The enum is tagged with `#[serde(tag = "command", rename_all = "snake_case")]`
    /// so that the JSON representation includes a `"command"` field identifying the
    /// variant.
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    #[serde(tag = "command", rename_all = "snake_case")]
    pub enum Command {
        CreateWorkspace {
            name: String,
        },
        CreateWorkspaceFromNgrams {
            name: String,
            text: String,
            #[serde(default = "default_ngrams_timeout_secs")]
            timeout_secs: u64,
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

        SearchPattern {
            workspace: String,
            query: Vec<TokenRef>,
        },
        SearchSequence {
            workspace: String,
            text: String,
        },

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

        ReadPattern {
            workspace: String,
            index: usize,
        },
        ReadAsText {
            workspace: String,
            index: usize,
        },
        ReadSequence {
            workspace: String,
            text: String,
        },
        ReadFile {
            workspace: String,
            path: String,
        },

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
        RenderAsciiGraph {
            workspace: String,
            #[serde(default)]
            ascii_dag: bool,
        },

        ListLogs {
            workspace: String,
            #[serde(default)]
            pattern: Option<String>,
            #[serde(default = "default_log_limit")]
            limit: usize,
        },
        GetLog {
            workspace: String,
            filename: String,
            #[serde(default)]
            filter: Option<String>,
            #[serde(default = "default_log_limit")]
            limit: usize,
            #[serde(default)]
            offset: usize,
        },
        QueryLog {
            workspace: String,
            filename: String,
            query: String,
            #[serde(default = "default_query_limit")]
            limit: usize,
        },
        AnalyzeLog {
            workspace: String,
            filename: String,
        },
        SearchLogs {
            workspace: String,
            query: String,
            #[serde(default = "default_search_limit_per_file")]
            limit_per_file: usize,
        },
        DeleteLog {
            workspace: String,
            filename: String,
        },
        DeleteLogs {
            workspace: String,
            #[serde(default)]
            older_than_days: Option<u32>,
        },

        ExportWorkspace {
            workspace: String,
            format: ExportFormat,
            #[serde(default)]
            path: Option<String>,
        },
        ImportWorkspace {
            name: String,
            path: String,
            #[serde(default)]
            overwrite: bool,
        },

        CompareWorkspaces {
            workspace_a: String,
            workspace_b: String,
            #[serde(default)]
            mode: CompareMode,
        },
        CompareVertices {
            workspace_a: String,
            index_a: usize,
            workspace_b: String,
            index_b: usize,
        },
    }
}

fn default_log_limit() -> usize {
    100
}

fn default_query_limit() -> usize {
    100
}

fn default_search_limit_per_file() -> usize {
    10
}

fn default_ngrams_timeout_secs() -> u64 {
    60
}

impl Command {
    pub fn command_name(&self) -> &'static str {
        match self {
            Command::CreateWorkspace { .. }
            | Command::CreateWorkspaceFromNgrams { .. }
            | Command::OpenWorkspace { .. }
            | Command::CloseWorkspace { .. }
            | Command::SaveWorkspace { .. }
            | Command::ListWorkspaces
            | Command::DeleteWorkspace { .. } => workspace_command_name(self),
            Command::AddAtom { .. }
            | Command::AddAtoms { .. }
            | Command::GetAtom { .. }
            | Command::ListAtoms { .. }
            | Command::AddSimplePattern { .. }
            | Command::GetVertex { .. }
            | Command::ListVertices { .. } => graph_command_name(self),
            Command::SearchPattern { .. }
            | Command::SearchSequence { .. }
            | Command::InsertFirstMatch { .. }
            | Command::InsertSequence { .. }
            | Command::InsertSequences { .. }
            | Command::ReadPattern { .. }
            | Command::ReadAsText { .. }
            | Command::ReadSequence { .. }
            | Command::ReadFile { .. } => traversal_command_name(self),
            Command::GetSnapshot { .. }
            | Command::GetStatistics { .. }
            | Command::ValidateGraph { .. }
            | Command::ShowGraph { .. }
            | Command::ShowVertex { .. }
            | Command::RenderAsciiGraph { .. } => inspection_command_name(self),
            Command::ListLogs { .. }
            | Command::GetLog { .. }
            | Command::QueryLog { .. }
            | Command::AnalyzeLog { .. }
            | Command::SearchLogs { .. }
            | Command::DeleteLog { .. }
            | Command::DeleteLogs { .. } => log_command_name(self),
            Command::ExportWorkspace { .. }
            | Command::ImportWorkspace { .. }
            | Command::CompareWorkspaces { .. }
            | Command::CompareVertices { .. } => integration_command_name(self),
        }
    }
}

fn workspace_command_name(cmd: &Command) -> &'static str {
    match cmd {
        Command::CreateWorkspace { .. } => "create_workspace",
        Command::CreateWorkspaceFromNgrams { .. } =>
            "create_workspace_from_ngrams",
        Command::OpenWorkspace { .. } => "open_workspace",
        Command::CloseWorkspace { .. } => "close_workspace",
        Command::SaveWorkspace { .. } => "save_workspace",
        Command::ListWorkspaces => "list_workspaces",
        Command::DeleteWorkspace { .. } => "delete_workspace",
        _ => unreachable!("workspace command expected"),
    }
}

fn graph_command_name(cmd: &Command) -> &'static str {
    match cmd {
        Command::AddAtom { .. } => "add_atom",
        Command::AddAtoms { .. } => "add_atoms",
        Command::GetAtom { .. } => "get_atom",
        Command::ListAtoms { .. } => "list_atoms",
        Command::AddSimplePattern { .. } => "add_simple_pattern",
        Command::GetVertex { .. } => "get_vertex",
        Command::ListVertices { .. } => "list_vertices",
        _ => unreachable!("graph command expected"),
    }
}

fn traversal_command_name(cmd: &Command) -> &'static str {
    match cmd {
        Command::SearchPattern { .. } => "search_pattern",
        Command::SearchSequence { .. } => "search_sequence",
        Command::InsertFirstMatch { .. } => "insert_first_match",
        Command::InsertSequence { .. } => "insert_sequence",
        Command::InsertSequences { .. } => "insert_sequences",
        Command::ReadPattern { .. } => "read_pattern",
        Command::ReadAsText { .. } => "read_as_text",
        Command::ReadSequence { .. } => "read_sequence",
        Command::ReadFile { .. } => "read_file",
        _ => unreachable!("traversal command expected"),
    }
}

fn inspection_command_name(cmd: &Command) -> &'static str {
    match cmd {
        Command::GetSnapshot { .. } => "get_snapshot",
        Command::GetStatistics { .. } => "get_statistics",
        Command::ValidateGraph { .. } => "validate_graph",
        Command::ShowGraph { .. } => "show_graph",
        Command::ShowVertex { .. } => "show_vertex",
        Command::RenderAsciiGraph { .. } => "render_ascii_graph",
        _ => unreachable!("inspection command expected"),
    }
}

fn log_command_name(cmd: &Command) -> &'static str {
    match cmd {
        Command::ListLogs { .. } => "list_logs",
        Command::GetLog { .. } => "get_log",
        Command::QueryLog { .. } => "query_log",
        Command::AnalyzeLog { .. } => "analyze_log",
        Command::SearchLogs { .. } => "search_logs",
        Command::DeleteLog { .. } => "delete_log",
        Command::DeleteLogs { .. } => "delete_logs",
        _ => unreachable!("log command expected"),
    }
}

fn integration_command_name(cmd: &Command) -> &'static str {
    match cmd {
        Command::ExportWorkspace { .. } => "export_workspace",
        Command::ImportWorkspace { .. } => "import_workspace",
        Command::CompareWorkspaces { .. } => "compare_workspaces",
        Command::CompareVertices { .. } => "compare_vertices",
        _ => unreachable!("integration command expected"),
    }
}
