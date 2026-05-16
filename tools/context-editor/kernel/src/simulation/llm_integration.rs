//! LLM Integration: Text-to-Voxel/Shader Generation, Naga Validation & Runtime Hot-Reload.
//!
//! Players type natural-language prompts to procedurally generate voxel structures
//! or custom WGSL shader effects. An LLM translates the prompt into either:
//! (a) voxel coordinate + material lists for SVO insertion, or
//! (b) WGSL shader snippets for custom spell/effect SDFs.
//!
//! All generated code is validated before reaching the GPU, and SpacetimeDB
//! persists both the prompt and result for multiplayer sharing.

mod mode;
mod models;
mod resources;
mod shader;
mod systems;
#[cfg(test)]
mod tests;

pub use mode::GenerationMode;
pub use models::{
    CustomShader,
    GeneratedContent,
    GeneratedVoxel,
    GenerationRequest,
    GenerationResult,
    VoxelGenResult,
};
pub use resources::{
    CustomShaderTable,
    GenerationHistory,
    GenerationRateLimiter,
    GenerationRequestQueue,
    GenerationResultQueue,
};
pub use shader::{
    compose_shader,
    validate_voxels,
    validate_wgsl_snippet,
    CustomSdfPipeline,
    ShaderValidation,
    SHADER_FOOTER,
    SHADER_HEADER,
};
pub use systems::LlmIntegrationPlugin;

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
