//! Glass Panel SDF — ECS components and GPU buffer for the glass system.
//!
//! Glass panels are floating translucent geometry evaluated in the tiled
//! rasteriser's glass pre-loop. Each panel has:
//! - Rounded-box SDF for smooth edges
//! - Snell's law refraction (bends the ray through the panel)
//! - Per-panel tint and IOR
//!
//! The [`GlassPanelBuffer`] resource holds a packed storage buffer read
//! by `tiled_raster.wgsl` at `@group(0) @binding(4)`.

use bevy::prelude::*;
use bevy::render::{
    render_resource::{Buffer, BufferDescriptor, BufferUsages},
    renderer::{RenderDevice, RenderQueue},
};

/// Maximum number of glass panels evaluated per frame.
pub const MAX_GLASS_PANELS: u32 = 16;

/// Byte size of a single packed glass panel on the GPU (16 × f32 = 64 bytes).
pub const GLASS_PANEL_GPU_SIZE: u64 = 64;

/// Total glass buffer size.
pub const GLASS_BUFFER_SIZE: u64 = GLASS_PANEL_GPU_SIZE * MAX_GLASS_PANELS as u64;

// ---------------------------------------------------------------------------
// ECS Component
// ---------------------------------------------------------------------------

/// Marks an entity as a glass panel for the SDF refraction system.
///
/// Attach to any entity with a [`Transform`] to position it in world space.
/// The tiled rasteriser evaluates the rounded-box SDF per-pixel and bends
/// rays that pass through the panel via Snell's law.
#[derive(Component)]
pub struct GlassPanel {
    /// Index of refraction (glass ≈ 1.5, water ≈ 1.33).
    pub ior: f32,
    /// sRGBA tint applied multiplicatively when looking through this panel.
    pub tint: [f32; 4],
    /// Blur roughness for frosted glass (0.0 = clear, 1.0 = fully frosted).
    pub blur_roughness: f32,
    /// Rounding radius for the panel corners.
    pub corner_radius: f32,
    /// Half-size of the panel's axis-aligned bounding box.
    pub half_size: Vec3,
    /// Caustic brightness multiplier (light convergence through refraction).
    pub caustic_strength: f32,
    /// Chromatic spread — how much RGB channels diverge through refraction.
    pub chromatic_spread: f32,
}

// ---------------------------------------------------------------------------
// GPU-packed struct (matches WGSL GlassPanelData)
// ---------------------------------------------------------------------------

/// Packed GPU representation of a glass panel (64 bytes).
///
/// WGSL layout:
/// ```wgsl
/// struct GlassPanelData {
///     center: vec3f,
///     corner_radius: f32,
///     half_size: vec3f,
///     ior: f32,
///     tint: vec4f,
///     blur_roughness: f32,
///     caustic_strength: f32,
///     chromatic_spread: f32,
///     _pad: f32,
/// }
/// ```
#[repr(C)]
#[derive(Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GlassPanelGpu {
    pub center: [f32; 3],
    pub corner_radius: f32,
    pub half_size: [f32; 3],
    pub ior: f32,
    pub tint: [f32; 4],
    pub blur_roughness: f32,
    pub caustic_strength: f32,
    pub chromatic_spread: f32,
    pub _pad: f32,
}

// ---------------------------------------------------------------------------
// GPU buffer resource
// ---------------------------------------------------------------------------

/// GPU storage buffer holding packed glass panel data.
///
/// Updated each frame by [`update_glass_panel_buffer`] and bound at
/// `@group(0) @binding(4)` in `tiled_raster.wgsl`.
#[derive(Resource)]
pub struct GlassPanelBuffer {
    pub buffer: Buffer,
    /// Number of active panels this frame (written into `RasterUniforms.glass_count`).
    pub count: u32,
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// Create the glass panel GPU storage buffer (once).
pub fn init_glass_resources(
    mut commands: Commands,
    device: Option<Res<RenderDevice>>,
    existing: Option<Res<GlassPanelBuffer>>,
) {
    if existing.is_some() {
        return;
    }
    let Some(device) = device else { return };

    let buffer = device.create_buffer(&BufferDescriptor {
        label: Some("glass_panels"),
        size: GLASS_BUFFER_SIZE,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    commands.insert_resource(GlassPanelBuffer { buffer, count: 0 });
}

/// Pack all [`GlassPanel`] entities into the GPU buffer each frame.
pub fn update_glass_panel_buffer(
    query: Query<(&Transform, &GlassPanel)>,
    glass_buffer: Option<ResMut<GlassPanelBuffer>>,
    render_queue: Option<Res<RenderQueue>>,
) {
    let Some(mut glass_buffer) = glass_buffer else { return };
    let Some(render_queue) = render_queue else { return };

    let mut panels = Vec::new();
    for (transform, panel) in query.iter().take(MAX_GLASS_PANELS as usize) {
        let pos = transform.translation;
        panels.push(GlassPanelGpu {
            center: [pos.x, pos.y, pos.z],
            corner_radius: panel.corner_radius,
            half_size: [panel.half_size.x, panel.half_size.y, panel.half_size.z],
            ior: panel.ior,
            tint: panel.tint,
            blur_roughness: panel.blur_roughness,
            caustic_strength: panel.caustic_strength,
            chromatic_spread: panel.chromatic_spread,
            _pad: 0.0,
        });
    }

    glass_buffer.count = panels.len() as u32;
    if !panels.is_empty() {
        render_queue.write_buffer(
            &glass_buffer.buffer,
            0,
            bytemuck::cast_slice(&panels),
        );
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glass_panel_gpu_size_matches_wgsl() {
        // 3 + 1 + 3 + 1 + 4 + 1 + 3 = 16 floats × 4 bytes = 64 bytes
        assert_eq!(std::mem::size_of::<GlassPanelGpu>(), 64);
        assert_eq!(GLASS_PANEL_GPU_SIZE, 64);
    }

    #[test]
    fn glass_panel_gpu_is_pod() {
        let bytes = [0u8; 64];
        let _: &GlassPanelGpu = bytemuck::from_bytes(&bytes);
    }

    #[test]
    fn buffer_fits_max_panels() {
        assert_eq!(
            GLASS_BUFFER_SIZE,
            GLASS_PANEL_GPU_SIZE * MAX_GLASS_PANELS as u64
        );
    }
}
