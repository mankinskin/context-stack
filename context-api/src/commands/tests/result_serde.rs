use crate::types::{
    AtomInfo,
    InsertResult,
    SearchResult,
    TokenInfo,
    ValidationReport,
    WorkspaceInfo,
};

use super::super::CommandResult;

#[test]
fn result_serde_ok() {
    let result = CommandResult::Ok;
    let json = serde_json::to_string(&result).unwrap();
    let deser: CommandResult = serde_json::from_str(&json).unwrap();
    assert!(matches!(deser, CommandResult::Ok));
}

#[test]
fn result_serde_atom_info() {
    let result = CommandResult::AtomInfo(AtomInfo { index: 0, ch: 'a' });
    let json = serde_json::to_string(&result).unwrap();
    let deser: CommandResult = serde_json::from_str(&json).unwrap();
    match deser {
        CommandResult::AtomInfo(info) => {
            assert_eq!(info.index, 0);
            assert_eq!(info.ch, 'a');
        },
        other => panic!("expected AtomInfo, got: {other:?}"),
    }
}

#[test]
fn result_serde_workspace_info() {
    let info = WorkspaceInfo {
        name: "demo".into(),
        vertex_count: 10,
        atom_count: 5,
        pattern_count: 5,
        created_at: "2025-01-01T00:00:00Z".into(),
        modified_at: "2025-01-02T00:00:00Z".into(),
    };
    let result = CommandResult::WorkspaceInfo(info);
    let json = serde_json::to_string(&result).unwrap();
    let deser: CommandResult = serde_json::from_str(&json).unwrap();
    match deser {
        CommandResult::WorkspaceInfo(i) => {
            assert_eq!(i.name, "demo");
            assert_eq!(i.vertex_count, 10);
        },
        other => panic!("expected WorkspaceInfo, got: {other:?}"),
    }
}

#[test]
fn result_serde_search_result() {
    let result = CommandResult::SearchResult(SearchResult {
        complete: true,
        token: Some(TokenInfo {
            index: 5,
            label: "ab".into(),
            width: 2,
        }),
        query_exhausted: true,
        partial: None,
    });
    let json = serde_json::to_string(&result).unwrap();
    let deser: CommandResult = serde_json::from_str(&json).unwrap();
    match deser {
        CommandResult::SearchResult(sr) => {
            assert!(sr.complete);
            assert!(sr.token.is_some());
        },
        other => panic!("expected SearchResult, got: {other:?}"),
    }
}

#[test]
fn result_serde_insert_result() {
    let result = CommandResult::InsertResult(InsertResult {
        token: TokenInfo {
            index: 7,
            label: "hello".into(),
            width: 5,
        },
        already_existed: false,
    });
    let json = serde_json::to_string(&result).unwrap();
    let deser: CommandResult = serde_json::from_str(&json).unwrap();
    match deser {
        CommandResult::InsertResult(ir) => {
            assert!(!ir.already_existed);
            assert_eq!(ir.token.label, "hello");
        },
        other => panic!("expected InsertResult, got: {other:?}"),
    }
}

#[test]
fn result_serde_text() {
    let result = CommandResult::Text {
        text: "hello world".into(),
    };
    let json = serde_json::to_string(&result).unwrap();
    let deser: CommandResult = serde_json::from_str(&json).unwrap();
    match deser {
        CommandResult::Text { text } => {
            assert_eq!(text, "hello world");
        },
        other => panic!("expected Text, got: {other:?}"),
    }
}

#[test]
fn result_serde_validation_report() {
    let result = CommandResult::ValidationReport(ValidationReport {
        valid: true,
        vertex_count: 5,
        issues: vec![],
    });
    let json = serde_json::to_string(&result).unwrap();
    let deser: CommandResult = serde_json::from_str(&json).unwrap();
    match deser {
        CommandResult::ValidationReport(vr) => {
            assert!(vr.valid);
            assert_eq!(vr.vertex_count, 5);
        },
        other => panic!("expected ValidationReport, got: {other:?}"),
    }
}