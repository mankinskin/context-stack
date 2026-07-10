use bevy::prelude::Resource;

use super::{
    models::GeneratedVoxel,
    MAX_GENERATED_VOXELS,
    MAX_GENERATION_RADIUS,
    MAX_SHADER_INSTRUCTIONS,
};

/// Validate and convert raw LLM voxel JSON into bounded `GeneratedVoxel`s.
pub fn validate_voxels(
    voxels: &[[i64; 4]],
    player_pos: (f32, f32, f32),
) -> Result<Vec<GeneratedVoxel>, String> {
    if voxels.len() > MAX_GENERATED_VOXELS {
        return Err(format!(
            "Too many voxels: {} > {}",
            voxels.len(),
            MAX_GENERATED_VOXELS
        ));
    }

    let mut result = Vec::with_capacity(voxels.len());
    for entry in voxels {
        let (x, y, z, color) = (entry[0], entry[1], entry[2], entry[3]);

        if x < -64 || x > 64 || y < -64 || y > 64 || z < -64 || z > 64 {
            return Err(format!(
                "Voxel coordinate ({}, {}, {}) out of range [-64, 64]",
                x, y, z
            ));
        }

        let dx = x as f32 - player_pos.0;
        let dy = y as f32 - player_pos.1;
        let dz = z as f32 - player_pos.2;
        let dist = (dx * dx + dy * dy + dz * dz).sqrt();
        if dist > MAX_GENERATION_RADIUS {
            return Err(format!(
                "Voxel at ({}, {}, {}) is {:.1} units from player (max {})",
                x, y, z, dist, MAX_GENERATION_RADIUS
            ));
        }

        result.push(GeneratedVoxel {
            x: x as i32,
            y: y as i32,
            z: z as i32,
            color: color as u32,
        });
    }
    Ok(result)
}

/// Result of WGSL shader validation.
#[derive(Clone, Debug, PartialEq)]
pub enum ShaderValidation {
    Valid,
    ParseError(String),
    MissingEntryPoint,
    TooComplex { instruction_count: usize },
}

fn find_sd_custom_body(
    trimmed: &str
) -> Result<(&str, &str), ShaderValidation> {
    let Some(fn_start) = trimmed.find("fn sd_custom") else {
        return Err(ShaderValidation::MissingEntryPoint);
    };
    let after_fn = &trimmed[fn_start..];
    let Some(body_start) = after_fn.find('{') else {
        return Err(ShaderValidation::ParseError(
            "Missing function body".into(),
        ));
    };

    let body_region = &after_fn[body_start..];
    let Some(body_end) = find_balanced_body_end(body_region) else {
        return Err(ShaderValidation::ParseError(
            "Unbalanced braces in function body".into(),
        ));
    };

    Ok((&after_fn[..body_start], &body_region[1..body_end]))
}

fn find_balanced_body_end(body_region: &str) -> Option<usize> {
    let mut depth: i32 = 0;

    for (index, ch) in body_region.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            },
            _ => {},
        }
    }

    None
}

fn validate_instruction_budget(body: &str) -> Result<(), ShaderValidation> {
    let instruction_count = body.chars().filter(|&ch| ch == ';').count();
    if instruction_count > MAX_SHADER_INSTRUCTIONS {
        return Err(ShaderValidation::TooComplex { instruction_count });
    }
    Ok(())
}

fn validate_disallowed_keywords(trimmed: &str) -> Result<(), ShaderValidation> {
    let disallowed = [
        "textureSample",
        "textureLoad",
        "textureStore",
        "storageBarrier",
        "workgroupBarrier",
    ];

    for keyword in &disallowed {
        if trimmed.contains(keyword) {
            return Err(ShaderValidation::ParseError(format!(
                "Disallowed construct: {}",
                keyword
            )));
        }
    }

    Ok(())
}

fn validate_signature(sig_region: &str) -> Result<(), ShaderValidation> {
    if sig_region.contains("vec3<f32>") && sig_region.contains("f32") {
        Ok(())
    } else {
        Err(ShaderValidation::ParseError(
            "Function signature must accept (p: vec3<f32>, time: f32)".into(),
        ))
    }
}

/// Validate a WGSL snippet intended for custom SDF rendering.
pub fn validate_wgsl_snippet(wgsl: &str) -> ShaderValidation {
    let trimmed = wgsl.trim();

    let (sig_region, body) = match find_sd_custom_body(trimmed) {
        Ok(parts) => parts,
        Err(validation) => return validation,
    };
    if let Err(validation) = validate_instruction_budget(body) {
        return validation;
    }
    if let Err(validation) = validate_disallowed_keywords(trimmed) {
        return validation;
    }
    if let Err(validation) = validate_signature(sig_region) {
        return validation;
    }

    ShaderValidation::Valid
}

/// Shader template components for composing a full ray-marching pipeline.
pub const SHADER_HEADER: &str = "\
// === Auto-generated shader header ===\n\
struct VertexOutput {\n\
    @builtin(position) pos: vec4<f32>,\n\
    @location(0) uv: vec2<f32>,\n\
};\n\
\n\
@group(0) @binding(0) var<uniform> time: f32;\n";

pub const SHADER_FOOTER: &str = "\
// === Ray-march loop ===\n\
@fragment\n\
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {\n\
    let ro = vec3<f32>(0.0, 0.0, -3.0);\n\
    let rd = normalize(vec3<f32>(in.uv, 1.0));\n\
    var t = 0.0;\n\
    for (var i = 0; i < 64; i = i + 1) {\n\
        let p = ro + rd * t;\n\
        let d = sd_custom(p, time);\n\
        if (d < 0.001) { break; }\n\
        t = t + d;\n\
        if (t > 100.0) { break; }\n\
    }\n\
    let brightness = 1.0 - t / 100.0;\n\
    return vec4<f32>(brightness, brightness, brightness, 1.0);\n\
}\n";

/// Compose a full shader from the header, a custom SDF function, and the footer.
pub fn compose_shader(custom_sdf: &str) -> String {
    format!("{}\n{}\n{}", SHADER_HEADER, custom_sdf, SHADER_FOOTER)
}

/// A custom SDF pipeline slot that can be hot-reloaded at runtime.
#[derive(Resource)]
pub struct CustomSdfPipeline {
    /// The currently active WGSL source (composed: header + custom + footer).
    pub active_source: Option<String>,
    /// Shader ID from the DB (for tracking which shader is loaded).
    pub active_shader_id: Option<u64>,
    /// Whether the pipeline needs recompilation.
    pub dirty: bool,
}

impl Default for CustomSdfPipeline {
    fn default() -> Self {
        Self {
            active_source: None,
            active_shader_id: None,
            dirty: false,
        }
    }
}

impl CustomSdfPipeline {
    /// Stage a new shader for compilation on the next frame.
    pub fn stage(
        &mut self,
        shader_id: u64,
        full_wgsl: String,
    ) {
        self.active_source = Some(full_wgsl);
        self.active_shader_id = Some(shader_id);
        self.dirty = true;
    }

    /// Mark the pipeline as up-to-date after GPU compilation.
    pub fn mark_compiled(&mut self) {
        self.dirty = false;
    }

    /// Clear the active shader (revert to default rendering).
    pub fn clear(&mut self) {
        self.active_source = None;
        self.active_shader_id = None;
        self.dirty = true;
    }
}
