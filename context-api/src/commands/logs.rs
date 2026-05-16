//! Log command implementations.
//!
//! Provides the business logic for the `ListLogs`, `GetLog`, `QueryLog`,
//! `AnalyzeLog`, `SearchLogs`, `DeleteLog`, and `DeleteLogs` command
//! variants.

mod cleanup;
#[cfg(test)]
mod tests;

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
        LogEntryInfo,
        LogFileInfo,
        LogFileSearchResult,
        SpanSummary,
    },
};

pub use cleanup::{
    delete_log,
    delete_logs,
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

    entries.sort_by(|a, b| {
        let a_time = a.metadata().and_then(|m| m.modified()).ok();
        let b_time = b.metadata().and_then(|m| m.modified()).ok();
        b_time.cmp(&a_time)
    });

    for entry in entries {
        let filename = entry.file_name().to_string_lossy().to_string();

        if let Some(pat) = pattern {
            if !filename.contains(pat) {
                continue;
            }
        }

        let metadata = entry.metadata().map_err(LogError::IoError)?;
        let modified = metadata
            .modified()
            .ok()
            .map(|t| {
                let datetime: chrono::DateTime<chrono::Utc> = t.into();
                datetime.to_rfc3339()
            })
            .unwrap_or_default();

        let command = filename
            .rsplit_once('_')
            .and_then(|(_, cmd)| cmd.strip_suffix(".log"))
            .or_else(|| filename.strip_suffix(".log"))
            .unwrap_or(&filename)
            .to_string();

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

    let values: Vec<serde_json::Value> = entries
        .iter()
        .filter_map(|e| serde_json::to_value(e).ok())
        .collect();

    let matching_values = crate::jq::filter_values(values.iter(), query)
        .map_err(|e| LogError::QueryError(e.to_string()))?;

    let total = matching_values.len();
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

    let mut by_level: HashMap<String, usize> = HashMap::new();
    for entry in &entries {
        *by_level.entry(entry.level.clone()).or_insert(0) += 1;
    }

    let mut by_event_type: HashMap<String, usize> = HashMap::new();
    for entry in &entries {
        *by_event_type.entry(entry.event_type.clone()).or_insert(0) += 1;
    }

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
                    let log_entry: Result<LogEntry, _> = serde_json::from_value(v);
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