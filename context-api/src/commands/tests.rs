use crate::workspace::manager::WorkspaceManager;

mod api;
mod command_serde;
mod execute_basic;
mod execute_queries;
mod result_serde;

fn tmp_manager() -> (tempfile::TempDir, WorkspaceManager) {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let mgr = WorkspaceManager::new(tmp.path().to_path_buf());
    (tmp, mgr)
}