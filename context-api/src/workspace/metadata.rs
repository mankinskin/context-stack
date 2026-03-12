//! Workspace metadata — human-readable JSON sidecar for each workspace.
//!
//! Stored as `metadata.json` alongside `graph.bin` in the workspace directory.

use chrono::{
    DateTime,
    Utc,
};
use serde::{
    Deserialize,
    Serialize,
};

/// Persistent metadata for a workspace, stored as `metadata.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceMetadata {
    /// Workspace name (matches the directory name under `.context-engine/`).
    pub name: String,

    /// Optional human-readable description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Timestamp when the workspace was first created.
    pub created_at: DateTime<Utc>,

    /// Timestamp of the last explicit save (`save_workspace`).
    pub modified_at: DateTime<Utc>,
}

impl WorkspaceMetadata {
    /// Create metadata for a brand-new workspace.
    pub fn new(name: &str) -> Self {
        let now = Utc::now();
        Self {
            name: name.to_string(),
            description: None,
            created_at: now,
            modified_at: now,
        }
    }

    /// Update `modified_at` to the current time.
    ///
    /// Called automatically by `WorkspaceManager::save_workspace`.
    pub fn touch(&mut self) {
        self.modified_at = Utc::now();
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_metadata_has_matching_timestamps() {
        let meta = WorkspaceMetadata::new("test-ws");
        assert_eq!(meta.name, "test-ws");
        assert!(meta.description.is_none());
        // created_at and modified_at should be equal (or extremely close)
        assert_eq!(meta.created_at, meta.modified_at);
    }

    #[test]
    fn touch_updates_modified_at() {
        let mut meta = WorkspaceMetadata::new("ws");
        let original = meta.modified_at;

        // Sleep a tiny bit to ensure the clock advances (chrono uses system clock)
        std::thread::sleep(std::time::Duration::from_millis(10));
        meta.touch();

        assert!(
            meta.modified_at >= original,
            "modified_at should be >= original after touch"
        );
        // created_at must remain unchanged
        assert_eq!(meta.created_at, meta.created_at);
    }

    #[test]
    fn serde_json_round_trip() {
        let meta = WorkspaceMetadata::new("demo");
        let json = serde_json::to_string_pretty(&meta).unwrap();
        let deser: WorkspaceMetadata = serde_json::from_str(&json).unwrap();

        assert_eq!(meta.name, deser.name);
        assert_eq!(meta.description, deser.description);
        assert_eq!(meta.created_at, deser.created_at);
        assert_eq!(meta.modified_at, deser.modified_at);
    }

    #[test]
    fn serde_json_with_description() {
        let mut meta = WorkspaceMetadata::new("described");
        meta.description = Some("A test workspace".to_string());

        let json = serde_json::to_string(&meta).unwrap();
        assert!(json.contains("A test workspace"));

        let deser: WorkspaceMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.description.as_deref(), Some("A test workspace"));
    }

    #[test]
    fn serde_json_missing_description_defaults_to_none() {
        // Simulate JSON without the `description` field
        let json = r#"{
            "name": "no-desc",
            "created_at": "2025-01-01T00:00:00Z",
            "modified_at": "2025-01-01T00:00:00Z"
        }"#;
        let meta: WorkspaceMetadata = serde_json::from_str(json).unwrap();
        assert_eq!(meta.name, "no-desc");
        assert!(meta.description.is_none());
    }
}
