//! Export/import workspace commands for data portability.
//!
//! Provides two functions:
//! - `export_workspace` — serialize an open workspace (graph + metadata) to
//!   JSON or bincode, writing to a file or returning bytes inline.
//! - `import_workspace` — read an export file, detect format, and create a
//!   new workspace from the imported data.
//!
//! ## JSON format
//!
//! The JSON export embeds the graph as **base64-encoded bincode bytes**
//! because `Hypergraph`'s serde implementation uses non-string map keys
//! (e.g. `Token` indices) which `serde_json` cannot represent as JSON
//! object keys. The metadata and version fields remain human-readable.
//!
//! ## Bincode format
//!
//! The bincode export stores everything (metadata serialized as JSON bytes
//! + raw graph bincode bytes) inside a simple length-prefixed envelope.
//! This avoids issues with bincode trying to deserialize serde-tagged enums
//! inside `WorkspaceMetadata`.

mod base64;
#[cfg(test)]
mod tests;

use std::{
    fs,
    path::Path,
};

use serde::{
    Deserialize,
    Serialize,
};

use context_trace::graph::{
    Hypergraph,
    kind::BaseGraphKind,
};

use crate::{
    error::WorkspaceError,
    ts_export,
    types::WorkspaceInfo,
    workspace::{
        manager::WorkspaceManager,
        metadata::WorkspaceMetadata,
        persistence,
    },
};

use self::base64::{
    base64_decode,
    base64_encode,
};

ts_export! {
    /// Format for workspace export/import.
    #[derive(
        Debug,
        Clone,
        Copy,
        PartialEq,
        Eq,
        Serialize,
        Deserialize,
        schemars::JsonSchema,
    )]
    #[serde(rename_all = "snake_case")]
    pub enum ExportFormat {
        /// Human-readable JSON (larger, useful for debugging).
        Json,
        /// Compact binary via bincode (smaller, useful for backup/transfer).
        Bincode,
    }
}

/// Current crate version embedded in exports for forward-compatibility checks.
const CONTEXT_API_VERSION: &str = env!("CARGO_PKG_VERSION");

/// A magic header written at the start of bincode exports so we can
/// distinguish them from JSON during auto-detection.
const BINCODE_MAGIC: &[u8; 4] = b"CXEI";

/// JSON-format export envelope.
///
/// The graph is stored as base64-encoded bincode bytes because
/// `Hypergraph`'s `Serialize` impl uses non-string map keys that
/// `serde_json` cannot represent as JSON object keys.
#[derive(Debug, Serialize, Deserialize)]
struct JsonExport {
    context_api_version: String,
    metadata: WorkspaceMetadata,
    /// Base64-encoded bincode-serialized `Hypergraph<BaseGraphKind>`.
    graph_b64: String,
}

/// Build a bincode export as: MAGIC(4) + metadata_json_len(u64 LE) +
/// metadata_json_bytes + graph_bincode_bytes.
///
/// We avoid nesting serde-tagged types inside bincode (which can mis-read
/// enum tags) by pre-serializing metadata to JSON bytes and graph to
/// bincode bytes, then concatenating with simple length-prefix framing.
fn build_bincode_export(
    metadata: &WorkspaceMetadata,
    graph: &Hypergraph<BaseGraphKind>,
) -> Result<Vec<u8>, WorkspaceError> {
    let meta_json = serde_json::to_vec(metadata).map_err(|e| {
        WorkspaceError::SerializationError(format!(
            "metadata json serialize: {e}"
        ))
    })?;
    let graph_bytes = bincode::serialize(graph).map_err(|e| {
        WorkspaceError::SerializationError(format!(
            "bincode graph serialize: {e}"
        ))
    })?;

    let version_bytes = CONTEXT_API_VERSION.as_bytes();
    let total =
        4 + 4 + version_bytes.len() + 8 + meta_json.len() + graph_bytes.len();
    let mut buf = Vec::with_capacity(total);
    buf.extend_from_slice(BINCODE_MAGIC);
    buf.extend_from_slice(&(version_bytes.len() as u32).to_le_bytes());
    buf.extend_from_slice(version_bytes);
    buf.extend_from_slice(&(meta_json.len() as u64).to_le_bytes());
    buf.extend_from_slice(&meta_json);
    buf.extend_from_slice(&graph_bytes);
    Ok(buf)
}

/// Parse a bincode export built by `build_bincode_export`.
///
/// Returns `(metadata, graph)`.
fn parse_bincode_export(
    data: &[u8]
) -> Result<(WorkspaceMetadata, Hypergraph<BaseGraphKind>), WorkspaceError> {
    let make_err = |msg: &str| {
        WorkspaceError::SerializationError(format!(
            "bincode export parse: {msg}"
        ))
    };

    if data.len() < 4 || &data[0..4] != BINCODE_MAGIC {
        return Err(make_err("missing magic header"));
    }
    let mut pos: usize = 4;

    if data.len() < pos + 4 {
        return Err(make_err("truncated version length"));
    }
    let ver_len =
        u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap()) as usize;
    pos += 4;
    if data.len() < pos + ver_len {
        return Err(make_err("truncated version string"));
    }
    let _version = std::str::from_utf8(&data[pos..pos + ver_len])
        .map_err(|_| make_err("invalid version utf8"))?;
    pos += ver_len;

    if data.len() < pos + 8 {
        return Err(make_err("truncated metadata length"));
    }
    let meta_len =
        u64::from_le_bytes(data[pos..pos + 8].try_into().unwrap()) as usize;
    pos += 8;
    if data.len() < pos + meta_len {
        return Err(make_err("truncated metadata"));
    }
    let metadata: WorkspaceMetadata =
        serde_json::from_slice(&data[pos..pos + meta_len]).map_err(|e| {
            WorkspaceError::SerializationError(format!(
                "metadata json deserialize: {e}"
            ))
        })?;
    pos += meta_len;

    let graph: Hypergraph<BaseGraphKind> = bincode::deserialize(&data[pos..])
        .map_err(|e| {
        WorkspaceError::SerializationError(format!(
            "bincode graph deserialize: {e}"
        ))
    })?;

    Ok((metadata, graph))
}

/// Export a workspace to a file or return the data inline.
///
/// If `path` is `Some`, writes the export to the given file path and returns
/// `Ok(None)`. If `path` is `None`, returns the raw export data as
/// `Ok(Some(bytes))`.
pub fn export_workspace(
    mgr: &WorkspaceManager,
    workspace: &str,
    format: ExportFormat,
    path: Option<&str>,
) -> Result<Option<Vec<u8>>, WorkspaceError> {
    let ws = mgr.get_workspace(workspace)?;

    let output_bytes = match format {
        ExportFormat::Json => {
            let graph_bytes = bincode::serialize(ws.graph()).map_err(|e| {
                WorkspaceError::SerializationError(format!(
                    "bincode graph serialize: {e}"
                ))
            })?;
            let graph_b64 = base64_encode(&graph_bytes);
            let envelope = JsonExport {
                context_api_version: CONTEXT_API_VERSION.to_string(),
                metadata: ws.metadata.clone(),
                graph_b64,
            };
            serde_json::to_string_pretty(&envelope)
                .map_err(|e| {
                    WorkspaceError::SerializationError(format!(
                        "json serialize: {e}"
                    ))
                })?
                .into_bytes()
        },
        ExportFormat::Bincode =>
            build_bincode_export(&ws.metadata, ws.graph())?,
    };

    match path {
        Some(p) => {
            fs::write(p, &output_bytes).map_err(WorkspaceError::IoError)?;
            Ok(None)
        },
        None => Ok(Some(output_bytes)),
    }
}

/// Import a workspace from a file.
///
/// Reads the file, detects format (tries JSON first, then bincode), creates
/// the workspace directory, and saves the graph and metadata. The workspace
/// is opened automatically and its `WorkspaceInfo` is returned.
pub fn import_workspace(
    mgr: &mut WorkspaceManager,
    name: &str,
    file_path: &str,
    overwrite: bool,
) -> Result<WorkspaceInfo, WorkspaceError> {
    let path = Path::new(file_path);
    let bytes = fs::read(path).map_err(WorkspaceError::IoError)?;

    let (metadata, graph) =
        if let Ok(json_export) = serde_json::from_slice::<JsonExport>(&bytes) {
            let graph_bytes =
                base64_decode(&json_export.graph_b64).map_err(|e| {
                    WorkspaceError::SerializationError(format!(
                        "base64 decode: {e}"
                    ))
                })?;
            let graph: Hypergraph<BaseGraphKind> =
                bincode::deserialize(&graph_bytes).map_err(|e| {
                    WorkspaceError::SerializationError(format!(
                        "json graph deserialize: {e}"
                    ))
                })?;
            (json_export.metadata, graph)
        } else if bytes.len() >= 4 && &bytes[0..4] == BINCODE_MAGIC {
            parse_bincode_export(&bytes)?
        } else {
            return Err(WorkspaceError::SerializationError(
                "failed to parse export file: not valid JSON or bincode export"
                    .to_string(),
            ));
        };

    let base = mgr.base_dir().to_path_buf();
    if persistence::workspace_exists(&base, name) {
        if !overwrite {
            return Err(WorkspaceError::AlreadyExists {
                name: name.to_string(),
            });
        }
        if mgr.is_open(name) {
            mgr.close_workspace(name)?;
        }
        mgr.delete_workspace(name)?;
    }

    let dir = persistence::workspace_dir(&base, name);
    fs::create_dir_all(&dir).map_err(WorkspaceError::IoError)?;

    let mut metadata = metadata;
    metadata.name = name.to_string();

    persistence::save_graph(&dir, &graph)?;
    persistence::save_metadata(&dir, &metadata)?;

    mgr.open_workspace(name)
}
