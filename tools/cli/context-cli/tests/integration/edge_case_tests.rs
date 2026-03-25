//! Category 6: Edge Cases & Error Handling Tests (6 tests)
//!
//! These tests validate error conditions and edge cases.

use crate::common::helpers::*;

#[test]
fn error_read_no_workspace() {
    let mut ws = TestWorkspace::new("err-no-ws");

    let result = ws.exec(context_api::commands::Command::ReadPattern {
        workspace: "nonexistent-ws".to_string(),
        index: 0,
    });
    assert!(
        result.is_err(),
        "reading from nonexistent workspace should error"
    );
}

#[test]
fn error_read_invalid_index() {
    let mut ws = TestWorkspace::new("err-bad-idx");

    let result = ws.exec(context_api::commands::Command::ReadPattern {
        workspace: ws.name.clone(),
        index: 999999,
    });
    assert!(result.is_err(), "reading invalid vertex index should error");
}

#[test]
fn error_read_closed_workspace() {
    let mut ws = TestWorkspace::new("err-closed");

    // Close the workspace
    ws.exec(context_api::commands::Command::CloseWorkspace {
        name: ws.name.clone(),
    })
    .expect("close should succeed");

    // Now try to read — should fail
    let result = ws.exec(context_api::commands::Command::ReadPattern {
        workspace: ws.name.clone(),
        index: 0,
    });
    assert!(
        result.is_err(),
        "reading from closed workspace should error"
    );
}

#[test]
fn edge_single_char() {
    let mut ws = TestWorkspace::new("edge-1char");

    let result = ws.read_sequence("x");
    let read = unwrap_read_result(&result);
    assert_eq!(read.text, "x");
    assert_eq!(read.root.width, 1);
}

#[test]
fn edge_two_chars() {
    let mut ws = TestWorkspace::new("edge-2char");

    let result = ws.read_sequence("ab");
    let read = unwrap_read_result(&result);
    assert_eq!(read.text, "ab");
    assert_eq!(read.root.width, 2);
}

#[test]
fn edge_repeated_single_char() {
    let mut ws = TestWorkspace::new("edge-repeat");

    let result = ws.read_sequence("aaaa");
    let read = unwrap_read_result(&result);
    assert_eq!(read.text, "aaaa");
    assert_eq!(read.root.width, 4);
}
