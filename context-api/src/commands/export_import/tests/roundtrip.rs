use super::{
    add_atoms,
    tmp_manager,
};
use super::super::*;

#[test]
fn export_import_json_round_trip() {
    let (tmp, mut mgr) = tmp_manager();

    mgr.create_workspace("source").unwrap();
    add_atoms(&mut mgr, "source", &['a', 'b', 'c']);

    let export_path = tmp.path().join("export.json");
    let result = export_workspace(
        &mgr,
        "source",
        ExportFormat::Json,
        Some(export_path.to_str().unwrap()),
    );
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
    assert!(export_path.exists());

    let content = fs::read_to_string(&export_path).unwrap();
    assert!(content.contains("context_api_version"));
    assert!(content.contains("source"));
    assert!(content.contains("graph_b64"));

    let info = import_workspace(
        &mut mgr,
        "imported",
        export_path.to_str().unwrap(),
        false,
    )
    .unwrap();

    assert_eq!(info.name, "imported");
    assert_eq!(info.atom_count, 3);
    assert_eq!(info.vertex_count, 3);
}

#[test]
fn export_import_bincode_round_trip() {
    let (tmp, mut mgr) = tmp_manager();

    mgr.create_workspace("binsrc").unwrap();
    add_atoms(&mut mgr, "binsrc", &['x', 'y']);

    let export_path = tmp.path().join("export.bin");
    export_workspace(
        &mgr,
        "binsrc",
        ExportFormat::Bincode,
        Some(export_path.to_str().unwrap()),
    )
    .unwrap();
    assert!(export_path.exists());

    let raw = fs::read(&export_path).unwrap();
    assert_eq!(&raw[0..4], BINCODE_MAGIC);

    let info = import_workspace(
        &mut mgr,
        "bin-imported",
        export_path.to_str().unwrap(),
        false,
    )
    .unwrap();

    assert_eq!(info.name, "bin-imported");
    assert_eq!(info.atom_count, 2);
    assert_eq!(info.vertex_count, 2);
}

#[test]
fn export_inline_json_returns_bytes() {
    let (_tmp, mut mgr) = tmp_manager();

    mgr.create_workspace("inline").unwrap();
    add_atoms(&mut mgr, "inline", &['a']);

    let result = export_workspace(&mgr, "inline", ExportFormat::Json, None)
        .unwrap();

    assert!(result.is_some());
    let bytes = result.unwrap();
    assert!(!bytes.is_empty());

    let parsed: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(parsed.get("context_api_version").is_some());
    assert!(parsed.get("metadata").is_some());
    assert!(parsed.get("graph_b64").is_some());
}

#[test]
fn export_inline_bincode_returns_bytes() {
    let (_tmp, mut mgr) = tmp_manager();

    mgr.create_workspace("inline-bin").unwrap();
    add_atoms(&mut mgr, "inline-bin", &['z']);

    let result = export_workspace(&mgr, "inline-bin", ExportFormat::Bincode, None)
        .unwrap();

    assert!(result.is_some());
    let bytes = result.unwrap();
    assert!(!bytes.is_empty());
    assert_eq!(&bytes[0..4], BINCODE_MAGIC);
}

#[test]
fn export_import_empty_workspace_json() {
    let (tmp, mut mgr) = tmp_manager();

    mgr.create_workspace("empty").unwrap();

    let export_path = tmp.path().join("empty.json");
    export_workspace(
        &mgr,
        "empty",
        ExportFormat::Json,
        Some(export_path.to_str().unwrap()),
    )
    .unwrap();

    let info = import_workspace(
        &mut mgr,
        "empty-copy",
        export_path.to_str().unwrap(),
        false,
    )
    .unwrap();

    assert_eq!(info.name, "empty-copy");
    assert_eq!(info.vertex_count, 0);
    assert_eq!(info.atom_count, 0);
}

#[test]
fn export_import_empty_workspace_bincode() {
    let (tmp, mut mgr) = tmp_manager();

    mgr.create_workspace("empty-bin").unwrap();

    let export_path = tmp.path().join("empty.bin");
    export_workspace(
        &mgr,
        "empty-bin",
        ExportFormat::Bincode,
        Some(export_path.to_str().unwrap()),
    )
    .unwrap();

    let info = import_workspace(
        &mut mgr,
        "empty-bin-copy",
        export_path.to_str().unwrap(),
        false,
    )
    .unwrap();

    assert_eq!(info.name, "empty-bin-copy");
    assert_eq!(info.vertex_count, 0);
    assert_eq!(info.atom_count, 0);
}

#[test]
fn json_export_contains_version() {
    let (_tmp, mut mgr) = tmp_manager();

    mgr.create_workspace("ver").unwrap();

    let bytes = export_workspace(&mgr, "ver", ExportFormat::Json, None)
        .unwrap()
        .unwrap();

    let parsed: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let version = parsed["context_api_version"].as_str().unwrap();
    assert_eq!(version, CONTEXT_API_VERSION);
}

#[test]
fn json_file_not_mistaken_for_bincode() {
    let (tmp, mut mgr) = tmp_manager();

    mgr.create_workspace("jonly").unwrap();
    add_atoms(&mut mgr, "jonly", &['a']);

    let export_path = tmp.path().join("json_only.json");
    export_workspace(
        &mgr,
        "jonly",
        ExportFormat::Json,
        Some(export_path.to_str().unwrap()),
    )
    .unwrap();

    let raw = fs::read(&export_path).unwrap();
    assert_ne!(&raw[0..4], BINCODE_MAGIC);

    let info = import_workspace(
        &mut mgr,
        "jonly-imported",
        export_path.to_str().unwrap(),
        false,
    )
    .unwrap();
    assert_eq!(info.atom_count, 1);
}