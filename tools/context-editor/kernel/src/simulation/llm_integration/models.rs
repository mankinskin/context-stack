use crate::multiplayer_backend::PlayerIdentity;

use super::mode::GenerationMode;

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
    /// Entry-point function name, e.g. `sd_custom`.
    pub function_name: String,
    pub active: bool,
}

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
