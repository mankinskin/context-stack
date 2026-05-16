use crate::multiplayer_backend::PlayerIdentity;

use super::super::*;

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
    assert!(!limiter.try_consume());
}

#[test]
fn rate_limiter_tick_resets() {
    let mut limiter = GenerationRateLimiter::default();
    limiter.try_consume();
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
    assert!(history.find_by_id(0).is_none());
    assert!(history.find_by_id(MAX_HISTORY_SIZE as u64 + 9).is_some());
}

#[test]
fn shader_table_add_and_find() {
    let mut table = CustomShaderTable::default();
    table.add_or_update(CustomShader {
        shader_id: 1,
        creator: PlayerIdentity::local(),
        wgsl_source:
            "fn sd_custom(p: vec3<f32>, time: f32) -> f32 { return 0.0; }"
                .into(),
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

#[test]
fn result_queue_push_pop() {
    let mut queue = GenerationResultQueue::default();
    queue.push(GenerationResult::Error("test".into()));
    let result = queue.pop().unwrap();
    assert!(matches!(result, GenerationResult::Error(_)));
    assert!(queue.pop().is_none());
}

#[test]
fn result_queue_voxel_result() {
    let mut queue = GenerationResultQueue::default();
    queue.push(GenerationResult::Voxels(vec![GeneratedVoxel {
        x: 0,
        y: 1,
        z: 2,
        color: 0xFF,
    }]));
    if let Some(GenerationResult::Voxels(voxels)) = queue.pop() {
        assert_eq!(voxels.len(), 1);
        assert_eq!(voxels[0].x, 0);
    } else {
        panic!("Expected Voxels result");
    }
}

#[test]
fn result_queue_shader_result() {
    let mut queue = GenerationResultQueue::default();
    queue.push(GenerationResult::Shader {
        wgsl_source:
            "fn sd_custom(p: vec3<f32>, time: f32) -> f32 { return 0.0; }"
                .into(),
        function_name: "sd_custom".into(),
    });
    if let Some(GenerationResult::Shader { function_name, .. }) =
        queue.pop()
    {
        assert_eq!(function_name, "sd_custom");
    } else {
        panic!("Expected Shader result");
    }
}
