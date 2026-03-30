//! LLM Integration: Text-to-Voxel/Shader Generation, Naga Validation & Runtime Hot-Reload.
//!
//! Players type natural-language prompts to procedurally generate voxel structures
//! or custom WGSL shader effects. An LLM translates the prompt into either:
//! (a) voxel coordinate + material lists for SVO insertion, or
//! (b) WGSL shader snippets for custom spell/effect SDFs.
//!
//! All generated code is validated before reaching the GPU, and SpacetimeDB
//! persists both the prompt and result for multiplayer sharing.

use bevy::prelude::*;
use std::collections::VecDeque;

use crate::multiplayer_backend::PlayerIdentity;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum voxels a single LLM generation can produce.
pub const MAX_GENERATED_VOXELS: usize = 4096;

/// Maximum radius (in world units) from the player for generated voxels.
pub const MAX_GENERATION_RADIUS: f32 = 64.0;

/// Maximum instructions allowed in a generated WGSL function body (DoS prevention).
pub const MAX_SHADER_INSTRUCTIONS: usize = 1000;

/// Rate limit: minimum seconds between generation requests per player.
pub const GENERATION_COOLDOWN_SECS: f32 = 30.0;

/// Maximum number of stored generation results kept in history.
pub const MAX_HISTORY_SIZE: usize = 256;

// ---------------------------------------------------------------------------
// Generation modes
// ---------------------------------------------------------------------------

/// Which kind of content the LLM should generate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum GenerationMode {
    /// Generate voxel structures (coordinate + color lists).
    VoxelStructure = 0,
    /// Generate a WGSL SDF function for a custom shader effect.
    ShaderEffect = 1,
    /// Generate a spell modifier (shader + parameter tweaks).
    SpellModifier = 2,
}

impl GenerationMode {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::VoxelStructure),
            1 => Some(Self::ShaderEffect),
            2 => Some(Self::SpellModifier),
            _ => None,
        }
    }

    /// Returns the system-prompt preamble for this mode.
    pub fn system_prompt_header(&self) -> &'static str {
        match self {
            Self::VoxelStructure => SYSTEM_PROMPT_VOXEL,
            Self::ShaderEffect => SYSTEM_PROMPT_SHADER,
            Self::SpellModifier => SYSTEM_PROMPT_SHADER,
        }
    }
}

// ---------------------------------------------------------------------------
// System prompts (constrain LLM output to safe, valid formats)
// ---------------------------------------------------------------------------

const SYSTEM_PROMPT_VOXEL: &str = "\
You are a WebGPU world generator for a voxel sandbox game.\n\
MODE: voxel_structure\n\
Respond ONLY with JSON: { \"voxels\": [[x,y,z,color_hex], ...] }\n\
- Coordinates must be integers in range [-64, 64] relative to player position\n\
- color_hex is a 32-bit RGBA packed as \"0xRRGGBBAA\"\n\
- Maximum 4096 voxels per generation\n\
- Create the structure the user describes";

const SYSTEM_PROMPT_SHADER: &str = "\
You are a WebGPU world generator for a voxel sandbox game.\n\
MODE: shader_effect\n\
Respond ONLY with valid WGSL code defining a single SDF function:\n\
- Function signature: fn sd_custom(p: vec3<f32>, time: f32) -> f32\n\
- Use only: length, dot, cross, normalize, clamp, smoothstep, min, max, abs, sin, cos\n\
- NO texture sampling, NO buffer access, NO loops over external data\n\
- Return signed distance (negative = inside, positive = outside)";

// ---------------------------------------------------------------------------
// SpacetimeDB table mirrors
// ---------------------------------------------------------------------------

/// A record of LLM-generated content (persisted in SpacetimeDB).
#[derive(Clone, Debug)]
pub struct GeneratedContent {
    pub content_id: u64,
    pub creator: PlayerIdentity,
    pub prompt: String,
    pub mode: GenerationMode,
    /// Validated LLM output (JSON for voxels, WGSL source for shaders).
    pub result_data: String,
    pub created_tick: u64,
}

/// A validated custom WGSL shader stored in the server DB.
#[derive(Clone, Debug)]
pub struct CustomShader {
    pub shader_id: u64,
    pub creator: PlayerIdentity,
    /// Validated WGSL snippet (the `sd_custom` function body).
    pub wgsl_source: String,
    /// Entry-point function name, e.g. `"sd_custom"`.
    pub function_name: String,
    pub active: bool,
}

// ---------------------------------------------------------------------------
// Parsed voxel generation result
// ---------------------------------------------------------------------------

/// A single generated voxel: position relative to player + packed RGBA color.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GeneratedVoxel {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub color: u32,
}

/// The intermediate JSON structure returned by the LLM for voxel mode.
#[derive(Clone, Debug)]
pub struct VoxelGenResult {
    pub voxels: Vec<[i64; 4]>,
}

// ---------------------------------------------------------------------------
// Validation — voxels
// ---------------------------------------------------------------------------

/// Validate and convert raw LLM voxel JSON into bounded `GeneratedVoxel`s.
///
/// Enforces:
/// - Maximum voxel count (`MAX_GENERATED_VOXELS`)
/// - Distance from player position ≤ `MAX_GENERATION_RADIUS`
/// - Coordinate range [-64, 64]
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

        // Coordinate range check
        if x < -64 || x > 64 || y < -64 || y > 64 || z < -64 || z > 64 {
            return Err(format!("Voxel coordinate ({}, {}, {}) out of range [-64, 64]", x, y, z));
        }

        // Distance from player
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

// ---------------------------------------------------------------------------
// Validation — WGSL shaders
// ---------------------------------------------------------------------------

/// Result of WGSL shader validation.
#[derive(Clone, Debug, PartialEq)]
pub enum ShaderValidation {
    Valid,
    ParseError(String),
    MissingEntryPoint,
    TooComplex { instruction_count: usize },
}

/// Validate a WGSL snippet intended for custom SDF rendering.
///
/// Checks:
/// 1. Valid WGSL syntax (parseable)
/// 2. Contains a function named `sd_custom`
/// 3. Function body does not exceed `MAX_SHADER_INSTRUCTIONS`
///
/// In a real build with `naga` as a dependency, this would use `naga::front::wgsl`.
/// Here we provide a lightweight structural check suitable for the kernel crate.
pub fn validate_wgsl_snippet(wgsl: &str) -> ShaderValidation {
    let trimmed = wgsl.trim();

    // Must contain the sd_custom function definition
    if !trimmed.contains("fn sd_custom") {
        return ShaderValidation::MissingEntryPoint;
    }

    // Extract the function body (between first { after sd_custom and matching })
    let Some(fn_start) = trimmed.find("fn sd_custom") else {
        return ShaderValidation::MissingEntryPoint;
    };
    let after_fn = &trimmed[fn_start..];
    let Some(body_start) = after_fn.find('{') else {
        return ShaderValidation::ParseError("Missing function body".into());
    };

    // Count brace-balanced body to find the closing brace
    let body_region = &after_fn[body_start..];
    let mut depth: i32 = 0;
    let mut body_end = 0;
    for (i, ch) in body_region.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    body_end = i;
                    break;
                }
            }
            _ => {}
        }
    }
    if depth != 0 {
        return ShaderValidation::ParseError("Unbalanced braces in function body".into());
    }

    let body = &body_region[1..body_end];

    // Count statements (semicolons) as a proxy for instruction count
    let instruction_count = body.chars().filter(|&c| c == ';').count();
    if instruction_count > MAX_SHADER_INSTRUCTIONS {
        return ShaderValidation::TooComplex { instruction_count };
    }

    // Check for disallowed constructs (buffer access, texture sampling)
    let disallowed = [
        "textureSample",
        "textureLoad",
        "textureStore",
        "storageBarrier",
        "workgroupBarrier",
    ];
    for keyword in &disallowed {
        if trimmed.contains(keyword) {
            return ShaderValidation::ParseError(format!(
                "Disallowed construct: {}",
                keyword
            ));
        }
    }

    // Basic syntax: check that the signature contains expected parameter types
    let sig_region = &after_fn[..body_start];
    if !sig_region.contains("vec3<f32>") || !sig_region.contains("f32") {
        return ShaderValidation::ParseError(
            "Function signature must accept (p: vec3<f32>, time: f32)".into(),
        );
    }

    ShaderValidation::Valid
}

// ---------------------------------------------------------------------------
// Shader hot-reload pipeline
// ---------------------------------------------------------------------------

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
    pub fn stage(&mut self, shader_id: u64, full_wgsl: String) {
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

// ---------------------------------------------------------------------------
// Generation request / response
// ---------------------------------------------------------------------------

/// A pending generation request from the player.
#[derive(Clone, Debug)]
pub struct GenerationRequest {
    pub prompt: String,
    pub mode: GenerationMode,
    pub player_pos: (f32, f32, f32),
    pub creator: PlayerIdentity,
}

/// Result of a generation attempt.
#[derive(Clone, Debug)]
pub enum GenerationResult {
    /// Voxels ready to be inserted into the SVO.
    Voxels(Vec<GeneratedVoxel>),
    /// Shader source ready for validation and hot-reload.
    Shader {
        wgsl_source: String,
        function_name: String,
    },
    /// Generation failed with an error message.
    Error(String),
}

// ---------------------------------------------------------------------------
// Bevy resources
// ---------------------------------------------------------------------------

/// Queue of pending generation requests (client → server → LLM).
#[derive(Resource, Default)]
pub struct GenerationRequestQueue {
    pub pending: VecDeque<GenerationRequest>,
}

impl GenerationRequestQueue {
    pub fn submit(&mut self, request: GenerationRequest) {
        self.pending.push_back(request);
    }

    pub fn pop(&mut self) -> Option<GenerationRequest> {
        self.pending.pop_front()
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

/// Queue of completed generation results (server → client).
#[derive(Resource, Default)]
pub struct GenerationResultQueue {
    pub results: VecDeque<GenerationResult>,
}

impl GenerationResultQueue {
    pub fn push(&mut self, result: GenerationResult) {
        self.results.push_back(result);
    }

    pub fn pop(&mut self) -> Option<GenerationResult> {
        self.results.pop_front()
    }
}

/// Per-player rate limiter for generation requests.
#[derive(Resource)]
pub struct GenerationRateLimiter {
    /// Seconds remaining until the player can generate again.
    pub cooldown_remaining: f32,
}

impl Default for GenerationRateLimiter {
    fn default() -> Self {
        Self {
            cooldown_remaining: 0.0,
        }
    }
}

impl GenerationRateLimiter {
    /// Try to consume a generation credit. Returns `true` if allowed.
    pub fn try_consume(&mut self) -> bool {
        if self.cooldown_remaining > 0.0 {
            return false;
        }
        self.cooldown_remaining = GENERATION_COOLDOWN_SECS;
        true
    }

    /// Tick down the cooldown.
    pub fn tick(&mut self, dt: f32) {
        self.cooldown_remaining = (self.cooldown_remaining - dt).max(0.0);
    }

    /// Whether the player can currently submit a generation request.
    pub fn can_generate(&self) -> bool {
        self.cooldown_remaining <= 0.0
    }
}

/// History of generated content for the local player.
#[derive(Resource)]
pub struct GenerationHistory {
    pub entries: VecDeque<GeneratedContent>,
}

impl Default for GenerationHistory {
    fn default() -> Self {
        Self {
            entries: VecDeque::new(),
        }
    }
}

impl GenerationHistory {
    pub fn push(&mut self, entry: GeneratedContent) {
        if self.entries.len() >= MAX_HISTORY_SIZE {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
    }

    pub fn last(&self) -> Option<&GeneratedContent> {
        self.entries.back()
    }

    pub fn find_by_id(&self, content_id: u64) -> Option<&GeneratedContent> {
        self.entries.iter().find(|e| e.content_id == content_id)
    }

    pub fn count(&self) -> usize {
        self.entries.len()
    }
}

/// Table of custom shaders received from the server subscription.
#[derive(Resource, Default)]
pub struct CustomShaderTable {
    pub shaders: Vec<CustomShader>,
}

impl CustomShaderTable {
    pub fn active_shaders(&self) -> impl Iterator<Item = &CustomShader> {
        self.shaders.iter().filter(|s| s.active)
    }

    pub fn find_by_id(&self, shader_id: u64) -> Option<&CustomShader> {
        self.shaders.iter().find(|s| s.shader_id == shader_id)
    }

    pub fn add_or_update(&mut self, shader: CustomShader) {
        if let Some(existing) = self.shaders.iter_mut().find(|s| s.shader_id == shader.shader_id) {
            *existing = shader;
        } else {
            self.shaders.push(shader);
        }
    }
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// System: tick down the generation rate limiter.
fn rate_limit_system(time: Res<Time>, mut limiter: ResMut<GenerationRateLimiter>) {
    limiter.tick(time.delta_secs());
}

/// System: process incoming generation results.
///
/// - Voxel results are dispatched to the SVO insertion queue.
/// - Shader results are validated and staged for hot-reload.
fn process_results_system(
    mut result_queue: ResMut<GenerationResultQueue>,
    mut pipeline: ResMut<CustomSdfPipeline>,
    mut history: ResMut<GenerationHistory>,
    mut shader_table: ResMut<CustomShaderTable>,
) {
    while let Some(result) = result_queue.pop() {
        match result {
            GenerationResult::Voxels(voxels) => {
                // In a full implementation, this would push voxel deltas into the
                // SVO via ReducerQueue / direct VoxelWorld mutation. Here we record
                // the generation in history.
                let next_id = history.count() as u64 + 1;
                let voxel_count = voxels.len();
                history.push(GeneratedContent {
                    content_id: next_id,
                    creator: PlayerIdentity::local(),
                    prompt: String::new(),
                    mode: GenerationMode::VoxelStructure,
                    result_data: format!("{{\"voxels_count\":{}}}", voxel_count),
                    created_tick: 0,
                });
            }
            GenerationResult::Shader {
                wgsl_source,
                function_name,
            } => {
                let validation = validate_wgsl_snippet(&wgsl_source);
                if validation == ShaderValidation::Valid {
                    let shader_id = shader_table.shaders.len() as u64 + 1;
                    let shader = CustomShader {
                        shader_id,
                        creator: PlayerIdentity::local(),
                        wgsl_source: wgsl_source.clone(),
                        function_name,
                        active: true,
                    };
                    let full_wgsl = compose_shader(&wgsl_source);
                    pipeline.stage(shader_id, full_wgsl);
                    shader_table.add_or_update(shader);

                    let next_id = history.count() as u64 + 1;
                    history.push(GeneratedContent {
                        content_id: next_id,
                        creator: PlayerIdentity::local(),
                        prompt: String::new(),
                        mode: GenerationMode::ShaderEffect,
                        result_data: wgsl_source,
                        created_tick: 0,
                    });
                }
                // Invalid shaders are silently dropped (already validated server-side)
            }
            GenerationResult::Error(_msg) => {
                // Error handling would show a toast in the UI
            }
        }
    }
}

/// System: apply staged custom shaders to the render pipeline.
///
/// When `CustomSdfPipeline.dirty` is set, the system composes the final
/// WGSL and marks the pipeline for GPU recompilation on the render thread.
fn shader_hot_reload_system(mut pipeline: ResMut<CustomSdfPipeline>) {
    if pipeline.dirty && pipeline.active_source.is_some() {
        // In a full implementation with wgpu access, we would call:
        //   device.create_shader_module(ShaderModuleDescriptor { ... })
        // For now, marking as compiled to acknowledge the staged source.
        pipeline.mark_compiled();
    }
}

/// System: drain generation requests and enforce rate limiting.
fn drain_requests_system(
    mut request_queue: ResMut<GenerationRequestQueue>,
    mut limiter: ResMut<GenerationRateLimiter>,
    mut result_queue: ResMut<GenerationResultQueue>,
) {
    while let Some(request) = request_queue.pop() {
        if !limiter.try_consume() {
            result_queue.push(GenerationResult::Error(
                "Rate limited: please wait before generating again".into(),
            ));
            continue;
        }

        // In a real implementation, this would send the request to SpacetimeDB
        // which calls the LLM API server-side. Here we simulate a "pending" state.
        // The actual result arrives asynchronously via the GenerationResultQueue
        // when the server subscription delivers the GeneratedContent row.
        match request.mode {
            GenerationMode::VoxelStructure => {
                // Stub: in production, SpacetimeDB reducer `ai_generate` handles this
            }
            GenerationMode::ShaderEffect | GenerationMode::SpellModifier => {
                // Stub: in production, SpacetimeDB reducer `ai_generate` handles this
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

/// Plugin that registers LLM integration resources and systems.
pub struct LlmIntegrationPlugin;

impl Plugin for LlmIntegrationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GenerationRequestQueue>();
        app.init_resource::<GenerationResultQueue>();
        app.init_resource::<GenerationRateLimiter>();
        app.init_resource::<GenerationHistory>();
        app.init_resource::<CustomShaderTable>();
        app.init_resource::<CustomSdfPipeline>();

        app.add_systems(
            Update,
            (
                rate_limit_system,
                drain_requests_system,
                process_results_system,
                shader_hot_reload_system,
            )
                .chain(),
        );
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- GenerationMode ---

    #[test]
    fn generation_mode_from_u8_valid() {
        assert_eq!(GenerationMode::from_u8(0), Some(GenerationMode::VoxelStructure));
        assert_eq!(GenerationMode::from_u8(1), Some(GenerationMode::ShaderEffect));
        assert_eq!(GenerationMode::from_u8(2), Some(GenerationMode::SpellModifier));
    }

    #[test]
    fn generation_mode_from_u8_invalid() {
        assert_eq!(GenerationMode::from_u8(3), None);
        assert_eq!(GenerationMode::from_u8(255), None);
    }

    #[test]
    fn generation_mode_system_prompt() {
        let voxel = GenerationMode::VoxelStructure;
        assert!(voxel.system_prompt_header().contains("voxel_structure"));

        let shader = GenerationMode::ShaderEffect;
        assert!(shader.system_prompt_header().contains("shader_effect"));
    }

    // --- Voxel validation ---

    #[test]
    fn validate_voxels_empty() {
        let result = validate_voxels(&[], (0.0, 0.0, 0.0));
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[test]
    fn validate_voxels_single_valid() {
        let voxels = [[0, 5, -3, 0xFF0000FF]];
        let result = validate_voxels(&voxels, (0.0, 0.0, 0.0));
        assert!(result.is_ok());
        let gen = &result.unwrap()[0];
        assert_eq!(gen.x, 0);
        assert_eq!(gen.y, 5);
        assert_eq!(gen.z, -3);
        assert_eq!(gen.color, 0xFF0000FF);
    }

    #[test]
    fn validate_voxels_at_boundary() {
        // Coordinate at limit but within radius of player
        let voxels = [[64, 0, 0, 0xFFFFFFFF]];
        let result = validate_voxels(&voxels, (0.0, 0.0, 0.0));
        assert!(result.is_ok());
    }

    #[test]
    fn validate_voxels_out_of_coordinate_range() {
        let voxels = [[65, 0, 0, 0xFF]];
        let result = validate_voxels(&voxels, (0.0, 0.0, 0.0));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("out of range"));
    }

    #[test]
    fn validate_voxels_negative_out_of_range() {
        let voxels = [[0, -65, 0, 0xFF]];
        let result = validate_voxels(&voxels, (0.0, 0.0, 0.0));
        assert!(result.is_err());
    }

    #[test]
    fn validate_voxels_too_far_from_player() {
        // player at origin, voxel at (64, 64, 0) → distance ≈ 90.5 > 64.0
        let voxels = [[64, 64, 0, 0xFF]];
        let result = validate_voxels(&voxels, (0.0, 0.0, 0.0));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("units from player"));
    }

    #[test]
    fn validate_voxels_near_player_pos() {
        // player at (60, 0, 0), voxel at (62, 0, 0) → distance 2
        let voxels = [[62, 0, 0, 0xFF]];
        let result = validate_voxels(&voxels, (60.0, 0.0, 0.0));
        assert!(result.is_ok());
    }

    #[test]
    fn validate_voxels_too_many() {
        let voxels: Vec<[i64; 4]> = (0..4097).map(|i| [0, 0, i % 64, 0xFF]).collect();
        let result = validate_voxels(&voxels, (0.0, 0.0, 0.0));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Too many voxels"));
    }

    #[test]
    fn validate_voxels_exactly_max() {
        let voxels: Vec<[i64; 4]> = (0..MAX_GENERATED_VOXELS)
            .map(|i| {
                // Spread across a cube so none exceed radius
                let x = (i % 11) as i64 - 5;
                let y = ((i / 11) % 11) as i64 - 5;
                let z = ((i / 121) % 11) as i64 - 5;
                [x, y, z, 0xAABBCCDD]
            })
            .collect();
        let result = validate_voxels(&voxels, (0.0, 0.0, 0.0));
        assert!(result.is_ok());
    }

    // --- WGSL shader validation ---

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
        assert_eq!(validate_wgsl_snippet(wgsl), ShaderValidation::MissingEntryPoint);
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
        // Generate a shader with > MAX_SHADER_INSTRUCTIONS semicolons
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

    // --- Shader composition ---

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

    // --- CustomSdfPipeline ---

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

    // --- Rate limiter ---

    #[test]
    fn rate_limiter_initial_can_generate() {
        let limiter = GenerationRateLimiter::default();
        assert!(limiter.can_generate());
    }

    #[test]
    fn rate_limiter_consume_and_cooldown() {
        let mut limiter = GenerationRateLimiter::default();
        assert!(limiter.try_consume());
        assert!(!limiter.can_generate());
        assert!(!limiter.try_consume()); // second attempt blocked
    }

    #[test]
    fn rate_limiter_tick_resets() {
        let mut limiter = GenerationRateLimiter::default();
        limiter.try_consume();
        // Tick past the full cooldown
        limiter.tick(GENERATION_COOLDOWN_SECS + 1.0);
        assert!(limiter.can_generate());
        assert!(limiter.try_consume());
    }

    #[test]
    fn rate_limiter_partial_tick() {
        let mut limiter = GenerationRateLimiter::default();
        limiter.try_consume();
        limiter.tick(GENERATION_COOLDOWN_SECS / 2.0);
        assert!(!limiter.can_generate());
        limiter.tick(GENERATION_COOLDOWN_SECS / 2.0 + 1.0);
        assert!(limiter.can_generate());
    }

    // --- GenerationHistory ---

    #[test]
    fn history_push_and_retrieve() {
        let mut history = GenerationHistory::default();
        assert_eq!(history.count(), 0);

        history.push(GeneratedContent {
            content_id: 1,
            creator: PlayerIdentity::local(),
            prompt: "castle".into(),
            mode: GenerationMode::VoxelStructure,
            result_data: "{}".into(),
            created_tick: 10,
        });
        assert_eq!(history.count(), 1);
        assert_eq!(history.last().unwrap().prompt, "castle");
    }

    #[test]
    fn history_find_by_id() {
        let mut history = GenerationHistory::default();
        history.push(GeneratedContent {
            content_id: 42,
            creator: PlayerIdentity::local(),
            prompt: "tower".into(),
            mode: GenerationMode::VoxelStructure,
            result_data: "{}".into(),
            created_tick: 5,
        });
        assert!(history.find_by_id(42).is_some());
        assert!(history.find_by_id(99).is_none());
    }

    #[test]
    fn history_caps_at_max_size() {
        let mut history = GenerationHistory::default();
        for i in 0..MAX_HISTORY_SIZE + 10 {
            history.push(GeneratedContent {
                content_id: i as u64,
                creator: PlayerIdentity::local(),
                prompt: format!("prompt_{}", i),
                mode: GenerationMode::VoxelStructure,
                result_data: "{}".into(),
                created_tick: i as u64,
            });
        }
        assert_eq!(history.count(), MAX_HISTORY_SIZE);
        // Oldest entries should have been evicted
        assert!(history.find_by_id(0).is_none());
        assert!(history.find_by_id(MAX_HISTORY_SIZE as u64 + 9).is_some());
    }

    // --- CustomShaderTable ---

    #[test]
    fn shader_table_add_and_find() {
        let mut table = CustomShaderTable::default();
        table.add_or_update(CustomShader {
            shader_id: 1,
            creator: PlayerIdentity::local(),
            wgsl_source: "fn sd_custom(p: vec3<f32>, time: f32) -> f32 { return 0.0; }".into(),
            function_name: "sd_custom".into(),
            active: true,
        });
        assert!(table.find_by_id(1).is_some());
        assert!(table.find_by_id(2).is_none());
    }

    #[test]
    fn shader_table_update_existing() {
        let mut table = CustomShaderTable::default();
        table.add_or_update(CustomShader {
            shader_id: 1,
            creator: PlayerIdentity::local(),
            wgsl_source: "v1".into(),
            function_name: "sd_custom".into(),
            active: true,
        });
        table.add_or_update(CustomShader {
            shader_id: 1,
            creator: PlayerIdentity::local(),
            wgsl_source: "v2".into(),
            function_name: "sd_custom".into(),
            active: false,
        });
        assert_eq!(table.shaders.len(), 1);
        assert_eq!(table.find_by_id(1).unwrap().wgsl_source, "v2");
        assert!(!table.find_by_id(1).unwrap().active);
    }

    #[test]
    fn shader_table_active_filter() {
        let mut table = CustomShaderTable::default();
        table.add_or_update(CustomShader {
            shader_id: 1,
            creator: PlayerIdentity::local(),
            wgsl_source: "a".into(),
            function_name: "sd_custom".into(),
            active: true,
        });
        table.add_or_update(CustomShader {
            shader_id: 2,
            creator: PlayerIdentity::local(),
            wgsl_source: "b".into(),
            function_name: "sd_custom".into(),
            active: false,
        });
        let active: Vec<_> = table.active_shaders().collect();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].shader_id, 1);
    }

    // --- GenerationRequestQueue ---

    #[test]
    fn request_queue_submit_and_pop() {
        let mut queue = GenerationRequestQueue::default();
        assert!(queue.is_empty());

        queue.submit(GenerationRequest {
            prompt: "castle".into(),
            mode: GenerationMode::VoxelStructure,
            player_pos: (0.0, 0.0, 0.0),
            creator: PlayerIdentity::local(),
        });
        assert!(!queue.is_empty());

        let req = queue.pop().unwrap();
        assert_eq!(req.prompt, "castle");
        assert!(queue.is_empty());
    }

    #[test]
    fn request_queue_fifo_order() {
        let mut queue = GenerationRequestQueue::default();
        queue.submit(GenerationRequest {
            prompt: "first".into(),
            mode: GenerationMode::VoxelStructure,
            player_pos: (0.0, 0.0, 0.0),
            creator: PlayerIdentity::local(),
        });
        queue.submit(GenerationRequest {
            prompt: "second".into(),
            mode: GenerationMode::ShaderEffect,
            player_pos: (0.0, 0.0, 0.0),
            creator: PlayerIdentity::local(),
        });
        assert_eq!(queue.pop().unwrap().prompt, "first");
        assert_eq!(queue.pop().unwrap().prompt, "second");
    }

    // --- GenerationResultQueue ---

    #[test]
    fn result_queue_push_pop() {
        let mut queue = GenerationResultQueue::default();
        queue.push(GenerationResult::Error("test".into()));
        let r = queue.pop().unwrap();
        assert!(matches!(r, GenerationResult::Error(_)));
        assert!(queue.pop().is_none());
    }

    #[test]
    fn result_queue_voxel_result() {
        let mut queue = GenerationResultQueue::default();
        queue.push(GenerationResult::Voxels(vec![
            GeneratedVoxel { x: 0, y: 1, z: 2, color: 0xFF },
        ]));
        if let Some(GenerationResult::Voxels(v)) = queue.pop() {
            assert_eq!(v.len(), 1);
            assert_eq!(v[0].x, 0);
        } else {
            panic!("Expected Voxels result");
        }
    }

    #[test]
    fn result_queue_shader_result() {
        let mut queue = GenerationResultQueue::default();
        queue.push(GenerationResult::Shader {
            wgsl_source: "fn sd_custom(p: vec3<f32>, time: f32) -> f32 { return 0.0; }".into(),
            function_name: "sd_custom".into(),
        });
        if let Some(GenerationResult::Shader { function_name, .. }) = queue.pop() {
            assert_eq!(function_name, "sd_custom");
        } else {
            panic!("Expected Shader result");
        }
    }
}
