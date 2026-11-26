//! The `.ctx` graph file: header, metadata, graph, checksum.
//!
//! ```text
//! b"CTXG" | format_version: u32 | meta_len: u64 | meta (JSON)
//!         | graph_len: u64 | graph (bincode PositionedGraph) | crc32: u32
//! ```
//!
//! Integers are little endian; the checksum covers every preceding byte.
//! Files are replaced atomically (temporary file, fsync, rename, fsync of
//! the directory), so a crash leaves either the old or the new file.

use std::{
    fs::{
        self,
        File,
    },
    io::{
        BufReader,
        Read,
        Write,
    },
    path::Path,
};

use context_trace::positioned::PositionedGraph;
use serde::{
    Deserialize,
    Serialize,
};

use crate::{
    error::ApiError,
    types::InputRecord,
};

pub const MAGIC: &[u8; 4] = b"CTXG";
pub const FORMAT_VERSION: u32 = 1;

/// Graph metadata stored in the file next to the graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphMeta {
    pub name: String,
    pub created_at: String,
    pub modified_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The input log; `id` equals the position.
    pub inputs: Vec<InputRecord>,
    /// Counts at the last save, so listing a store needs no graph decode.
    pub vertex_count: usize,
    pub atom_count: usize,
    pub generation: u64,
}

impl GraphMeta {
    pub fn new(name: &str) -> Self {
        let now = now();
        Self {
            name: name.to_string(),
            created_at: now.clone(),
            modified_at: now,
            description: None,
            inputs: Vec::new(),
            vertex_count: 0,
            atom_count: 0,
            generation: 0,
        }
    }

    pub fn total_input_chars(&self) -> usize {
        self.inputs.iter().map(|input| input.width).sum()
    }
}

pub(crate) fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn corrupt(
    path: &Path,
    reason: impl Into<String>,
) -> ApiError {
    ApiError::Corrupt {
        path: path.display().to_string(),
        reason: reason.into(),
    }
}

/// Encode a graph file.
pub fn encode(
    meta: &GraphMeta,
    graph: &PositionedGraph,
) -> Result<Vec<u8>, ApiError> {
    let meta = serde_json::to_vec(meta)
        .map_err(|e| ApiError::Internal(format!("encode metadata: {e}")))?;
    let graph = bincode::serialize(graph)
        .map_err(|e| ApiError::Internal(format!("encode graph: {e}")))?;
    let mut bytes = Vec::with_capacity(4 + 4 + 8 + meta.len() + 8 + graph.len() + 4);
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    bytes.extend_from_slice(&(meta.len() as u64).to_le_bytes());
    bytes.extend_from_slice(&meta);
    bytes.extend_from_slice(&(graph.len() as u64).to_le_bytes());
    bytes.extend_from_slice(&graph);
    let crc = crc32fast::hash(&bytes);
    bytes.extend_from_slice(&crc.to_le_bytes());
    Ok(bytes)
}

/// Decode and validate a graph file (checksum, metadata, graph).
pub fn decode(
    path: &Path,
    bytes: &[u8],
) -> Result<(GraphMeta, PositionedGraph), ApiError> {
    if bytes.len() < 4 + 4 + 8 + 8 + 4 {
        return Err(corrupt(path, "file too short"));
    }
    let (body, crc) = bytes.split_at(bytes.len() - 4);
    let crc = u32::from_le_bytes(crc.try_into().expect("4 bytes"));
    let mut cursor = Cursor { path, bytes: body };
    cursor.header()?;
    if crc32fast::hash(body) != crc {
        return Err(corrupt(path, "checksum mismatch"));
    }
    let meta = cursor.meta()?;
    let graph_bytes = cursor.section("graph")?;
    if !cursor.bytes.is_empty() {
        return Err(corrupt(path, "trailing bytes"));
    }
    let graph = bincode::deserialize(graph_bytes)
        .map_err(|e| corrupt(path, format!("graph: {e}")))?;
    Ok((meta, graph))
}

struct Cursor<'a> {
    path: &'a Path,
    bytes: &'a [u8],
}

impl<'a> Cursor<'a> {
    fn take(
        &mut self,
        n: usize,
        what: &str,
    ) -> Result<&'a [u8], ApiError> {
        if self.bytes.len() < n {
            return Err(corrupt(self.path, format!("truncated {what}")));
        }
        let (head, tail) = self.bytes.split_at(n);
        self.bytes = tail;
        Ok(head)
    }

    fn u64(
        &mut self,
        what: &str,
    ) -> Result<u64, ApiError> {
        Ok(u64::from_le_bytes(
            self.take(8, what)?.try_into().expect("8 bytes"),
        ))
    }

    fn header(&mut self) -> Result<(), ApiError> {
        if self.take(4, "magic")? != MAGIC {
            return Err(corrupt(self.path, "not a context graph file (bad magic)"));
        }
        let version =
            u32::from_le_bytes(self.take(4, "version")?.try_into().expect("4 bytes"));
        if version != FORMAT_VERSION {
            return Err(corrupt(
                self.path,
                format!("unsupported format version {version} (expected {FORMAT_VERSION})"),
            ));
        }
        Ok(())
    }

    fn section(
        &mut self,
        what: &str,
    ) -> Result<&'a [u8], ApiError> {
        let len = self.u64(what)?;
        let len = usize::try_from(len)
            .map_err(|_| corrupt(self.path, format!("{what} too large")))?;
        self.take(len, what)
    }

    fn meta(&mut self) -> Result<GraphMeta, ApiError> {
        let bytes = self.section("metadata")?;
        serde_json::from_slice(bytes)
            .map_err(|e| corrupt(self.path, format!("metadata: {e}")))
    }
}

/// Read only the header and metadata (no checksum or graph decode).
pub fn read_meta(path: &Path) -> Result<GraphMeta, ApiError> {
    let mut reader = BufReader::new(File::open(path)?);
    let mut head = [0u8; 16];
    reader
        .read_exact(&mut head)
        .map_err(|_| corrupt(path, "file too short"))?;
    let mut cursor = Cursor { path, bytes: &head };
    cursor.header()?;
    let len = cursor.u64("metadata")?;
    let len = usize::try_from(len).map_err(|_| corrupt(path, "metadata too large"))?;
    let mut meta = vec![0u8; len];
    reader
        .read_exact(&mut meta)
        .map_err(|_| corrupt(path, "truncated metadata"))?;
    serde_json::from_slice(&meta).map_err(|e| corrupt(path, format!("metadata: {e}")))
}

pub fn read(path: &Path) -> Result<(GraphMeta, PositionedGraph), ApiError> {
    let bytes = fs::read(path)?;
    decode(path, &bytes)
}

/// Atomically replace `path` with `bytes`.
pub fn write_atomic(
    path: &Path,
    bytes: &[u8],
) -> Result<(), ApiError> {
    let dir = path
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let file_name = path
        .file_name()
        .ok_or_else(|| ApiError::InvalidInput(format!("not a file path: {}", path.display())))?
        .to_string_lossy();
    let tmp = dir.join(format!(".{file_name}.tmp.{}", std::process::id()));
    let result = (|| {
        let mut file = File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        #[cfg(test)]
        tests::before_rename()?;
        fs::rename(&tmp, path)?;
        // Persist the rename; not supported on every platform.
        if let Ok(dir) = File::open(dir) {
            let _ = dir.sync_all();
        }
        Ok::<_, std::io::Error>(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    Ok(result?)
}

#[cfg(test)]
pub(crate) mod tests {
    use std::cell::Cell;

    use super::*;

    thread_local! {
        static FAIL_BEFORE_RENAME: Cell<bool> = const { Cell::new(false) };
    }

    /// Simulate a crash between writing the temporary file and the rename.
    pub(crate) fn fail_before_rename(fail: bool) {
        FAIL_BEFORE_RENAME.with(|f| f.set(fail));
    }

    pub(super) fn before_rename() -> std::io::Result<()> {
        if FAIL_BEFORE_RENAME.with(Cell::get) {
            return Err(std::io::Error::other("simulated crash before rename"));
        }
        Ok(())
    }

    fn sample() -> (GraphMeta, PositionedGraph) {
        let graph = context_trace::positioned::PositionedGraphRef::new(
            PositionedGraph::default(),
        );
        context_read::read(&graph, "abcabca".chars());
        context_read::read(&graph, "bcab".chars());
        let graph = graph.read().clone();
        let mut meta = GraphMeta::new("sample");
        meta.vertex_count = graph.vertex_count();
        (meta, graph)
    }

    #[test]
    fn roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("g.ctx");
        let (meta, graph) = sample();
        write_atomic(&path, &encode(&meta, &graph).unwrap()).unwrap();
        let (meta2, graph2) = read(&path).unwrap();
        assert_eq!(meta, meta2);
        assert_eq!(graph.to_interchange(), graph2.to_interchange());
        assert_eq!(read_meta(&path).unwrap(), meta);
    }

    #[test]
    fn crash_before_rename_keeps_old_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("g.ctx");
        let (meta, graph) = sample();
        let old = encode(&GraphMeta::new("sample"), &PositionedGraph::default()).unwrap();
        write_atomic(&path, &old).unwrap();
        fail_before_rename(true);
        assert!(write_atomic(&path, &encode(&meta, &graph).unwrap()).is_err());
        fail_before_rename(false);
        assert_eq!(fs::read(&path).unwrap(), old);
        assert!(read(&path).is_ok());
        let leftovers: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".tmp."))
            .collect();
        assert!(leftovers.is_empty());
    }

    #[test]
    fn corrupt_files_are_rejected() {
        let path = Path::new("g.ctx");
        let (meta, graph) = sample();
        let bytes = encode(&meta, &graph).unwrap();
        let reason = |bytes: &[u8]| match decode(path, bytes) {
            Err(ApiError::Corrupt { reason, .. }) => reason,
            other => panic!("expected Corrupt, got {other:?}"),
        };
        assert!(reason(&bytes[..10]).contains("too short"));
        let mut bad_magic = bytes.clone();
        bad_magic[0] = b'X';
        assert!(reason(&bad_magic).contains("magic"));
        let mut bad_version = bytes.clone();
        bad_version[4] = 9;
        assert!(reason(&bad_version).contains("version"));
        let mut flipped = bytes.clone();
        let middle = flipped.len() / 2;
        flipped[middle] ^= 0xff;
        assert!(reason(&flipped).contains("checksum"));
        assert!(reason(&bytes[..bytes.len() - 1]).contains("checksum"));
    }
}
