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
    types::WorkspaceInfo,
    workspace::{
        manager::WorkspaceManager,
        metadata::WorkspaceMetadata,
        persistence,
    },
};

// ---------------------------------------------------------------------------
// ExportFormat
// ---------------------------------------------------------------------------

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
#[cfg_attr(feature = "ts-gen", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-gen",
    ts(export, export_to = "../../../packages/context-types/src/generated/")
)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    /// Human-readable JSON (larger, useful for debugging).
    Json,
    /// Compact binary via bincode (smaller, useful for backup/transfer).
    Bincode,
}

// ---------------------------------------------------------------------------
// Internal envelope types
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Base64 helpers (simple, no external dependency)
// ---------------------------------------------------------------------------

/// Standard base64 alphabet.
const B64_CHARS: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(input: &[u8]) -> String {
    let mut out = String::with_capacity((input.len() + 2) / 3 * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(B64_CHARS[((triple >> 18) & 0x3F) as usize] as char);
        out.push(B64_CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(B64_CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(B64_CHARS[(triple & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

fn base64_decode(input: &str) -> Result<Vec<u8>, String> {
    fn val(c: u8) -> Result<u32, String> {
        match c {
            b'A'..=b'Z' => Ok((c - b'A') as u32),
            b'a'..=b'z' => Ok((c - b'a' + 26) as u32),
            b'0'..=b'9' => Ok((c - b'0' + 52) as u32),
            b'+' => Ok(62),
            b'/' => Ok(63),
            b'=' => Ok(0),
            _ => Err(format!("invalid base64 character: {c}")),
        }
    }

    let bytes: Vec<u8> =
        input.bytes().filter(|b| !b.is_ascii_whitespace()).collect();

    if bytes.len() % 4 != 0 {
        return Err("base64 input length is not a multiple of 4".into());
    }

    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for chunk in bytes.chunks(4) {
        let a = val(chunk[0])?;
        let b = val(chunk[1])?;
        let c = val(chunk[2])?;
        let d = val(chunk[3])?;
        let triple = (a << 18) | (b << 12) | (c << 6) | d;
        out.push(((triple >> 16) & 0xFF) as u8);
        if chunk[2] != b'=' {
            out.push(((triple >> 8) & 0xFF) as u8);
        }
        if chunk[3] != b'=' {
            out.push((triple & 0xFF) as u8);
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Bincode envelope helpers
// ---------------------------------------------------------------------------

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
    // Layout: MAGIC(4) + version_len(u32 LE) + version_bytes +
    //         meta_len(u64 LE) + meta_json + graph_bytes (rest)
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

    // version
    if data.len() < pos + 4 {
        return Err(make_err("truncated version length"));
    }
    let ver_len =
        u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap()) as usize;
    pos += 4;
    if data.len() < pos + ver_len {
        return Err(make_err("truncated version string"));
    }
    // We read but don't enforce the version for now — future: compatibility check.
    let _version = std::str::from_utf8(&data[pos..pos + ver_len])
        .map_err(|_| make_err("invalid version utf8"))?;
    pos += ver_len;

    // metadata
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

    // graph (remainder)
    let graph: Hypergraph<BaseGraphKind> = bincode::deserialize(&data[pos..])
        .map_err(|e| {
        WorkspaceError::SerializationError(format!(
            "bincode graph deserialize: {e}"
        ))
    })?;

    Ok((metadata, graph))
}

// ---------------------------------------------------------------------------
// export_workspace
// ---------------------------------------------------------------------------

/// Export a workspace to a file or return the data inline.
///
/// If `path` is `Some`, writes the export to the given file path and returns
/// `Ok(None)`. If `path` is `None`, returns the raw export data as
/// `Ok(Some(bytes))`.
///
/// # Errors
///
/// - `WorkspaceError::NotOpen` if the workspace is not currently open.
/// - `WorkspaceError::SerializationError` on serialization failure.
/// - `WorkspaceError::IoError` on file-write failure.
pub fn export_workspace(
    mgr: &WorkspaceManager,
    workspace: &str,
    format: ExportFormat,
    path: Option<&str>,
) -> Result<Option<Vec<u8>>, WorkspaceError> {
    let ws = mgr.get_workspace(workspace)?;

    let output_bytes = match format {
        ExportFormat::Json => {
            // Serialize graph to bincode, then base64-encode for JSON embedding.
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

// ---------------------------------------------------------------------------
// import_workspace
// ---------------------------------------------------------------------------

/// Import a workspace from a file.
///
/// Reads the file, detects format (tries JSON first, then bincode), creates
/// the workspace directory, and saves the graph and metadata. The workspace
/// is opened automatically and its `WorkspaceInfo` is returned.
///
/// # Errors
///
/// - `WorkspaceError::IoError` if the file cannot be read or the directory
///   cannot be created.
/// - `WorkspaceError::SerializationError` if the file cannot be parsed as
///   either JSON or bincode.
/// - `WorkspaceError::AlreadyExists` if `overwrite` is `false` and a
///   workspace with the given name already exists.
pub fn import_workspace(
    mgr: &mut WorkspaceManager,
    name: &str,
    file_path: &str,
    overwrite: bool,
) -> Result<WorkspaceInfo, WorkspaceError> {
    let path = Path::new(file_path);
    let bytes = fs::read(path).map_err(WorkspaceError::IoError)?;

    // Detect format: try JSON first, fall back to bincode.
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

    // Check if workspace already exists.
    let base = mgr.base_dir().to_path_buf();
    if persistence::workspace_exists(&base, name) {
        if !overwrite {
            return Err(WorkspaceError::AlreadyExists {
                name: name.to_string(),
            });
        }
        // Close if open, then delete so we can recreate.
        if mgr.is_open(name) {
            mgr.close_workspace(name)?;
        }
        mgr.delete_workspace(name)?;
    }

    // Create workspace directory and save imported data.
    let dir = persistence::workspace_dir(&base, name);
    fs::create_dir_all(&dir).map_err(WorkspaceError::IoError)?;

    // Update metadata with the target name (may differ from original).
    let mut metadata = metadata;
    metadata.name = name.to_string();

    persistence::save_graph(&dir, &graph)?;
    persistence::save_metadata(&dir, &metadata)?;

    // Open the imported workspace so we can return info.
    mgr.open_workspace(name)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::WorkspaceApi;

    /// Helper: create a `WorkspaceManager` backed by a temporary directory.
    fn tmp_manager() -> (tempfile::TempDir, WorkspaceManager) {
        let tmp = tempfile::tempdir().expect("failed to create temp dir");
        let mgr = WorkspaceManager::new(tmp.path().to_path_buf());
        (tmp, mgr)
    }

    /// Helper: add atoms to an open workspace via the WorkspaceApi trait.
    fn add_atoms(
        mgr: &mut WorkspaceManager,
        ws: &str,
        chars: &[char],
    ) {
        for &ch in chars {
            mgr.add_atom(ws, ch).unwrap();
        }
    }

    // -- Base64 unit tests --------------------------------------------------

    #[test]
    fn base64_round_trip_empty() {
        let encoded = base64_encode(b"");
        assert_eq!(encoded, "");
        let decoded = base64_decode(&encoded).unwrap();
        assert!(decoded.is_empty());
    }

    #[test]
    fn base64_round_trip_small() {
        for input in &[b"a".as_slice(), b"ab", b"abc", b"abcd", b"hello world"]
        {
            let encoded = base64_encode(input);
            let decoded = base64_decode(&encoded).unwrap();
            assert_eq!(&decoded, input, "round-trip failed for {:?}", input);
        }
    }

    #[test]
    fn base64_round_trip_binary() {
        let input: Vec<u8> = (0..=255).collect();
        let encoded = base64_encode(&input);
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(decoded, input);
    }

    // -- ExportFormat serde -------------------------------------------------

    #[test]
    fn export_format_serde_json_variant() {
        let json = serde_json::to_string(&ExportFormat::Json).unwrap();
        assert_eq!(json, r#""json""#);
        let deser: ExportFormat = serde_json::from_str(&json).unwrap();
        assert_eq!(deser, ExportFormat::Json);
    }

    #[test]
    fn export_format_serde_bincode_variant() {
        let json = serde_json::to_string(&ExportFormat::Bincode).unwrap();
        assert_eq!(json, r#""bincode""#);
        let deser: ExportFormat = serde_json::from_str(&json).unwrap();
        assert_eq!(deser, ExportFormat::Bincode);
    }

    // -- JSON round-trip ----------------------------------------------------

    #[test]
    fn export_import_json_round_trip() {
        let (tmp, mut mgr) = tmp_manager();

        // Create workspace and add some data.
        mgr.create_workspace("source").unwrap();
        add_atoms(&mut mgr, "source", &['a', 'b', 'c']);

        // Export to JSON file.
        let export_path = tmp.path().join("export.json");
        let result = export_workspace(
            &mgr,
            "source",
            ExportFormat::Json,
            Some(export_path.to_str().unwrap()),
        );
        assert!(result.is_ok());
        assert!(result.unwrap().is_none()); // None because written to file.
        assert!(export_path.exists());

        // Verify JSON is human-readable.
        let content = fs::read_to_string(&export_path).unwrap();
        assert!(content.contains("context_api_version"));
        assert!(content.contains("source"));
        assert!(content.contains("graph_b64"));

        // Import into a new workspace.
        let info = import_workspace(
            &mut mgr,
            "imported",
            export_path.to_str().unwrap(),
            false,
        )
        .unwrap();

        assert_eq!(info.name, "imported");
        assert_eq!(info.atom_count, 3);
        assert_eq!(info.vertex_count, 3);
    }

    // -- Bincode round-trip -------------------------------------------------

    #[test]
    fn export_import_bincode_round_trip() {
        let (tmp, mut mgr) = tmp_manager();

        mgr.create_workspace("binsrc").unwrap();
        add_atoms(&mut mgr, "binsrc", &['x', 'y']);

        // Export to bincode file.
        let export_path = tmp.path().join("export.bin");
        export_workspace(
            &mgr,
            "binsrc",
            ExportFormat::Bincode,
            Some(export_path.to_str().unwrap()),
        )
        .unwrap();
        assert!(export_path.exists());

        // Bincode file should start with magic header.
        let raw = fs::read(&export_path).unwrap();
        assert_eq!(&raw[0..4], BINCODE_MAGIC);

        // Import into a new workspace.
        let info = import_workspace(
            &mut mgr,
            "bin-imported",
            export_path.to_str().unwrap(),
            false,
        )
        .unwrap();

        assert_eq!(info.name, "bin-imported");
        assert_eq!(info.atom_count, 2);
        assert_eq!(info.vertex_count, 2);
    }

    // -- Inline export (no file) --------------------------------------------

    #[test]
    fn export_inline_json_returns_bytes() {
        let (_tmp, mut mgr) = tmp_manager();

        mgr.create_workspace("inline").unwrap();
        add_atoms(&mut mgr, "inline", &['a']);

        let result =
            export_workspace(&mgr, "inline", ExportFormat::Json, None).unwrap();

        assert!(result.is_some());
        let bytes = result.unwrap();
        assert!(!bytes.is_empty());

        // Should be parseable as JSON.
        let parsed: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(parsed.get("context_api_version").is_some());
        assert!(parsed.get("metadata").is_some());
        assert!(parsed.get("graph_b64").is_some());
    }

    #[test]
    fn export_inline_bincode_returns_bytes() {
        let (_tmp, mut mgr) = tmp_manager();

        mgr.create_workspace("inline-bin").unwrap();
        add_atoms(&mut mgr, "inline-bin", &['z']);

        let result =
            export_workspace(&mgr, "inline-bin", ExportFormat::Bincode, None)
                .unwrap();

        assert!(result.is_some());
        let bytes = result.unwrap();
        assert!(!bytes.is_empty());
        assert_eq!(&bytes[0..4], BINCODE_MAGIC);
    }

    // -- Overwrite behavior -------------------------------------------------

    #[test]
    fn import_overwrite_false_fails_if_exists() {
        let (tmp, mut mgr) = tmp_manager();

        mgr.create_workspace("original").unwrap();
        add_atoms(&mut mgr, "original", &['a']);

        // Export original workspace.
        let export_path = tmp.path().join("overwrite_test.json");
        export_workspace(
            &mgr,
            "original",
            ExportFormat::Json,
            Some(export_path.to_str().unwrap()),
        )
        .unwrap();

        // Attempt import with same name without overwrite.
        // Close original first so import doesn't fail for lock reasons.
        mgr.close_workspace("original").unwrap();

        let result = import_workspace(
            &mut mgr,
            "original",
            export_path.to_str().unwrap(),
            false,
        );
        assert!(result.is_err());
        match result.unwrap_err() {
            WorkspaceError::AlreadyExists { name } => {
                assert_eq!(name, "original");
            },
            other => panic!("expected AlreadyExists, got: {other}"),
        }
    }

    #[test]
    fn import_overwrite_true_replaces_existing() {
        let (tmp, mut mgr) = tmp_manager();

        // Create a workspace with 1 atom.
        mgr.create_workspace("overme").unwrap();
        add_atoms(&mut mgr, "overme", &['a']);
        mgr.save_workspace("overme").unwrap();

        // Export it to get a file we can import from.
        let export_path = tmp.path().join("for_overwrite.json");
        export_workspace(
            &mgr,
            "overme",
            ExportFormat::Json,
            Some(export_path.to_str().unwrap()),
        )
        .unwrap();

        // Close so we can re-import.
        mgr.close_workspace("overme").unwrap();

        // Now create a different workspace to export, with 3 atoms.
        mgr.create_workspace("bigger").unwrap();
        add_atoms(&mut mgr, "bigger", &['x', 'y', 'z']);

        let export_path2 = tmp.path().join("bigger.json");
        export_workspace(
            &mgr,
            "bigger",
            ExportFormat::Json,
            Some(export_path2.to_str().unwrap()),
        )
        .unwrap();

        // Import "bigger" export into the "overme" workspace name with overwrite.
        let info = import_workspace(
            &mut mgr,
            "overme",
            export_path2.to_str().unwrap(),
            true,
        )
        .unwrap();

        assert_eq!(info.name, "overme");
        assert_eq!(info.atom_count, 3);
        assert_eq!(info.vertex_count, 3);
    }

    // -- Error cases --------------------------------------------------------

    #[test]
    fn export_not_open_workspace_fails() {
        let (_tmp, mgr) = tmp_manager();

        let result = export_workspace(&mgr, "ghost", ExportFormat::Json, None);

        assert!(result.is_err());
        match result.unwrap_err() {
            WorkspaceError::NotOpen { name } => {
                assert_eq!(name, "ghost");
            },
            other => panic!("expected NotOpen, got: {other}"),
        }
    }

    #[test]
    fn import_nonexistent_file_fails() {
        let (_tmp, mut mgr) = tmp_manager();

        let result = import_workspace(
            &mut mgr,
            "nope",
            "/definitely/not/a/real/path.json",
            false,
        );

        assert!(result.is_err());
        match result.unwrap_err() {
            WorkspaceError::IoError(_) => { /* expected */ },
            other => panic!("expected IoError, got: {other}"),
        }
    }

    #[test]
    fn import_garbage_file_fails() {
        let (tmp, mut mgr) = tmp_manager();

        let garbage_path = tmp.path().join("garbage.bin");
        fs::write(&garbage_path, b"this is not a valid export file").unwrap();

        let result = import_workspace(
            &mut mgr,
            "bad",
            garbage_path.to_str().unwrap(),
            false,
        );

        assert!(result.is_err());
        match result.unwrap_err() {
            WorkspaceError::SerializationError(msg) => {
                assert!(
                    msg.contains("failed to parse"),
                    "error should mention parse failure, got: {msg}"
                );
            },
            other => panic!("expected SerializationError, got: {other}"),
        }
    }

    // -- Cross-format import ------------------------------------------------

    #[test]
    fn json_export_auto_detected_on_import() {
        let (tmp, mut mgr) = tmp_manager();

        mgr.create_workspace("jsrc").unwrap();
        add_atoms(&mut mgr, "jsrc", &['a', 'b']);

        // Export as JSON but give it a .bin extension — format detection
        // should still recognize JSON.
        let export_path = tmp.path().join("actually_json.bin");
        export_workspace(
            &mgr,
            "jsrc",
            ExportFormat::Json,
            Some(export_path.to_str().unwrap()),
        )
        .unwrap();

        let info = import_workspace(
            &mut mgr,
            "auto-json",
            export_path.to_str().unwrap(),
            false,
        )
        .unwrap();

        assert_eq!(info.name, "auto-json");
        assert_eq!(info.atom_count, 2);
    }

    #[test]
    fn bincode_export_auto_detected_on_import() {
        let (tmp, mut mgr) = tmp_manager();

        mgr.create_workspace("bsrc").unwrap();
        add_atoms(&mut mgr, "bsrc", &['x', 'y', 'z']);

        // Export as bincode but give it a .json extension — format detection
        // should still recognize bincode via magic header.
        let export_path = tmp.path().join("actually_bincode.json");
        export_workspace(
            &mgr,
            "bsrc",
            ExportFormat::Bincode,
            Some(export_path.to_str().unwrap()),
        )
        .unwrap();

        let info = import_workspace(
            &mut mgr,
            "auto-bin",
            export_path.to_str().unwrap(),
            false,
        )
        .unwrap();

        assert_eq!(info.name, "auto-bin");
        assert_eq!(info.atom_count, 3);
    }

    // -- Empty workspace round-trip -----------------------------------------

    #[test]
    fn export_import_empty_workspace_json() {
        let (tmp, mut mgr) = tmp_manager();

        mgr.create_workspace("empty").unwrap();

        let export_path = tmp.path().join("empty.json");
        export_workspace(
            &mgr,
            "empty",
            ExportFormat::Json,
            Some(export_path.to_str().unwrap()),
        )
        .unwrap();

        let info = import_workspace(
            &mut mgr,
            "empty-copy",
            export_path.to_str().unwrap(),
            false,
        )
        .unwrap();

        assert_eq!(info.name, "empty-copy");
        assert_eq!(info.vertex_count, 0);
        assert_eq!(info.atom_count, 0);
    }

    #[test]
    fn export_import_empty_workspace_bincode() {
        let (tmp, mut mgr) = tmp_manager();

        mgr.create_workspace("empty-bin").unwrap();

        let export_path = tmp.path().join("empty.bin");
        export_workspace(
            &mgr,
            "empty-bin",
            ExportFormat::Bincode,
            Some(export_path.to_str().unwrap()),
        )
        .unwrap();

        let info = import_workspace(
            &mut mgr,
            "empty-bin-copy",
            export_path.to_str().unwrap(),
            false,
        )
        .unwrap();

        assert_eq!(info.name, "empty-bin-copy");
        assert_eq!(info.vertex_count, 0);
        assert_eq!(info.atom_count, 0);
    }

    // -- Metadata preservation ----------------------------------------------

    #[test]
    fn import_renames_workspace_but_preserves_created_at() {
        let (tmp, mut mgr) = tmp_manager();

        mgr.create_workspace("stamped").unwrap();

        // Export.
        let export_path = tmp.path().join("stamped.json");
        export_workspace(
            &mgr,
            "stamped",
            ExportFormat::Json,
            Some(export_path.to_str().unwrap()),
        )
        .unwrap();

        // Read the export to get original timestamps.
        let export_content = fs::read_to_string(&export_path).unwrap();
        let export_json: serde_json::Value =
            serde_json::from_str(&export_content).unwrap();
        let original_created = export_json["metadata"]["created_at"]
            .as_str()
            .unwrap()
            .to_string();

        // Import to a different name.
        let info = import_workspace(
            &mut mgr,
            "stamped-copy",
            export_path.to_str().unwrap(),
            false,
        )
        .unwrap();

        // The imported workspace should use the renamed name.
        assert_eq!(info.name, "stamped-copy");

        // The created_at should be preserved. Both use chrono RFC 3339 but
        // the suffix may differ (+00:00 vs Z), so parse and compare.
        let original_dt =
            chrono::DateTime::parse_from_rfc3339(&original_created).unwrap();
        let imported_dt =
            chrono::DateTime::parse_from_rfc3339(&info.created_at).unwrap();
        assert_eq!(original_dt, imported_dt);
    }

    // -- Version field presence ---------------------------------------------

    #[test]
    fn json_export_contains_version() {
        let (_tmp, mut mgr) = tmp_manager();

        mgr.create_workspace("ver").unwrap();

        let bytes = export_workspace(&mgr, "ver", ExportFormat::Json, None)
            .unwrap()
            .unwrap();

        let parsed: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let version = parsed["context_api_version"].as_str().unwrap();
        assert_eq!(version, CONTEXT_API_VERSION);
    }

    // -- Import with open source workspace ----------------------------------

    #[test]
    fn import_overwrite_closes_open_workspace() {
        let (tmp, mut mgr) = tmp_manager();

        // Create and export a workspace.
        mgr.create_workspace("src-ow").unwrap();
        add_atoms(&mut mgr, "src-ow", &['q']);

        let export_path = tmp.path().join("ow.json");
        export_workspace(
            &mgr,
            "src-ow",
            ExportFormat::Json,
            Some(export_path.to_str().unwrap()),
        )
        .unwrap();

        // Create target workspace that's still open.
        mgr.close_workspace("src-ow").unwrap();
        mgr.create_workspace("target-ow").unwrap();
        assert!(mgr.is_open("target-ow"));

        // Overwrite the open target workspace.
        let info = import_workspace(
            &mut mgr,
            "target-ow",
            export_path.to_str().unwrap(),
            true,
        )
        .unwrap();

        assert_eq!(info.name, "target-ow");
        assert_eq!(info.atom_count, 1);
    }

    // -- Cross-format round-trip: export JSON, verify bincode import fails ---

    #[test]
    fn json_file_not_mistaken_for_bincode() {
        let (tmp, mut mgr) = tmp_manager();

        mgr.create_workspace("jonly").unwrap();
        add_atoms(&mut mgr, "jonly", &['a']);

        let export_path = tmp.path().join("json_only.json");
        export_workspace(
            &mgr,
            "jonly",
            ExportFormat::Json,
            Some(export_path.to_str().unwrap()),
        )
        .unwrap();

        // Verify the file does NOT start with bincode magic.
        let raw = fs::read(&export_path).unwrap();
        assert_ne!(&raw[0..4], BINCODE_MAGIC);

        // Import should still succeed (detected as JSON).
        let info = import_workspace(
            &mut mgr,
            "jonly-imported",
            export_path.to_str().unwrap(),
            false,
        )
        .unwrap();
        assert_eq!(info.atom_count, 1);
    }
}
