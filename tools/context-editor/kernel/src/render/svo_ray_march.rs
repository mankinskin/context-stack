//! SVO Ray March — compute node for Phase 1b.
//!
//! Dispatches `svo_ray_march.wgsl` to trace world-space rays against the GPU
//! SVO, then blits the result to the view target (swapchain surface).
//!
//! ## Pipeline overview
//!
//! The node runs two GPU passes per frame (when the ray-march toggle is active):
//!
//! 1. **Compute pass** (`ray_march_main`) — writes RGBA + depth per pixel into
//!    `SvoRayMarchBuffers::{color, depth}`.
//! 2. **Blit render pass** (`blit_vs` / `blit_fs`) — copies the RGBA buffer to
//!    the camera's `ViewTarget` (swapchain surface).
//!
//! ## Bind group layouts
//!
//! **Compute (group 0)**
//!
//! | Binding | Type | Content |
//! |---------|------|---------|
//! | 0 | `storage<read>` | `octree: array<vec2u>` — SVO front buffer |
//! | 1 | `uniform` | `RayMarchUniforms` |
//! | 2 | `uniform` | `SvoTransform` |
//! | 3 | `storage<read_write>` | `depth_buffer: array<f32>` |
//! | 4 | `storage<read_write>` | `color_buffer: array<f32>` (4 f32 per pixel) |
//!
//! **Blit (group 1)**
//!
//! | Binding | Type | Content |
//! |---------|------|---------|
//! | 0 | `storage<read>` | `blit_color_buf: array<f32>` |
//! | 1 | `uniform` | `RayMarchUniforms` (same buffer, rebound at 1) |

use bevy::{
    prelude::*,
    render::{
        extract_resource::ExtractResource,
        render_graph::{
            Node,
            NodeRunError,
            RenderGraphContext,
        },
        render_resource::{
            BindGroup,
            BindGroupEntry,
            BindGroupLayoutDescriptor,
            BindGroupLayoutEntry,
            BindingType,
            BlendComponent,
            BlendFactor,
            BlendOperation,
            BlendState,
            Buffer,
            BufferBindingType,
            BufferDescriptor,
            BufferUsages,
            CachedComputePipelineId,
            CachedRenderPipelineId,
            ColorTargetState,
            ColorWrites,
            ComputePassDescriptor,
            ComputePipelineDescriptor,
            FragmentState,
            MultisampleState,
            PipelineCache,
            PrimitiveState,
            RenderPassDescriptor,
            RenderPipelineDescriptor,
            ShaderStages,
            TextureFormat,
            VertexState,
        },
        renderer::{
            RenderContext,
            RenderDevice,
            RenderQueue,
        },
        view::ViewTarget,
    },
};
use bytemuck::{
    Pod,
    Zeroable,
};

use crate::{
    debug_overlay::{
        lod_softness,
        lod_threshold,
        ray_march_feature_flags,
    },
    gpu::{
        svo_transform::SvoTransformBuffer,
        SvoDoubleBuffer,
        SvoPageTableBuffer,
    },
};

mod node;
mod pipeline;
#[cfg(test)]
mod tests;

pub use node::SvoRayMarchNode;
pub use pipeline::{
    queue_ray_march_pipelines,
    rebuild_ray_march_bind_groups,
    SvoRayMarchBlitBindGroup,
    SvoRayMarchBlitPipeline,
    SvoRayMarchComputeBindGroup,
    SvoRayMarchComputePipeline,
};

// Per-frame counter incremented each time ray march uniforms are written.
static FRAME_COUNTER: std::sync::atomic::AtomicU32 =
    std::sync::atomic::AtomicU32::new(0);

// ---------------------------------------------------------------------------
// Uniform data (matches WGSL `RayMarchUniforms`)
// ---------------------------------------------------------------------------

/// Ray march uniforms: 144 bytes packed for GPU (buffer rounded up to 256 for alignment).
const RAY_MARCH_UNIFORM_SIZE: u64 = 256; // round up to uniform alignment

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
pub struct RayMarchUniformData {
    pub inv_view_proj: [f32; 16], // 64 bytes
    pub view_proj: [f32; 16],     // 64 bytes — Phase 3a: for NDC depth output
    pub camera_pos: [f32; 3],     // 12 bytes
    pub cot_half_fov: f32,        //  4 bytes
    pub resolution: [f32; 2],     //  8 bytes
    pub screen_width: u32,        //  4 bytes
    pub frame_index: u32,         //  4 bytes — Phase 4b: temporal noise seed
    pub light_dir: [f32; 3],      // 12 bytes
    pub _pad1: f32,               //  4 bytes
    pub light_color: [f32; 3],    // 12 bytes
    pub max_bounces: u32,         //  4 bytes
    pub max_shadow_dist: f32,     //  4 bytes
    pub feature_flags: u32, //  4 bytes (bit0=neighbor_blend, bit1=shadow, bit2=reflect, bit3=lod)
    pub lod_threshold: f32, //  4 bytes — Phase 4b: screen-px size below which LOD stops
    pub lod_softness: f32, //  4 bytes — Phase 4b: soft-band half-width in pixels
    pub _pad3: [u32; 4],   // 16 bytes (alignment pad to 16-byte boundary)
} // total: 224 bytes

const _: () = assert!(
    std::mem::size_of::<RayMarchUniformData>() == 224,
    "RayMarchUniformData must be 224 bytes"
);

// ---------------------------------------------------------------------------
// Per-frame output buffers (colour + depth)
// ---------------------------------------------------------------------------

/// GPU storage buffers written by the ray march compute shader each frame.
///
/// * `color`: 4 × f32 per pixel (RGBA, linear [0,1]) — pixel_idx*4 + channel
/// * `depth`: 1 × f32 per pixel — world-space ray parameter `t` at first hit
///
/// Resized whenever the window dimensions change.
#[derive(Resource, Clone)]
pub struct SvoRayMarchBuffers {
    pub color: Buffer,
    pub depth: Buffer,
    pub width: u32,
    pub height: u32,
}

impl ExtractResource for SvoRayMarchBuffers {
    type Source = SvoRayMarchBuffers;
    fn extract_resource(source: &Self::Source) -> Self {
        source.clone()
    }
}

impl SvoRayMarchBuffers {
    pub fn new(
        device: &RenderDevice,
        width: u32,
        height: u32,
    ) -> Self {
        let pixels = (width.max(1) as u64) * (height.max(1) as u64);
        let rw = BufferUsages::STORAGE | BufferUsages::COPY_DST;
        Self {
            color: device.create_buffer(&BufferDescriptor {
                label: Some("svo_ray_march_color"),
                size: pixels * 4 * 4, // 4 channels × 4 bytes (f32)
                usage: rw,
                mapped_at_creation: false,
            }),
            depth: device.create_buffer(&BufferDescriptor {
                label: Some("svo_ray_march_depth"),
                size: pixels * 4, // 1 × f32
                usage: rw,
                mapped_at_creation: false,
            }),
            width,
            height,
        }
    }
}

/// Create (or recreate on resize) the ray march output buffers.
pub fn init_ray_march_buffers(
    mut commands: Commands,
    device: Option<Res<RenderDevice>>,
    existing: Option<Res<SvoRayMarchBuffers>>,
    windows: Query<&Window>,
) {
    let Some(device) = device else { return };
    let Ok(window) = windows.single() else { return };
    let w = window.physical_width().max(1);
    let h = window.physical_height().max(1);

    if let Some(buf) = &existing {
        if buf.width == w && buf.height == h {
            return; // size unchanged
        }
    }
    commands.insert_resource(SvoRayMarchBuffers::new(&device, w, h));
}

// ---------------------------------------------------------------------------
// Uniform buffer resource
// ---------------------------------------------------------------------------

/// GPU uniform buffer for the ray march parameters.
#[derive(Resource, Clone)]
pub struct SvoRayMarchUniformBuffer(pub Buffer);

impl ExtractResource for SvoRayMarchUniformBuffer {
    type Source = SvoRayMarchUniformBuffer;
    fn extract_resource(source: &Self::Source) -> Self {
        source.clone()
    }
}

impl SvoRayMarchUniformBuffer {
    pub fn new(device: &RenderDevice) -> Self {
        Self(device.create_buffer(&BufferDescriptor {
            label: Some("svo_ray_march_uniforms"),
            size: RAY_MARCH_UNIFORM_SIZE,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }))
    }
}

/// Create the uniform buffer once.
pub fn init_ray_march_uniforms(
    mut commands: Commands,
    device: Option<Res<RenderDevice>>,
    existing: Option<Res<SvoRayMarchUniformBuffer>>,
) {
    if existing.is_some() {
        return;
    }
    let Some(device) = device else { return };
    commands.insert_resource(SvoRayMarchUniformBuffer::new(&device));
}

/// Write per-frame camera + viewport uniforms.
pub fn update_ray_march_uniforms(
    camera_query: Query<(&GlobalTransform, &Projection), With<Camera3d>>,
    windows: Query<&Window>,
    render_queue: Option<Res<RenderQueue>>,
    uniform_buf: Option<Res<SvoRayMarchUniformBuffer>>,
) {
    let Some(render_queue) = render_queue else {
        return;
    };
    let Some(uniform_buf) = uniform_buf else {
        return;
    };
    let Ok((tf, proj)) = camera_query.single() else {
        return;
    };
    let Ok(window) = windows.single() else { return };

    let view_mat = tf.to_matrix().inverse();
    let proj_mat = proj.get_clip_from_view();
    let vp_mat = proj_mat * view_mat; // view_proj (used for NDC depth)
    let inv_vp = vp_mat.inverse();
    let cam_pos = tf.translation();

    let width = window.physical_width().max(1) as f32;
    let height = window.physical_height().max(1) as f32;

    // Derive cot_half_fov from the projection matrix.
    // For a symmetric perspective: cot_half_fov_y = proj[1][1]
    let cot_half_fov = proj_mat.y_axis.y;

    let data = RayMarchUniformData {
        inv_view_proj: inv_vp.to_cols_array(),
        view_proj: vp_mat.to_cols_array(),
        camera_pos: [cam_pos.x, cam_pos.y, cam_pos.z],
        cot_half_fov,
        resolution: [width, height],
        screen_width: width as u32,
        frame_index: FRAME_COUNTER
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        light_dir: [0.267, 0.802, 0.534], // normalize(0.3, 0.9, 0.6)
        _pad1: 0.0,
        light_color: [1.0, 0.98, 0.95],
        max_bounces: 2,
        max_shadow_dist: 200.0,
        feature_flags: ray_march_feature_flags(),
        lod_threshold: lod_threshold(),
        lod_softness: lod_softness(),
        _pad3: [0; 4],
    };

    render_queue.write_buffer(&uniform_buf.0, 0, bytemuck::bytes_of(&data));
}
