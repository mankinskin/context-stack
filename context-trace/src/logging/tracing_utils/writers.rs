//! Log file writers for test tracing
//!
//! Provides `FlushingWriter` (ensures data is flushed on every write for panic safety)
//! and `PrettyJsonWriter` (buffers JSON, applies transformations, pretty-prints).

use std::{
    fs,
    io::Write,
    sync::{
        Arc,
        Mutex,
    },
};

use super::debug_to_json::{
    convert_paths_to_unix,
    transform_structured_fields,
};

/// A file wrapper that flushes after every write to ensure logs are visible on panic.
///
/// This is necessary because when a test panics, buffered data may not be flushed
/// to disk, resulting in truncated log files.
#[derive(Clone)]
pub(super) struct FlushingWriter {
    file: Arc<Mutex<fs::File>>,
}

impl FlushingWriter {
    pub(super) fn new(file: fs::File) -> Self {
        Self {
            file: Arc::new(Mutex::new(file)),
        }
    }
}

impl Write for FlushingWriter {
    fn write(
        &mut self,
        buf: &[u8],
    ) -> std::io::Result<usize> {
        // Use lock().ok() to handle poisoned mutex during panic gracefully
        // If we can't get the lock (e.g., during unwind), skip the write
        let Some(mut file) = self.file.lock().ok().or_else(|| {
            // Mutex is poisoned, try to recover it
            self.file.clear_poison();
            self.file.lock().ok()
        }) else {
            return Err(std::io::Error::other("Failed to acquire file lock"));
        };
        let result = file.write(buf)?;
        file.flush()?;
        Ok(result)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if let Some(mut file) = self.file.lock().ok().or_else(|| {
            self.file.clear_poison();
            self.file.lock().ok()
        }) {
            file.flush()
        } else {
            Ok(())
        }
    }
}

/// A writer that pretty-prints JSON output with indentation
///
/// Wraps another writer and buffers JSON objects. When a complete JSON
/// object is detected, it's parsed and re-serialized with indentation.
#[derive(Clone)]
pub(super) struct PrettyJsonWriter<W> {
    inner: W,
    buffer: Arc<Mutex<Vec<u8>>>,
}

impl<W: Clone> PrettyJsonWriter<W> {
    pub(super) fn new(writer: W) -> Self {
        Self {
            inner: writer,
            buffer: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl<W: Write + Clone> Write for PrettyJsonWriter<W> {
    fn write(
        &mut self,
        buf: &[u8],
    ) -> std::io::Result<usize> {
        let mut buffer = self.buffer.lock().map_err(|_| {
            std::io::Error::other("Failed to acquire buffer lock")
        })?;

        // Add incoming data to buffer
        buffer.extend_from_slice(buf);

        // Check if we have a complete JSON object (ends with newline)
        if buffer.ends_with(b"\n") {
            // Try to parse and pretty-print the JSON
            if let Ok(json_str) = std::str::from_utf8(&buffer) {
                let trimmed = json_str.trim();
                if !trimmed.is_empty() {
                    if let Ok(mut value) =
                        serde_json::from_str::<serde_json::Value>(trimmed)
                    {
                        // Convert Windows paths to Unix paths
                        convert_paths_to_unix(&mut value);

                        // Transform structured fields (fn_sig, etc.) into JSON objects
                        transform_structured_fields(&mut value);

                        // Write pretty-printed JSON
                        let pretty = serde_json::to_string_pretty(&value)
                            .unwrap_or_else(|_| trimmed.to_string());
                        let mut inner = self.inner.clone();
                        inner.write_all(pretty.as_bytes())?;
                        inner.write_all(b"\n\n")?; // Double newline between entries
                        inner.flush()?;
                        buffer.clear();
                        return Ok(buf.len());
                    }
                }
            }

            // Fallback: write raw data if JSON parsing fails
            let mut inner = self.inner.clone();
            inner.write_all(&buffer)?;
            inner.flush()?;
            buffer.clear();
        }

        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.clone().flush()
    }
}
