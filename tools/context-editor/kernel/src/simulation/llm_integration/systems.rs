use bevy::prelude::*;

use crate::multiplayer_backend::PlayerIdentity;

use super::{
    compose_shader,
    validate_wgsl_snippet,
    CustomSdfPipeline,
    CustomShader,
    CustomShaderTable,
    GeneratedContent,
    GenerationHistory,
    GenerationMode,
    GenerationRateLimiter,
    GenerationRequestQueue,
    GenerationResult,
    GenerationResultQueue,
    ShaderValidation,
};

/// System: tick down the generation rate limiter.
fn rate_limit_system(
    time: Res<Time>,
    mut limiter: ResMut<GenerationRateLimiter>,
) {
    limiter.tick(time.delta_secs());
}

/// System: process incoming generation results.
fn process_results_system(
    mut result_queue: ResMut<GenerationResultQueue>,
    mut pipeline: ResMut<CustomSdfPipeline>,
    mut history: ResMut<GenerationHistory>,
    mut shader_table: ResMut<CustomShaderTable>,
) {
    while let Some(result) = result_queue.pop() {
        match result {
            GenerationResult::Voxels(voxels) => {
                let next_id = history.count() as u64 + 1;
                let voxel_count = voxels.len();
                history.push(GeneratedContent {
                    content_id: next_id,
                    creator: PlayerIdentity::local(),
                    prompt: String::new(),
                    mode: GenerationMode::VoxelStructure,
                    result_data: format!(
                        "{{\"voxels_count\":{}}}",
                        voxel_count
                    ),
                    created_tick: 0,
                });
            },
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
            },
            GenerationResult::Error(_msg) => {},
        }
    }
}

/// System: apply staged custom shaders to the render pipeline.
fn shader_hot_reload_system(mut pipeline: ResMut<CustomSdfPipeline>) {
    if pipeline.dirty && pipeline.active_source.is_some() {
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

        match request.mode {
            GenerationMode::VoxelStructure => {},
            GenerationMode::ShaderEffect | GenerationMode::SpellModifier => {},
        }
    }
}

/// Plugin that registers LLM integration resources and systems.
pub struct LlmIntegrationPlugin;

impl Plugin for LlmIntegrationPlugin {
    fn build(
        &self,
        app: &mut App,
    ) {
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
