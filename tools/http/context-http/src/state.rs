//! Application state shared across HTTP handlers.

use std::{
    path::PathBuf,
    sync::{
        Arc,
        Mutex,
    },
};

use context_api::{
    commands::Command,
    tracing_capture::CaptureConfig,
    workspace::manager::WorkspaceManager,
};

/// Extract the workspace name from a [`Command`], if the variant carries one.
///
/// Workspace-lifecycle commands use a `name` field; all other commands use
/// `workspace`. Commands like `ListWorkspaces` don't reference any workspace
/// and return `None`.
fn workspace_name_of(cmd: &Command) -> Option<&str> {
    match cmd {
        // Workspace lifecycle — field is called `name`
        Command::CreateWorkspace { name }
        | Command::OpenWorkspace { name }
        | Command::CloseWorkspace { name }
        | Command::SaveWorkspace { name }
        | Command::DeleteWorkspace { name }
        | Command::ImportWorkspace { name, .. } => Some(name),

        // No workspace
        Command::ListWorkspaces => None,

        // Everything else uses `workspace`
        Command::AddAtom { workspace, .. }
        | Command::AddAtoms { workspace, .. }
        | Command::GetAtom { workspace, .. }
        | Command::ListAtoms { workspace }
        | Command::AddSimplePattern { workspace, .. }
        | Command::GetVertex { workspace, .. }
        | Command::ListVertices { workspace }
        | Command::SearchPattern { workspace, .. }
        | Command::SearchSequence { workspace, .. }
        | Command::InsertFirstMatch { workspace, .. }
        | Command::InsertSequence { workspace, .. }
        | Command::InsertSequences { workspace, .. }
        | Command::ReadPattern { workspace, .. }
        | Command::ReadAsText { workspace, .. }
        | Command::ReadSequence { workspace, .. }
        | Command::ReadFile { workspace, .. }
        | Command::GetSnapshot { workspace }
        | Command::GetStatistics { workspace }
        | Command::ValidateGraph { workspace }
        | Command::ShowGraph { workspace }
        | Command::ShowVertex { workspace, .. }
        | Command::ListLogs { workspace, .. }
        | Command::GetLog { workspace, .. }
        | Command::QueryLog { workspace, .. }
        | Command::AnalyzeLog { workspace, .. }
        | Command::SearchLogs { workspace, .. }
        | Command::DeleteLog { workspace, .. }
        | Command::DeleteLogs { workspace, .. }
        | Command::ExportWorkspace { workspace, .. } => Some(workspace),

        // Compare commands — span two workspaces; return workspace_a as the
        // primary for tracing purposes (or None to skip tracing).
        Command::CompareWorkspaces { workspace_a, .. }
        | Command::CompareVertices { workspace_a, .. } => Some(workspace_a),

        // Commands that don't fit a single workspace
        Command::CreateWorkspaceFromNgrams { name, .. } => Some(name),
        Command::RenderAsciiGraph { workspace, .. } => Some(workspace),
    }
}

/// Shared application state for HTTP handlers.
///
/// Wraps the [`WorkspaceManager`] in an `Arc<Mutex<_>>` so it can be shared
/// across concurrent Axum handler tasks. The `Mutex` is required because
/// [`WorkspaceManager`] methods take `&mut self`.
///
/// The `Arc` allows cheap cloning (Axum requires `Clone` for state), while
/// the `Mutex` serialises access to the underlying manager.
#[derive(Clone)]
pub struct AppState {
    /// The workspace manager (thread-safe via `Arc<Mutex<_>>`).
    pub manager: Arc<Mutex<WorkspaceManager>>,
}

impl AppState {
    /// Create a new `AppState` from an owned [`WorkspaceManager`].
    pub fn new(manager: WorkspaceManager) -> Self {
        Self {
            manager: Arc::new(Mutex::new(manager)),
        }
    }

    /// Create an `AppState` backed by a temporary in-memory directory.
    ///
    /// Useful for tests where no persistent storage is needed. The
    /// returned state uses the current working directory as the base,
    /// which means `.context-engine/` will be created lazily if any
    /// workspace is actually persisted.
    pub fn new_in_memory() -> Self {
        let base =
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self::new(WorkspaceManager::new(base))
    }

    /// Derive a [`CaptureConfig`] for the given command, if the command
    /// targets a specific workspace.
    ///
    /// The capture config tells `execute_traced` where to write the trace
    /// log file (the workspace-local `logs/` directory). Commands that do
    /// not reference a workspace (e.g. `ListWorkspaces`) return `None`.
    pub fn capture_config_for(
        &self,
        cmd: &Command,
    ) -> Option<CaptureConfig> {
        let ws_name = workspace_name_of(cmd)?;

        // Resolve the workspace log directory via the manager.
        // We need to lock the manager briefly to ask for the log dir.
        let mgr = self.manager.lock().ok()?;
        let log_dir = mgr.log_dir(ws_name).ok()?;

        Some(CaptureConfig {
            enabled: true,
            log_dir,
            level: "TRACE".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_in_memory_does_not_panic() {
        let _state = AppState::new_in_memory();
    }

    #[test]
    fn state_is_clone() {
        let state = AppState::new_in_memory();
        let _clone = state.clone();
    }

    #[test]
    fn capture_config_for_list_workspaces_is_none() {
        let state = AppState::new_in_memory();
        let cmd = Command::ListWorkspaces;
        assert!(state.capture_config_for(&cmd).is_none());
    }
}
