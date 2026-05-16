use super::{
    add_atoms,
    tmp_manager,
};
use super::super::*;

#[test]
fn import_overwrite_false_fails_if_exists() {
    let (tmp, mut mgr) = tmp_manager();

    mgr.create_workspace("original").unwrap();
    add_atoms(&mut mgr, "original", &['a']);

    let export_path = tmp.path().join("overwrite_test.json");
    export_workspace(
        &mgr,
        "original",
        ExportFormat::Json,
        Some(export_path.to_str().unwrap()),
    )
    .unwrap();

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

    mgr.create_workspace("overme").unwrap();
    add_atoms(&mut mgr, "overme", &['a']);
    mgr.save_workspace("overme").unwrap();

    let export_path = tmp.path().join("for_overwrite.json");
    export_workspace(
        &mgr,
        "overme",
        ExportFormat::Json,
        Some(export_path.to_str().unwrap()),
    )
    .unwrap();

    mgr.close_workspace("overme").unwrap();

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
        WorkspaceError::IoError(_) => {},
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

#[test]
fn json_export_auto_detected_on_import() {
    let (tmp, mut mgr) = tmp_manager();

    mgr.create_workspace("jsrc").unwrap();
    add_atoms(&mut mgr, "jsrc", &['a', 'b']);

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

#[test]
fn import_renames_workspace_but_preserves_created_at() {
    let (tmp, mut mgr) = tmp_manager();

    mgr.create_workspace("stamped").unwrap();

    let export_path = tmp.path().join("stamped.json");
    export_workspace(
        &mgr,
        "stamped",
        ExportFormat::Json,
        Some(export_path.to_str().unwrap()),
    )
    .unwrap();

    let export_content = fs::read_to_string(&export_path).unwrap();
    let export_json: serde_json::Value =
        serde_json::from_str(&export_content).unwrap();
    let original_created = export_json["metadata"]["created_at"]
        .as_str()
        .unwrap()
        .to_string();

    let info = import_workspace(
        &mut mgr,
        "stamped-copy",
        export_path.to_str().unwrap(),
        false,
    )
    .unwrap();

    assert_eq!(info.name, "stamped-copy");

    let original_dt =
        chrono::DateTime::parse_from_rfc3339(&original_created).unwrap();
    let imported_dt =
        chrono::DateTime::parse_from_rfc3339(&info.created_at).unwrap();
    assert_eq!(original_dt, imported_dt);
}

#[test]
fn import_overwrite_closes_open_workspace() {
    let (tmp, mut mgr) = tmp_manager();

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

    mgr.close_workspace("src-ow").unwrap();
    mgr.create_workspace("target-ow").unwrap();
    assert!(mgr.is_open("target-ow"));

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