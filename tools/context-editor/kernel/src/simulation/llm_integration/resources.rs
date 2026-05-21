use bevy::prelude::Resource;
use std::collections::VecDeque;

use super::{
    models::{
        CustomShader,
        GeneratedContent,
        GenerationRequest,
        GenerationResult,
    },
    GENERATION_COOLDOWN_SECS,
    MAX_HISTORY_SIZE,
};

/// Queue of pending generation requests (client -> server -> LLM).
#[derive(Resource, Default)]
pub struct GenerationRequestQueue {
    pub pending: VecDeque<GenerationRequest>,
}

impl GenerationRequestQueue {
    pub fn submit(
        &mut self,
        request: GenerationRequest,
    ) {
        self.pending.push_back(request);
    }

    pub fn pop(&mut self) -> Option<GenerationRequest> {
        self.pending.pop_front()
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

/// Queue of completed generation results (server -> client).
#[derive(Resource, Default)]
pub struct GenerationResultQueue {
    pub results: VecDeque<GenerationResult>,
}

impl GenerationResultQueue {
    pub fn push(
        &mut self,
        result: GenerationResult,
    ) {
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
    pub fn tick(
        &mut self,
        dt: f32,
    ) {
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
    pub fn push(
        &mut self,
        entry: GeneratedContent,
    ) {
        if self.entries.len() >= MAX_HISTORY_SIZE {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
    }

    pub fn last(&self) -> Option<&GeneratedContent> {
        self.entries.back()
    }

    pub fn find_by_id(
        &self,
        content_id: u64,
    ) -> Option<&GeneratedContent> {
        self.entries
            .iter()
            .find(|entry| entry.content_id == content_id)
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
        self.shaders.iter().filter(|shader| shader.active)
    }

    pub fn find_by_id(
        &self,
        shader_id: u64,
    ) -> Option<&CustomShader> {
        self.shaders
            .iter()
            .find(|shader| shader.shader_id == shader_id)
    }

    pub fn add_or_update(
        &mut self,
        shader: CustomShader,
    ) {
        if let Some(existing) = self
            .shaders
            .iter_mut()
            .find(|existing| existing.shader_id == shader.shader_id)
        {
            *existing = shader;
        } else {
            self.shaders.push(shader);
        }
    }
}
