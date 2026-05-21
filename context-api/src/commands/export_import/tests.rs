use crate::workspace::manager::WorkspaceManager;

mod base64;
mod import_behavior;
mod roundtrip;

fn tmp_manager() -> (tempfile::TempDir, WorkspaceManager) {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let mgr = WorkspaceManager::new(tmp.path().to_path_buf());
    (tmp, mgr)
}

fn add_atoms(
    mgr: &mut WorkspaceManager,
    ws: &str,
    chars: &[char],
) {
    for &ch in chars {
        mgr.add_atom(ws, ch).unwrap();
    }
}
