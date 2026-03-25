//! Category 4: File Input Tests (4 tests)
//!
//! These tests validate reading files through the graph via the
//! `Command::ReadFile` API.

use crate::common::helpers::*;

#[test]
fn file_read_basic() {
    let mut ws = TestWorkspace::new("file-basic");

    // Write a temp file
    let dir = ws.base_dir();
    let file_path = dir.join("test_input.txt");
    std::fs::write(&file_path, "hello world")
        .expect("failed to write test file");

    let result = ws.read_file(file_path.to_str().unwrap());
    match result {
        Ok(cmd_result) => {
            let read = unwrap_read_result(&cmd_result);
            assert_eq!(read.text, "hello world");
        },
        Err(e) => panic!("read_file failed: {e}"),
    }
}

#[test]
fn file_read_unicode() {
    let mut ws = TestWorkspace::new("file-unicode");

    let dir = ws.base_dir();
    let file_path = dir.join("unicode_input.txt");
    std::fs::write(&file_path, "café").expect("failed to write test file");

    let result = ws.read_file(file_path.to_str().unwrap());
    match result {
        Ok(cmd_result) => {
            let read = unwrap_read_result(&cmd_result);
            assert_eq!(read.text, "café");
        },
        Err(e) => panic!("read_file failed: {e}"),
    }
}

#[test]
fn file_read_empty() {
    let mut ws = TestWorkspace::new("file-empty");

    let dir = ws.base_dir();
    let file_path = dir.join("empty_input.txt");
    std::fs::write(&file_path, "").expect("failed to write test file");

    let result = ws.read_file(file_path.to_str().unwrap());
    // Empty file should return an error (SequenceTooShort)
    assert!(
        result.is_err(),
        "reading an empty file should return an error"
    );
}

#[test]
fn file_read_nonexistent() {
    let mut ws = TestWorkspace::new("file-noexist");

    let result = ws.read_file("/nonexistent/path/to/file.txt");
    assert!(
        result.is_err(),
        "reading a nonexistent file should return an error"
    );
}
