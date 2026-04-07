//! Runtime-tweakable parameters for every stage of the voxel rendering pipeline.
//!
//! All parameters are exposed as Bevy resources and flow to the GPU each frame
//! through [`GpuRenderUniforms`].  Changing any field at runtime takes effect on
//! the very next frame — no pipeline recreation required.

use bevy::prelude::*;
use bytemuck::{Pod, Zeroable};

// ---------------------------------------------------------------------------
// Per-subsystem parameter resources
// ---------------------------------------------------------------------------

/// Sparse Voxel Octree structural parameters.
#[derive(Resource, Clone, Debug)]
pub struct SvoParams {
    /// Maximum octree depth (default: 10 → 1024³).
    pub max_depth: u32,
    /// Voxels per chunk for collider generation (default: 16).
    pub chunk_size: u32,
}

impl Default for SvoParams {
    fn default() -> Self {
        Self {
            max_depth: 10,
            chunk_size: 16,
        }
    }
}

/// Voxel splat generation parameters.
#[derive(Resource, Clone, Debug)]
pub struct SplatParams {
    /// Maximum number of splats in the GPU buffer.
    pub max_splats: u32,
    /// Bits used for roughness encoding.
    pub roughness_bits: u32,
    /// Distance at which splats render at max detail.
    pub lod_near: f32,
    /// Distance at which splats render at minimum detail.
    pub lod_far: f32,
    /// Minimum splat scale factor at far LOD.
    pub lod_min_scale: f32,
    /// When `false`, splat generation is disabled (debug).
    pub generation_enabled: bool,
}

impl Default for SplatParams {
    fn default() -> Self {
        Self {
            max_splats: 2_000_000,
            roughness_bits: 5,
            lod_near: 5.0,
            lod_far: 100.0,
            lod_min_scale: 0.5,
            generation_enabled: true,
        }
    }
}

/// Sort-key construction / AABB screen projection parameters.
#[derive(Resource, Clone, Debug)]
pub struct SortKeyParams {
    /// Anti-aliasing: added to cov2d diagonal.
    pub low_pass_filter: f32,
    /// Skip splats smaller than this many pixels.
    pub cull_screen_threshold: f32,
}

impl Default for SortKeyParams {
    fn default() -> Self {
        Self {
            low_pass_filter: 0.3,
            cull_screen_threshold: 0.1,
        }
    }
}

/// GPU radix-sort parameters.
#[derive(Resource, Clone, Debug)]
pub struct SortParams {
    /// Bits processed per sort pass.
    pub radix_bits: u32,
    /// Number of radix passes.
    pub num_passes: u32,
    /// Threads per compute workgroup.
    pub workgroup_size: u32,
}

impl Default for SortParams {
    fn default() -> Self {
        Self {
            radix_bits: 4,
            num_passes: 8,
            workgroup_size: 256,
        }
    }
}

/// Tile-based rasterisation parameters.
#[derive(Resource, Clone, Debug)]
pub struct TileParams {
    /// Pixels per tile edge.
    pub tile_size: u32,
    /// Maximum splats considered per tile.
    pub max_splats_per_tile: u32,
    /// Stop blending when remaining alpha falls below this.
    pub early_out_alpha: f32,
}

impl Default for TileParams {
    fn default() -> Self {
        Self {
            tile_size: 16,
            max_splats_per_tile: 512,
            early_out_alpha: 0.01,
        }
    }
}

/// Glass refraction / caustic / frost parameters.
#[derive(Resource, Clone, Debug)]
pub struct GlassParams {
    /// Index of refraction.
    pub ior: f32,
    /// Red channel chromatic aberration scale.
    pub chromatic_r_scale: f32,
    /// Green channel chromatic aberration scale.
    pub chromatic_g_scale: f32,
    /// Blue channel chromatic aberration scale.
    pub chromatic_b_scale: f32,
    /// Multiplicative strength of pseudo-caustics.
    pub caustic_strength: f32,
    /// Maximum mipmap level sampled for frost blur.
    pub max_frost_mip_level: f32,
    /// Multiplier for fwidth(normal)→blur radius mapping.
    pub curvature_blur_factor: f32,
}

impl Default for GlassParams {
    fn default() -> Self {
        Self {
            ior: 1.5,
            chromatic_r_scale: 1.0,
            chromatic_g_scale: 1.1,
            chromatic_b_scale: 1.2,
            caustic_strength: 2.0,
            max_frost_mip_level: 9.0,
            curvature_blur_factor: 4.0,
        }
    }
}

/// Double-buffer upload behaviour.
#[derive(Resource, Clone, Debug)]
pub struct DoubleBufferParams {
    /// When `false`, falls back to single-buffer mode (may stutter).
    pub enabled: bool,
    /// Maximum bytes to upload per frame.
    pub upload_budget_bytes: u32,
}

impl Default for DoubleBufferParams {
    fn default() -> Self {
        Self {
            enabled: true,
            upload_budget_bytes: 4 * 1024 * 1024, // 4 MiB
        }
    }
}

/// Top-level aggregate containing all runtime parameters.
#[derive(Resource, Clone, Debug)]
pub struct RenderParams {
    pub svo: SvoParams,
    pub splat: SplatParams,
    pub sort_key: SortKeyParams,
    pub sort: SortParams,
    pub tile: TileParams,
    pub glass: GlassParams,
    pub double_buffer: DoubleBufferParams,
}

impl Default for RenderParams {
    fn default() -> Self {
        Self {
            svo: SvoParams::default(),
            splat: SplatParams::default(),
            sort_key: SortKeyParams::default(),
            sort: SortParams::default(),
            tile: TileParams::default(),
            glass: GlassParams::default(),
            double_buffer: DoubleBufferParams::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// GPU uniform struct
// ---------------------------------------------------------------------------

/// Packed uniform buffer uploaded to the GPU every frame.
///
/// Field layout matches the WGSL `RenderUniforms` struct that all compute and
/// fragment shaders reference through `@group(0) @binding(0)`.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct GpuRenderUniforms {
    // Camera (set from query each frame)
    pub view_matrix: [f32; 16],
    pub proj_matrix: [f32; 16],
    pub camera_pos: [f32; 4],
    pub viewport_size: [f32; 2],

    // Splat params
    pub max_splats: u32,
    pub roughness_bits: u32,
    pub lod_near: f32,
    pub lod_far: f32,
    pub lod_min_scale: f32,

    // AABB projection
    pub aabb_padding: f32,
    pub cull_screen_threshold: f32,

    // Tiling
    pub tile_size: u32,
    pub grid_width: u32,
    pub grid_height: u32,
    pub early_out_alpha: f32,

    // Glass
    pub glass_ior: f32,
    pub chromatic_scales: [f32; 3],
    pub caustic_strength: f32,
    pub max_frost_mip: f32,
    pub curvature_blur_factor: f32,

    // Frame
    pub frame_index: u32,
    pub _padding: [f32; 3],
}

impl GpuRenderUniforms {
    /// Build a complete uniform struct from the aggregate [`RenderParams`],
    /// camera transforms, and the current viewport + frame index.
    pub fn from_params(
        params: &RenderParams,
        view: &Mat4,
        proj: &Mat4,
        cam_pos: Vec3,
        viewport: Vec2,
        frame: u32,
    ) -> Self {
        let grid_w = (viewport.x as u32 + params.tile.tile_size - 1) / params.tile.tile_size;
        let grid_h = (viewport.y as u32 + params.tile.tile_size - 1) / params.tile.tile_size;

        Self {
            view_matrix: view.to_cols_array(),
            proj_matrix: proj.to_cols_array(),
            camera_pos: [cam_pos.x, cam_pos.y, cam_pos.z, 1.0],
            viewport_size: [viewport.x, viewport.y],

            max_splats: params.splat.max_splats,
            roughness_bits: params.splat.roughness_bits,
            lod_near: params.splat.lod_near,
            lod_far: params.splat.lod_far,
            lod_min_scale: params.splat.lod_min_scale,

            aabb_padding: params.sort_key.low_pass_filter,
            cull_screen_threshold: params.sort_key.cull_screen_threshold,

            tile_size: params.tile.tile_size,
            grid_width: grid_w,
            grid_height: grid_h,
            early_out_alpha: params.tile.early_out_alpha,

            glass_ior: params.glass.ior,
            chromatic_scales: [
                params.glass.chromatic_r_scale,
                params.glass.chromatic_g_scale,
                params.glass.chromatic_b_scale,
            ],
            caustic_strength: params.glass.caustic_strength,
            max_frost_mip: params.glass.max_frost_mip_level,
            curvature_blur_factor: params.glass.curvature_blur_factor,

            frame_index: frame,
            _padding: [0.0; 3],
        }
    }
}

// ---------------------------------------------------------------------------
// GPU buffer resource
// ---------------------------------------------------------------------------

/// Bevy resource holding the GPU buffer for [`GpuRenderUniforms`].
#[derive(Resource)]
pub struct GpuRenderUniformBuffer {
    pub buffer: wgpu::Buffer,
    pub frame_index: u32,
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// One-shot system: create the uniform buffer if it doesn't exist yet.
pub fn init_uniform_buffer(
    mut commands: Commands,
    device: Res<bevy::render::renderer::RenderDevice>,
    existing: Option<Res<GpuRenderUniformBuffer>>,
) {
    if existing.is_some() {
        return;
    }
    let size = std::mem::size_of::<GpuRenderUniforms>() as u64;
    let raw_device: &wgpu::Device = device.wgpu_device();
    let buffer = raw_device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("render_uniforms"),
        size,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    commands.insert_resource(GpuRenderUniformBuffer {
        buffer,
        frame_index: 0,
    });
}

/// Per-frame system: re-upload the uniform buffer from the current
/// [`RenderParams`] and camera query.
pub fn upload_render_uniforms(
    params: Res<RenderParams>,
    camera_q: Query<(&GlobalTransform, &Projection), With<Camera3d>>,
    mut uniform_buf: Option<ResMut<GpuRenderUniformBuffer>>,
    queue: Res<bevy::render::renderer::RenderQueue>,
    windows: Query<&Window>,
) {
    let Some(ref mut buf) = uniform_buf else {
        return;
    };

    let Ok((cam_transform, cam_proj)) = camera_q.single() else {
        return;
    };

    let viewport = windows
        .single()
        .map(|w| Vec2::new(w.width(), w.height()))
        .unwrap_or(Vec2::new(1280.0, 720.0));

    let view = cam_transform.to_matrix().inverse();
    let proj = cam_proj.get_clip_from_view();

    buf.frame_index = buf.frame_index.wrapping_add(1);

    let uniforms = GpuRenderUniforms::from_params(
        &params,
        &view,
        &proj,
        cam_transform.translation(),
        viewport,
        buf.frame_index,
    );

    queue.write_buffer(&buf.buffer, 0, bytemuck::bytes_of(&uniforms));
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

/// Registers all runtime parameter resources and the per-frame uniform upload.
pub struct RuntimeParamsPlugin;

impl Plugin for RuntimeParamsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RenderParams>();
        app.add_systems(
            PostUpdate,
            (init_uniform_buffer, upload_render_uniforms).chain(),
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
    fn default_params_are_sane() {
        let p = RenderParams::default();
        assert_eq!(p.svo.max_depth, 10);
        assert_eq!(p.splat.max_splats, 2_000_000);
        assert!(p.splat.generation_enabled);
        assert!(p.double_buffer.enabled);
        assert_eq!(p.double_buffer.upload_budget_bytes, 4 * 1024 * 1024);
        assert_eq!(p.tile.tile_size, 16);
        assert_eq!(p.sort.num_passes, 8);
        assert!((p.glass.ior - 1.5).abs() < f32::EPSILON);
    }

    #[test]
    fn gpu_uniforms_size_is_pod() {
        // Must be a multiple of 16 for WebGPU alignment.
        let size = std::mem::size_of::<GpuRenderUniforms>();
        assert_eq!(size % 16, 0, "GpuRenderUniforms must be 16-byte aligned, got {size}");
    }

    #[test]
    fn from_params_computes_grid_dimensions() {
        let params = RenderParams::default(); // tile_size=16
        let uniforms = GpuRenderUniforms::from_params(
            &params,
            &Mat4::IDENTITY,
            &Mat4::IDENTITY,
            Vec3::ZERO,
            Vec2::new(800.0, 600.0),
            42,
        );
        // 800/16=50, 600/16=37.5 → ceil → 38
        assert_eq!(uniforms.grid_width, 50);
        assert_eq!(uniforms.grid_height, 38);
        assert_eq!(uniforms.frame_index, 42);
    }

    #[test]
    fn glass_chromatic_scales_packed() {
        let params = RenderParams::default();
        let uniforms = GpuRenderUniforms::from_params(
            &params,
            &Mat4::IDENTITY,
            &Mat4::IDENTITY,
            Vec3::ZERO,
            Vec2::new(1920.0, 1080.0),
            0,
        );
        assert!((uniforms.chromatic_scales[0] - 1.0).abs() < f32::EPSILON);
        assert!((uniforms.chromatic_scales[1] - 1.1).abs() < f32::EPSILON);
        assert!((uniforms.chromatic_scales[2] - 1.2).abs() < f32::EPSILON);
    }

    #[test]
    fn disabled_generation_preserves_max_splats() {
        let mut params = RenderParams::default();
        params.splat.generation_enabled = false;
        // generation_enabled is a CPU-side flag — the uniform still
        // carries max_splats so the GPU buffer allocation stays stable.
        let uniforms = GpuRenderUniforms::from_params(
            &params,
            &Mat4::IDENTITY,
            &Mat4::IDENTITY,
            Vec3::ZERO,
            Vec2::new(640.0, 480.0),
            0,
        );
        assert_eq!(uniforms.max_splats, 2_000_000);
    }
}
