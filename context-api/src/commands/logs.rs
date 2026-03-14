//! Log command implementations.
//!
//! Provides the business logic for the `ListLogs`, `GetLog`, `QueryLog`,
//! `AnalyzeLog`, `SearchLogs`, `DeleteLog`, and `DeleteLogs` command
//! variants.

use std::{
    collections::HashMap,
    fs,
    path::Path,
};

use crate::{
    error::LogError,
    log_parser::{
        LogEntry,
        LogParser,
    },
    types::{
        LogAnalysis,
        LogDeleteResult,
        LogEntryInfo,
        LogFileInfo,
        LogFileSearchResult,
        SpanSummary,
    },
};

/// List log files in a directory, optionally filtering by pattern.
pub fn list_logs(
    log_dir: &Path,
    pattern: Option<&str>,
    limit: usize,
) -> Result<Vec<LogFileInfo>, LogError> {
    let mut logs = Vec::new();

    if !log_dir.exists() {
        return Ok(logs);
    }

    let mut entries: Vec<_> = fs::read_dir(log_dir)
        .map_err(LogError::IoError)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "log"))
        .collect();

    // Sort by modification time (newest first)
    entries.sort_by(|a, b| {
        let a_time = a.metadata().and_then(|m| m.modified()).ok();
        let b_time = b.metadata().and_then(|m| m.modified()).ok();
        b_time.cmp(&a_time)
    });

    for entry in entries {
        let filename = entry.file_name().to_string_lossy().to_string();

        // Apply pattern filter if provided
        if let Some(pat) = pattern {
            if !filename.contains(pat) {
                continue;
            }
        }

        let metadata = entry.metadata().map_err(LogError::IoError)?;
        let modified = metadata
            .modified()
            .ok()
            .and_then(|t| {
                let datetime: chrono::DateTime<chrono::Utc> = t.into();
                Some(datetime.to_rfc3339())
            })
            .unwrap_or_default();

        // Extract command name from filename: <timestamp>_<command>.log
        let command = filename
            .rsplit_once('_')
            .and_then(|(_, cmd)| cmd.strip_suffix(".log"))
            .or_else(|| filename.strip_suffix(".log"))
            .unwrap_or(&filename)
            .to_string();

        // Quick scan for feature-flag markers (check first 64KB for speed)
        let path = entry.path();
        let bytes = fs::read(&path).unwrap_or_default();
        let scan_len = bytes.len().min(64 * 1024);
        let scan_bytes = &bytes[..scan_len];

        let has_graph_snapshot = scan_bytes
            .windows(b"graph_snapshot".len())
            .any(|w| w == b"graph_snapshot");

        let has_search_ops = scan_bytes
            .windows(br#"\"op_type\":\"search\""#.len())
            .any(|w| w == br#"\"op_type\":\"search\""#)
            || scan_bytes
                .windows(br#""op_type": "search""#.len())
                .any(|w| w == br#""op_type": "search""#);

        let has_insert_ops = scan_bytes
            .windows(br#"\"op_type\":\"insert\""#.len())
            .any(|w| w == br#"\"op_type\":\"insert\""#)
            || scan_bytes
                .windows(br#""op_type": "insert""#.len())
                .any(|w| w == br#""op_type": "insert""#);

        let has_search_paths = scan_bytes
            .windows(b"path_transition".len())
            .any(|w| w == b"path_transition");

        logs.push(LogFileInfo {
            filename,
            size: metadata.len(),
            modified,
            command,
            has_graph_snapshot,
            has_search_ops,
            has_insert_ops,
            has_search_paths,
        });

        if logs.len() >= limit {
            break;
        }
    }

    Ok(logs)
}

/// Read and parse a log file, with optional level filter and pagination.
///
/// Returns a tuple of `(entries, total_matching_count)`.
pub fn get_log(
    log_dir: &Path,
    filename: &str,
    filter: Option<&str>,
    limit: usize,
    offset: usize,
) -> Result<(Vec<LogEntryInfo>, usize), LogError> {
    let path = log_dir.join(filename);
    if !path.exists() {
        return Err(LogError::FileNotFound {
            filename: filename.to_string(),
        });
    }

    let content = fs::read_to_string(&path).map_err(LogError::IoError)?;
    let parser = LogParser::new();
    let entries = parser.parse(&content);

    // Apply level filter if provided
    let filtered: Vec<&LogEntry> = if let Some(level) = filter {
        let level_upper = level.to_uppercase();
        entries.iter().filter(|e| e.level == level_upper).collect()
    } else {
        entries.iter().collect()
    };

    let total = filtered.len();
    let page: Vec<LogEntryInfo> = filtered
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(LogEntryInfo::from)
        .collect();

    Ok((page, total))
}

/// Run a JQ query against a log file.
///
/// Returns a tuple of `(matching_entries, total_match_count)`.
pub fn query_log(
    log_dir: &Path,
    filename: &str,
    query: &str,
    limit: usize,
) -> Result<(Vec<LogEntryInfo>, usize), LogError> {
    let path = log_dir.join(filename);
    if !path.exists() {
        return Err(LogError::FileNotFound {
            filename: filename.to_string(),
        });
    }

    let content = fs::read_to_string(&path).map_err(LogError::IoError)?;
    let parser = LogParser::new();
    let entries = parser.parse(&content);

    // Convert entries to JSON values for JQ filtering
    let values: Vec<serde_json::Value> = entries
        .iter()
        .filter_map(|e| serde_json::to_value(e).ok())
        .collect();

    let matching_values = crate::jq::filter_values(values.iter(), query)
        .map_err(|e| LogError::QueryError(e.to_string()))?;

    let total = matching_values.len();

    // Convert matching values back to LogEntryInfo
    let results: Vec<LogEntryInfo> = matching_values
        .into_iter()
        .take(limit)
        .filter_map(|v| {
            let entry: Result<LogEntry, _> = serde_json::from_value(v);
            entry.ok().map(|e| LogEntryInfo::from(&e))
        })
        .collect();

    Ok((results, total))
}

/// Analyze a log file and produce summary statistics.
pub fn analyze_log(
    log_dir: &Path,
    filename: &str,
) -> Result<LogAnalysis, LogError> {
    let path = log_dir.join(filename);
    if !path.exists() {
        return Err(LogError::FileNotFound {
            filename: filename.to_string(),
        });
    }

    let content = fs::read_to_string(&path).map_err(LogError::IoError)?;
    let parser = LogParser::new();
    let entries = parser.parse(&content);

    let total_entries = entries.len();

    // Count by level
    let mut by_level: HashMap<String, usize> = HashMap::new();
    for entry in &entries {
        *by_level.entry(entry.level.clone()).or_insert(0) += 1;
    }

    // Count by event type
    let mut by_event_type: HashMap<String, usize> = HashMap::new();
    for entry in &entries {
        *by_event_type.entry(entry.event_type.clone()).or_insert(0) += 1;
    }

    // Summarize spans
    let mut span_counts: HashMap<String, (usize, bool)> = HashMap::new();
    for entry in &entries {
        if let Some(ref name) = entry.span_name {
            let (count, has_errors) =
                span_counts.entry(name.clone()).or_insert((0, false));
            *count += 1;
            if entry.level == "ERROR" {
                *has_errors = true;
            }
        }
    }

    let spans: Vec<SpanSummary> = span_counts
        .into_iter()
        .map(|(name, (count, has_errors))| SpanSummary {
            name,
            count,
            has_errors,
        })
        .collect();

    // Collect error entries
    let errors: Vec<LogEntryInfo> = entries
        .iter()
        .filter(|e| e.level == "ERROR")
        .map(LogEntryInfo::from)
        .collect();

    Ok(LogAnalysis {
        total_entries,
        by_level,
        by_event_type,
        spans,
        errors,
    })
}

/// Search across all log files in a directory using a JQ query.
///
/// Returns a tuple of `(per_file_results, total_files_with_matches)`.
pub fn search_logs(
    log_dir: &Path,
    query: &str,
    limit_per_file: usize,
) -> Result<(Vec<LogFileSearchResult>, usize), LogError> {
    let mut results = Vec::new();
    let mut total_files_with_matches = 0;

    if !log_dir.exists() {
        return Ok((results, 0));
    }

    let mut entries: Vec<_> = fs::read_dir(log_dir)
        .map_err(LogError::IoError)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "log"))
        .collect();

    // Sort by modification time (newest first)
    entries.sort_by(|a, b| {
        let a_time = a.metadata().and_then(|m| m.modified()).ok();
        let b_time = b.metadata().and_then(|m| m.modified()).ok();
        b_time.cmp(&a_time)
    });

    for entry in entries {
        let filename = entry.file_name().to_string_lossy().to_string();
        let path = entry.path();

        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let parser = LogParser::new();
        let log_entries = parser.parse(&content);

        let values: Vec<serde_json::Value> = log_entries
            .iter()
            .filter_map(|e| serde_json::to_value(e).ok())
            .collect();

        let matching = match crate::jq::filter_values(values.iter(), query) {
            Ok(m) => m,
            Err(e) => return Err(LogError::QueryError(e.to_string())),
        };

        if !matching.is_empty() {
            total_files_with_matches += 1;
            let match_count = matching.len();
            let matched_entries: Vec<LogEntryInfo> = matching
                .into_iter()
                .take(limit_per_file)
                .filter_map(|v| {
                    let log_entry: Result<LogEntry, _> =
                        serde_json::from_value(v);
                    log_entry.ok().map(|e| LogEntryInfo::from(&e))
                })
                .collect();

            results.push(LogFileSearchResult {
                filename,
                matches: match_count,
                entries: matched_entries,
            });
        }
    }

    Ok((results, total_files_with_matches))
}

/// Delete a specific log file.
pub fn delete_log(
    log_dir: &Path,
    filename: &str,
) -> Result<(), LogError> {
    let path = log_dir.join(filename);
    if !path.exists() {
        return Err(LogError::FileNotFound {
            filename: filename.to_string(),
        });
    }
    fs::remove_file(&path).map_err(LogError::IoError)?;
    Ok(())
}

/// Delete log files, optionally only those older than the given number of days.
///
/// If `older_than_days` is `None`, all `.log` files in the directory are deleted.
pub fn delete_logs(
    log_dir: &Path,
    older_than_days: Option<u32>,
) -> Result<LogDeleteResult, LogError> {
    let mut deleted_count: usize = 0;
    let mut freed_bytes: u64 = 0;

    if !log_dir.exists() {
        return Ok(LogDeleteResult {
            deleted_count: 0,
            freed_bytes: 0,
        });
    }

    let cutoff = older_than_days.map(|days| {
        std::time::SystemTime::now()
            - std::time::Duration::from_secs(u64::from(days) * 86400)
    });

    let entries = fs::read_dir(log_dir).map_err(LogError::IoError)?;

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "log") {
            let should_delete = if let Some(cutoff_time) = cutoff {
                entry
                    .metadata()
                    .and_then(|m| m.modified())
                    .map(|modified| modified < cutoff_time)
                    .unwrap_or(false)
            } else {
                true // No age filter — delete all
            };

            if should_delete {
                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                if fs::remove_file(&path).is_ok() {
                    deleted_count += 1;
                    freed_bytes += size;
                }
            }
        }
    }

    Ok(LogDeleteResult {
        deleted_count,
        freed_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn write_test_log(
        dir: &Path,
        name: &str,
        content: &str,
    ) {
        let path = dir.join(name);
        let mut f = fs::File::create(path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
    }

    const SAMPLE_LOG: &str = r#"{"timestamp":"2026-03-14T00:00:00Z","level":"INFO","fields":{"message":"hello world","step":1},"target":"test"}

{"timestamp":"2026-03-14T00:00:01Z","level":"ERROR","fields":{"message":"something failed"},"target":"test"}

{"timestamp":"2026-03-14T00:00:02Z","level":"INFO","fields":{"message":"recovered ok","step":2},"target":"test","span":"my_span","spans":["my_span"]}
"#;

    #[test]
    fn test_list_logs_empty() {
        let tmp = TempDir::new().unwrap();
        let logs = list_logs(tmp.path(), None, 100).unwrap();
        assert!(logs.is_empty());
    }

    #[test]
    fn test_list_logs_nonexistent_dir() {
        let logs = list_logs(Path::new("/nonexistent/dir"), None, 100).unwrap();
        assert!(logs.is_empty());
    }

    #[test]
    fn test_list_logs_with_files() {
        let tmp = TempDir::new().unwrap();
        write_test_log(tmp.path(), "20260314T000000_insert.log", SAMPLE_LOG);
        write_test_log(tmp.path(), "20260314T000001_search.log", SAMPLE_LOG);
        write_test_log(tmp.path(), "not_a_log.txt", "ignore me");

        let logs = list_logs(tmp.path(), None, 100).unwrap();
        assert_eq!(logs.len(), 2);
        // All entries should have .log extension filenames
        for log in &logs {
            assert!(log.filename.ends_with(".log"));
        }
    }

    #[test]
    fn test_list_logs_with_pattern() {
        let tmp = TempDir::new().unwrap();
        write_test_log(tmp.path(), "20260314T000000_insert.log", SAMPLE_LOG);
        write_test_log(tmp.path(), "20260314T000001_search.log", SAMPLE_LOG);

        let logs = list_logs(tmp.path(), Some("insert"), 100).unwrap();
        assert_eq!(logs.len(), 1);
        assert!(logs[0].filename.contains("insert"));
    }

    #[test]
    fn test_list_logs_with_limit() {
        let tmp = TempDir::new().unwrap();
        write_test_log(tmp.path(), "a.log", SAMPLE_LOG);
        write_test_log(tmp.path(), "b.log", SAMPLE_LOG);
        write_test_log(tmp.path(), "c.log", SAMPLE_LOG);

        let logs = list_logs(tmp.path(), None, 2).unwrap();
        assert_eq!(logs.len(), 2);
    }

    #[test]
    fn test_list_logs_command_extraction() {
        let tmp = TempDir::new().unwrap();
        write_test_log(tmp.path(), "20260314T000000_insert.log", SAMPLE_LOG);

        let logs = list_logs(tmp.path(), None, 100).unwrap();
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].command, "insert");
    }

    #[test]
    fn test_get_log() {
        let tmp = TempDir::new().unwrap();
        write_test_log(tmp.path(), "test.log", SAMPLE_LOG);

        let (entries, total) =
            get_log(tmp.path(), "test.log", None, 100, 0).unwrap();
        assert_eq!(total, 3);
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].level, "INFO");
        assert_eq!(entries[1].level, "ERROR");
    }

    #[test]
    fn test_get_log_with_filter() {
        let tmp = TempDir::new().unwrap();
        write_test_log(tmp.path(), "test.log", SAMPLE_LOG);

        let (entries, total) =
            get_log(tmp.path(), "test.log", Some("ERROR"), 100, 0).unwrap();
        assert_eq!(total, 1);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].level, "ERROR");
    }

    #[test]
    fn test_get_log_with_filter_case_insensitive() {
        let tmp = TempDir::new().unwrap();
        write_test_log(tmp.path(), "test.log", SAMPLE_LOG);

        let (entries, total) =
            get_log(tmp.path(), "test.log", Some("error"), 100, 0).unwrap();
        assert_eq!(total, 1);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].level, "ERROR");
    }

    #[test]
    fn test_get_log_pagination() {
        let tmp = TempDir::new().unwrap();
        write_test_log(tmp.path(), "test.log", SAMPLE_LOG);

        let (entries, total) =
            get_log(tmp.path(), "test.log", None, 2, 0).unwrap();
        assert_eq!(total, 3);
        assert_eq!(entries.len(), 2);

        let (entries, total) =
            get_log(tmp.path(), "test.log", None, 2, 2).unwrap();
        assert_eq!(total, 3);
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn test_get_log_not_found() {
        let tmp = TempDir::new().unwrap();
        let result = get_log(tmp.path(), "nope.log", None, 100, 0);
        assert!(result.is_err());
        match result.unwrap_err() {
            LogError::FileNotFound { filename } => {
                assert_eq!(filename, "nope.log");
            },
            other => panic!("expected FileNotFound, got: {other}"),
        }
    }

    #[test]
    fn test_analyze_log() {
        let tmp = TempDir::new().unwrap();
        write_test_log(tmp.path(), "test.log", SAMPLE_LOG);

        let analysis = analyze_log(tmp.path(), "test.log").unwrap();
        assert_eq!(analysis.total_entries, 3);
        assert_eq!(*analysis.by_level.get("INFO").unwrap_or(&0), 2);
        assert_eq!(*analysis.by_level.get("ERROR").unwrap_or(&0), 1);
        assert_eq!(analysis.errors.len(), 1);
    }

    #[test]
    fn test_analyze_log_not_found() {
        let tmp = TempDir::new().unwrap();
        let result = analyze_log(tmp.path(), "nope.log");
        assert!(result.is_err());
    }

    #[test]
    fn test_delete_log() {
        let tmp = TempDir::new().unwrap();
        write_test_log(tmp.path(), "test.log", SAMPLE_LOG);
        assert!(tmp.path().join("test.log").exists());

        delete_log(tmp.path(), "test.log").unwrap();
        assert!(!tmp.path().join("test.log").exists());
    }

    #[test]
    fn test_delete_log_not_found() {
        let tmp = TempDir::new().unwrap();
        let result = delete_log(tmp.path(), "nope.log");
        assert!(result.is_err());
        match result.unwrap_err() {
            LogError::FileNotFound { filename } => {
                assert_eq!(filename, "nope.log");
            },
            other => panic!("expected FileNotFound, got: {other}"),
        }
    }

    #[test]
    fn test_delete_logs_all() {
        let tmp = TempDir::new().unwrap();
        write_test_log(tmp.path(), "a.log", "{}");
        write_test_log(tmp.path(), "b.log", "{}");

        let result = delete_logs(tmp.path(), None).unwrap();
        assert_eq!(result.deleted_count, 2);
    }

    #[test]
    fn test_delete_logs_nonexistent_dir() {
        let result = delete_logs(Path::new("/nonexistent/dir"), None).unwrap();
        assert_eq!(result.deleted_count, 0);
        assert_eq!(result.freed_bytes, 0);
    }

    #[test]
    fn test_delete_logs_preserves_non_log_files() {
        let tmp = TempDir::new().unwrap();
        write_test_log(tmp.path(), "a.log", "{}");
        write_test_log(tmp.path(), "b.txt", "keep me");

        let result = delete_logs(tmp.path(), None).unwrap();
        assert_eq!(result.deleted_count, 1);
        assert!(tmp.path().join("b.txt").exists());
    }

    #[cfg(feature = "jq")]
    #[test]
    fn test_query_log() {
        let tmp = TempDir::new().unwrap();
        write_test_log(tmp.path(), "test.log", SAMPLE_LOG);

        let (entries, total) = query_log(
            tmp.path(),
            "test.log",
            "select(.level == \"ERROR\")",
            100,
        )
        .unwrap();
        assert_eq!(total, 1);
        assert_eq!(entries.len(), 1);
    }

    #[cfg(feature = "jq")]
    #[test]
    fn test_query_log_not_found() {
        let tmp = TempDir::new().unwrap();
        let result = query_log(tmp.path(), "nope.log", ".", 100);
        assert!(result.is_err());
    }

    #[cfg(feature = "jq")]
    #[test]
    fn test_search_logs() {
        let tmp = TempDir::new().unwrap();
        write_test_log(tmp.path(), "a.log", SAMPLE_LOG);
        write_test_log(tmp.path(), "b.log", SAMPLE_LOG);

        let (results, files_with_matches) =
            search_logs(tmp.path(), "select(.level == \"ERROR\")", 10).unwrap();
        assert_eq!(files_with_matches, 2);
        assert_eq!(results.len(), 2);
        for r in &results {
            assert_eq!(r.matches, 1);
        }
    }

    #[cfg(feature = "jq")]
    #[test]
    fn test_search_logs_nonexistent_dir() {
        let (results, count) =
            search_logs(Path::new("/nonexistent"), ".", 10).unwrap();
        assert!(results.is_empty());
        assert_eq!(count, 0);
    }

    #[test]
    fn test_list_logs_feature_flags() {
        let tmp = TempDir::new().unwrap();
        // Log with graph_snapshot marker
        write_test_log(
            tmp.path(),
            "20260314T000000_snapshot.log",
            r#"{"timestamp":"2026-03-14T00:00:00Z","level":"INFO","fields":{"message":"graph_snapshot captured"},"target":"test"}
"#,
        );
        // Log with search op marker
        write_test_log(
            tmp.path(),
            "20260314T000001_search.log",
            r#"{"timestamp":"2026-03-14T00:00:00Z","level":"INFO","fields":{"message":"running op","op_type": "search"},"target":"test"}
"#,
        );
        // Log with insert op marker
        write_test_log(
            tmp.path(),
            "20260314T000002_insert.log",
            r#"{"timestamp":"2026-03-14T00:00:00Z","level":"INFO","fields":{"message":"running op","op_type": "insert"},"target":"test"}
"#,
        );
        // Log with path_transition marker
        write_test_log(
            tmp.path(),
            "20260314T000003_paths.log",
            r#"{"timestamp":"2026-03-14T00:00:00Z","level":"INFO","fields":{"message":"path_transition detected"},"target":"test"}
"#,
        );
        // Log with NO markers
        write_test_log(
            tmp.path(),
            "20260314T000004_plain.log",
            r#"{"timestamp":"2026-03-14T00:00:00Z","level":"INFO","fields":{"message":"nothing special"},"target":"test"}
"#,
        );

        let logs = list_logs(tmp.path(), None, 100).unwrap();
        assert_eq!(logs.len(), 5);

        // Find each log by filename and check flags
        let snapshot_log = logs
            .iter()
            .find(|l| l.filename.contains("snapshot"))
            .unwrap();
        assert!(snapshot_log.has_graph_snapshot);
        assert!(!snapshot_log.has_search_ops);

        let search_log =
            logs.iter().find(|l| l.filename.contains("search")).unwrap();
        assert!(search_log.has_search_ops);
        assert!(!search_log.has_insert_ops);

        let insert_log =
            logs.iter().find(|l| l.filename.contains("insert")).unwrap();
        assert!(insert_log.has_insert_ops);
        assert!(!insert_log.has_search_ops);

        let paths_log =
            logs.iter().find(|l| l.filename.contains("paths")).unwrap();
        assert!(paths_log.has_search_paths);

        let plain_log =
            logs.iter().find(|l| l.filename.contains("plain")).unwrap();
        assert!(!plain_log.has_graph_snapshot);
        assert!(!plain_log.has_search_ops);
        assert!(!plain_log.has_insert_ops);
        assert!(!plain_log.has_search_paths);
    }

    #[test]
    fn test_list_logs_feature_flags_escaped_format() {
        let tmp = TempDir::new().unwrap();
        // Log with escaped JSON format markers (double-encoded JSON where
        // quotes appear as literal backslash-quote sequences on disk)
        write_test_log(
            tmp.path(),
            "20260314T000000_escaped.log",
            r#"{"timestamp":"2026-03-14T00:00:00Z","level":"INFO","fields":{"message":"{\"op_type\":\"search\"}"},"target":"test"}"#,
        );

        let logs = list_logs(tmp.path(), None, 100).unwrap();
        assert_eq!(logs.len(), 1);
        assert!(logs[0].has_search_ops);
    }
}
