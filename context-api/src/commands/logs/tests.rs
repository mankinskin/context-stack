use std::io::Write;

use tempfile::TempDir;

use super::*;

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

    let (entries, total) = get_log(tmp.path(), "test.log", None, 2, 0).unwrap();
    assert_eq!(total, 3);
    assert_eq!(entries.len(), 2);

    let (entries, total) = get_log(tmp.path(), "test.log", None, 2, 2).unwrap();
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
    write_test_log(
        tmp.path(),
        "20260314T000000_snapshot.log",
        r#"{"timestamp":"2026-03-14T00:00:00Z","level":"INFO","fields":{"message":"graph_snapshot captured"},"target":"test"}
"#,
    );
    write_test_log(
        tmp.path(),
        "20260314T000001_search.log",
        r#"{"timestamp":"2026-03-14T00:00:00Z","level":"INFO","fields":{"message":"running op","op_type": "search"},"target":"test"}
"#,
    );
    write_test_log(
        tmp.path(),
        "20260314T000002_insert.log",
        r#"{"timestamp":"2026-03-14T00:00:00Z","level":"INFO","fields":{"message":"running op","op_type": "insert"},"target":"test"}
"#,
    );
    write_test_log(
        tmp.path(),
        "20260314T000003_paths.log",
        r#"{"timestamp":"2026-03-14T00:00:00Z","level":"INFO","fields":{"message":"path_transition detected"},"target":"test"}
"#,
    );
    write_test_log(
        tmp.path(),
        "20260314T000004_plain.log",
        r#"{"timestamp":"2026-03-14T00:00:00Z","level":"INFO","fields":{"message":"nothing special"},"target":"test"}
"#,
    );

    let logs = list_logs(tmp.path(), None, 100).unwrap();
    assert_eq!(logs.len(), 5);

    let snapshot_log = logs
        .iter()
        .find(|l| l.filename.contains("snapshot"))
        .unwrap();
    assert!(snapshot_log.has_graph_snapshot);
    assert!(!snapshot_log.has_search_ops);

    let search_log = logs.iter().find(|l| l.filename.contains("search")).unwrap();
    assert!(search_log.has_search_ops);
    assert!(!search_log.has_insert_ops);

    let insert_log = logs.iter().find(|l| l.filename.contains("insert")).unwrap();
    assert!(insert_log.has_insert_ops);
    assert!(!insert_log.has_search_ops);

    let paths_log = logs.iter().find(|l| l.filename.contains("paths")).unwrap();
    assert!(paths_log.has_search_paths);

    let plain_log = logs.iter().find(|l| l.filename.contains("plain")).unwrap();
    assert!(!plain_log.has_graph_snapshot);
    assert!(!plain_log.has_search_ops);
    assert!(!plain_log.has_insert_ops);
    assert!(!plain_log.has_search_paths);
}

#[test]
fn test_list_logs_feature_flags_escaped_format() {
    let tmp = TempDir::new().unwrap();
    write_test_log(
        tmp.path(),
        "20260314T000000_escaped.log",
        r#"{"timestamp":"2026-03-14T00:00:00Z","level":"INFO","fields":{"message":"{\"op_type\":\"search\"}"},"target":"test"}"#,
    );

    let logs = list_logs(tmp.path(), None, 100).unwrap();
    assert_eq!(logs.len(), 1);
    assert!(logs[0].has_search_ops);
}