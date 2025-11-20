//! Path utilities for finding target directory

use std::{
    env,
    fs,
    path::PathBuf,
};

/// Get the target directory used by Cargo
///
/// This respects the workspace structure by checking:
/// 1. CARGO_TARGET_DIR environment variable (if set by user/CI)
/// 2. CARGO_MANIFEST_DIR at runtime to find workspace root
/// 3. Falls back to "target" relative to current directory
pub(super) fn get_target_dir() -> PathBuf {
    // First check if CARGO_TARGET_DIR is set (user override or CI)
    if let Ok(target_dir) = env::var("CARGO_TARGET_DIR") {
        return PathBuf::from(target_dir);
    }

    // During test execution, CARGO_MANIFEST_DIR points to the crate being tested
    // For workspace, we want to use the workspace root's target directory
    if let Ok(manifest_dir) = env::var("CARGO_MANIFEST_DIR") {
        let manifest_path = PathBuf::from(&manifest_dir);
        let mut current = manifest_path.clone();

        // Walk up to find workspace root (has Cargo.toml with [workspace])
        while let Some(parent) = current.parent() {
            let workspace_toml = parent.join("Cargo.toml");
            if workspace_toml.exists() {
                // Check if this is a workspace root by looking for [workspace] section
                if let Ok(contents) = fs::read_to_string(&workspace_toml)
                    && contents.contains("[workspace]")
                {
                    return parent.join("target");
                }
            }
            current = parent.to_path_buf();
        }

        // If no workspace found, use the manifest dir's target
        return manifest_path.join("target");
    }

    // Fallback to relative "target" directory (current directory)
    PathBuf::from("target")
}

/// Get the workspace root directory
///
/// Returns the workspace root if in a workspace, otherwise returns current directory
pub(super) fn get_workspace_root() -> PathBuf {
    // During test execution, CARGO_MANIFEST_DIR points to the crate being tested
    if let Ok(manifest_dir) = env::var("CARGO_MANIFEST_DIR") {
        let manifest_path = PathBuf::from(&manifest_dir);
        let mut current = manifest_path.clone();

        // Walk up to find workspace root (has Cargo.toml with [workspace])
        while let Some(parent) = current.parent() {
            let workspace_toml = parent.join("Cargo.toml");
            if workspace_toml.exists() {
                // Check if this is a workspace root by looking for [workspace] section
                if let Ok(contents) = fs::read_to_string(&workspace_toml)
                    && contents.contains("[workspace]")
                {
                    return parent.to_path_buf();
                }
            }
            current = parent.to_path_buf();
        }

        // If no workspace found, return the manifest dir
        return manifest_path;
    }

    // Fallback to current directory
    env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}
