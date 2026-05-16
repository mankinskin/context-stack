use super::super::*;

#[test]
fn validate_wgsl_valid_minimal() {
    let wgsl = "fn sd_custom(p: vec3<f32>, time: f32) -> f32 { return length(p) - 1.0; }";
    assert_eq!(validate_wgsl_snippet(wgsl), ShaderValidation::Valid);
}

#[test]
fn validate_wgsl_valid_multiline() {
    let wgsl = r#"
fn sd_custom(p: vec3<f32>, time: f32) -> f32 {
    let q = abs(p) - vec3<f32>(1.0);
    let d = length(max(q, vec3<f32>(0.0)));
    return d + min(max(q.x, max(q.y, q.z)), 0.0);
}
"#;
    assert_eq!(validate_wgsl_snippet(wgsl), ShaderValidation::Valid);
}

#[test]
fn validate_wgsl_missing_entry_point() {
    let wgsl = "fn some_other_fn(p: vec3<f32>) -> f32 { return 0.0; }";
    assert_eq!(
        validate_wgsl_snippet(wgsl),
        ShaderValidation::MissingEntryPoint
    );
}

#[test]
fn validate_wgsl_unbalanced_braces() {
    let wgsl = "fn sd_custom(p: vec3<f32>, time: f32) -> f32 { return 0.0;";
    assert!(matches!(
        validate_wgsl_snippet(wgsl),
        ShaderValidation::ParseError(_)
    ));
}

#[test]
fn validate_wgsl_disallowed_texture_sample() {
    let wgsl = "fn sd_custom(p: vec3<f32>, time: f32) -> f32 { let c = textureSample(t, s, p.xy); return 0.0; }";
    let result = validate_wgsl_snippet(wgsl);
    assert!(matches!(result, ShaderValidation::ParseError(_)));
    if let ShaderValidation::ParseError(msg) = result {
        assert!(msg.contains("textureSample"));
    }
}

#[test]
fn validate_wgsl_disallowed_texture_load() {
    let wgsl = "fn sd_custom(p: vec3<f32>, time: f32) -> f32 { let c = textureLoad(t, coord, 0); return 0.0; }";
    assert!(matches!(
        validate_wgsl_snippet(wgsl),
        ShaderValidation::ParseError(_)
    ));
}

#[test]
fn validate_wgsl_too_complex() {
    let statements: String = (0..1001)
        .map(|i| format!("    let v{} = 0.0;", i))
        .collect::<Vec<_>>()
        .join("\n");
    let wgsl = format!(
        "fn sd_custom(p: vec3<f32>, time: f32) -> f32 {{\n{}\n    return 0.0;\n}}",
        statements
    );
    let result = validate_wgsl_snippet(&wgsl);
    assert!(matches!(result, ShaderValidation::TooComplex { .. }));
}

#[test]
fn compose_shader_includes_all_parts() {
    let custom = "fn sd_custom(p: vec3<f32>, time: f32) -> f32 { return length(p) - 1.0; }";
    let full = compose_shader(custom);
    assert!(full.contains("struct VertexOutput"));
    assert!(full.contains("sd_custom"));
    assert!(full.contains("fs_main"));
}

#[test]
fn compose_shader_preserves_custom_body() {
    let custom = "fn sd_custom(p: vec3<f32>, time: f32) -> f32 {\n    return sin(time) + length(p);\n}";
    let full = compose_shader(custom);
    assert!(full.contains("sin(time) + length(p)"));
}

#[test]
fn pipeline_default_state() {
    let pipeline = CustomSdfPipeline::default();
    assert!(pipeline.active_source.is_none());
    assert!(pipeline.active_shader_id.is_none());
    assert!(!pipeline.dirty);
}

#[test]
fn pipeline_stage_and_compile() {
    let mut pipeline = CustomSdfPipeline::default();
    pipeline.stage(42, "wgsl source".into());
    assert!(pipeline.dirty);
    assert_eq!(pipeline.active_shader_id, Some(42));
    assert_eq!(pipeline.active_source.as_deref(), Some("wgsl source"));

    pipeline.mark_compiled();
    assert!(!pipeline.dirty);
}

#[test]
fn pipeline_clear() {
    let mut pipeline = CustomSdfPipeline::default();
    pipeline.stage(1, "src".into());
    pipeline.mark_compiled();
    pipeline.clear();
    assert!(pipeline.active_source.is_none());
    assert!(pipeline.active_shader_id.is_none());
    assert!(pipeline.dirty);
}
