//! Per-command tracing capture for CLI and MCP binaries.
//!
//! Provides [`capture_traced`] which wraps a closure with a scoped tracing
//! dispatcher that captures structured JSON events to a log file.

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::atomic::Ordering,
    time::Instant,
};

use context_trace::logging::build_capture_dispatch;

use crate::types::TraceSummary;

/// Configuration for per-command tracing capture.
#[derive(Debug, Clone)]
pub struct CaptureConfig {
    /// Whether capture is enabled.
    pub enabled: bool,
    /// The log directory to write to.
    pub log_dir: PathBuf,
    /// Minimum tracing level to capture (default: "TRACE").
    pub level: String,
}

/// Result of a traced execution.
#[derive(Debug)]
pub struct CaptureResult<T> {
    /// The inner result from the closure.
    pub result: T,
    /// Path to the log file (if capture was enabled).
    pub log_file: Option<PathBuf>,
    /// Summary of what was captured (if capture was enabled).
    pub summary: Option<TraceSummary>,
}

/// Generate a log filename from a command name.
///
/// Format: `<YYYYMMDD>T<HHMMSS><millis>_<command_name>.log`
fn log_filename(command_name: &str) -> String {
    let now = chrono::Utc::now();
    format!("{}_{}.log", now.format("%Y%m%dT%H%M%S%3f"), command_name,)
}

/// Wrap a closure with per-command tracing capture.
///
/// Creates a scoped tracing dispatcher that writes JSON events to a
/// log file in `config.log_dir` for the duration of the closure.
/// Uses the same format as `TestTracing`, compatible with `LogParser`.
///
/// # Arguments
///
/// * `config` — Capture configuration (must have `enabled: true` to actually capture).
/// * `command_name` — Used in the log filename.
/// * `f` — The closure to execute under the scoped dispatcher.
///
/// # Returns
///
/// A `CaptureResult<T>` containing the closure's return value and
/// optional capture metadata.
pub fn capture_traced<F, T>(
    config: &CaptureConfig,
    command_name: &str,
    f: F,
) -> CaptureResult<T>
where
    F: FnOnce() -> T,
{
    if !config.enabled {
        return CaptureResult {
            result: f(),
            log_file: None,
            summary: None,
        };
    }

    let filename = log_filename(command_name);
    let log_path = config.log_dir.join(&filename);

    let capture = match build_capture_dispatch(&log_path, &config.level) {
        Ok(c) => c,
        Err(_) => {
            // If we can't create the capture, run without it
            return CaptureResult {
                result: f(),
                log_file: None,
                summary: None,
            };
        },
    };

    let start = Instant::now();
    let result = tracing::dispatcher::with_default(&capture.dispatch, f);
    let duration = start.elapsed();

    let entry_count = capture.event_count.load(Ordering::Relaxed);

    let summary = TraceSummary {
        log_file: filename,
        entry_count,
        event_summary: HashMap::new(), // Populated by callers who parse the log
        duration_ms: duration.as_millis() as u64,
    };

    CaptureResult {
        result,
        log_file: Some(log_path),
        summary: Some(summary),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn capture_disabled_runs_closure() {
        let tmp = TempDir::new().unwrap();
        let config = CaptureConfig {
            enabled: false,
            log_dir: tmp.path().to_path_buf(),
            level: "TRACE".to_string(),
        };

        let result = capture_traced(&config, "test_cmd", || 42);
        assert_eq!(result.result, 42);
        assert!(result.log_file.is_none());
        assert!(result.summary.is_none());
    }

    #[test]
    fn capture_enabled_creates_log_file() {
        let tmp = TempDir::new().unwrap();
        let config = CaptureConfig {
            enabled: true,
            log_dir: tmp.path().to_path_buf(),
            level: "TRACE".to_string(),
        };

        let result = capture_traced(&config, "test_cmd", || {
            tracing::info!("test event inside capture");
            42
        });

        assert_eq!(result.result, 42);
        assert!(result.log_file.is_some());
        assert!(result.log_file.as_ref().unwrap().exists());
        assert!(result.summary.is_some());

        let summary = result.summary.unwrap();
        assert!(summary.log_file.contains("test_cmd"));
        assert!(summary.log_file.ends_with(".log"));
        // At least one event should have been captured
        assert!(
            summary.entry_count >= 1,
            "Expected at least 1 event, got {}",
            summary.entry_count
        );
    }

    #[test]
    fn log_filename_format() {
        let name = log_filename("insert_sequence");
        assert!(name.ends_with("_insert_sequence.log"));
        // Should start with a date-like pattern
        assert!(name.len() > 25);
    }
}
