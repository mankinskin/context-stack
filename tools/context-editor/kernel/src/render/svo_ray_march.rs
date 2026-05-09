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

// ---------------------------------------------------------------------------
// Compute bind group layout
// ---------------------------------------------------------------------------

/// Compute bind group layout matching `svo_ray_march.wgsl` group(0).
fn compute_bind_group_layout() -> BindGroupLayoutDescriptor {
    BindGroupLayoutDescriptor::new(
        "bgl_svo_ray_march_compute",
        &[
            // 0: octree (read-only storage)
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // 1: RayMarchUniforms (uniform)
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // 2: SvoTransform (uniform)
            BindGroupLayoutEntry {
                binding: 2,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // 3: depth_buffer (read-write storage)
            BindGroupLayoutEntry {
                binding: 3,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // 4: color_buffer (read-write storage)
            BindGroupLayoutEntry {
                binding: 4,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // 5: page_table (read-only storage — Phase 4a)
            BindGroupLayoutEntry {
                binding: 5,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    )
}

// ---------------------------------------------------------------------------
// Blit bind group layout (group 1)
// ---------------------------------------------------------------------------

/// Empty placeholder layout for group 0 in the blit pipeline.
/// The blit shader uses @group(1) for its bindings; WebGPU requires that the
/// pipeline layout include a (possibly empty) BGL at every index up to the
/// highest group used, so group 0 must exist even though no blit binding sits there.
fn blit_dummy_group0_layout() -> BindGroupLayoutDescriptor {
    BindGroupLayoutDescriptor::new("bgl_svo_blit_group0_empty", &[])
}

/// Blit bind group layout matching `svo_ray_march.wgsl` group(1).
fn blit_bind_group_layout() -> BindGroupLayoutDescriptor {
    BindGroupLayoutDescriptor::new(
        "bgl_svo_ray_march_blit",
        &[
            // 0: blit_color_buf (read-only storage)
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // 1: RayMarchUniforms (uniform) — same buffer rebound for screen_width
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    )
}

// ---------------------------------------------------------------------------
// Pipeline resources (created in the render sub-app Queue schedule)
// ---------------------------------------------------------------------------

/// Cached compute pipeline for the ray march dispatch.
#[derive(Resource)]
pub struct SvoRayMarchComputePipeline(pub CachedComputePipelineId);

/// Cached render pipeline for the blit pass.
#[derive(Resource)]
pub struct SvoRayMarchBlitPipeline(pub CachedRenderPipelineId);

/// Pre-built compute bind group (rebuilt each frame because buffers can resize).
#[derive(Resource)]
pub struct SvoRayMarchComputeBindGroup(pub BindGroup);

/// Pre-built blit bind group (rebuilt each frame because buffers can resize).
#[derive(Resource)]
pub struct SvoRayMarchBlitBindGroup(pub BindGroup);

/// Queue both pipelines for compilation (runs once at startup).
pub fn queue_ray_march_pipelines(
    mut commands: Commands,
    pipeline_cache: Res<PipelineCache>,
    asset_server: Res<AssetServer>,
    existing_cmp: Option<Res<SvoRayMarchComputePipeline>>,
    existing_blit: Option<Res<SvoRayMarchBlitPipeline>>,
) {
    let shader = asset_server
        .load("embedded://context_editor_kernel/render/svo_ray_march.wgsl");

    if existing_cmp.is_none() {
        let id =
            pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
                label: Some("svo_ray_march_compute".into()),
                layout: vec![compute_bind_group_layout()],
                push_constant_ranges: vec![],
                shader: shader.clone(),
                shader_defs: vec![],
                entry_point: Some("ray_march_main".into()),
                zero_initialize_workgroup_memory: true,
            });
        commands.insert_resource(SvoRayMarchComputePipeline(id));
    }

    if existing_blit.is_none() {
        let id =
            pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
                label: Some("svo_ray_march_blit".into()),
                layout: vec![
                    blit_dummy_group0_layout(),
                    blit_bind_group_layout(),
                ],
                push_constant_ranges: vec![],
                vertex: VertexState {
                    shader: shader.clone(),
                    shader_defs: vec![],
                    entry_point: Some("blit_vs".into()),
                    buffers: vec![],
                },
                fragment: Some(FragmentState {
                    shader,
                    shader_defs: vec![],
                    entry_point: Some("blit_fs".into()),
                    targets: vec![Some(ColorTargetState {
                        format: TextureFormat::bevy_default(),
                        blend: Some(BlendState {
                            color: BlendComponent {
                                src_factor: BlendFactor::One,
                                dst_factor: BlendFactor::Zero,
                                operation: BlendOperation::Add,
                            },
                            alpha: BlendComponent::OVER,
                        }),
                        write_mask: ColorWrites::ALL,
                    })],
                }),
                primitive: PrimitiveState::default(),
                depth_stencil: None,
                multisample: MultisampleState {
                    count: 4, // matches Bevy default Msaa::Sample4
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                zero_initialize_workgroup_memory: false,
            });
        commands.insert_resource(SvoRayMarchBlitPipeline(id));
    }
}

/// Rebuild both bind groups each frame (buffers may be recreated on resize).
pub fn rebuild_ray_march_bind_groups(
    mut commands: Commands,
    device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    rm_buffers: Option<Res<SvoRayMarchBuffers>>,
    rm_uniforms: Option<Res<SvoRayMarchUniformBuffer>>,
    svo: Option<Res<SvoDoubleBuffer>>,
    svo_tf: Option<Res<SvoTransformBuffer>>,
    page_table_buf: Option<Res<SvoPageTableBuffer>>,
) {
    let Some(rm_buffers) = rm_buffers else { return };
    let Some(rm_uniforms) = rm_uniforms else {
        return;
    };
    let Some(svo) = svo else { return };
    let Some(svo_tf) = svo_tf else { return };

    // Binding 5: use the real page table once available, otherwise fall back
    // to the octree buffer so the bind group can be built on startup frames
    // before `init_page_table_system` has run.
    let page_table_binding = page_table_buf
        .as_ref()
        .map(|pt| pt.buffer.as_entire_binding())
        .unwrap_or_else(|| svo.read_source().as_entire_binding());

    // Compute bind group
    {
        let layout =
            pipeline_cache.get_bind_group_layout(&compute_bind_group_layout());
        let bg = device.create_bind_group(
            "bg_svo_ray_march_compute",
            &layout,
            &[
                BindGroupEntry {
                    binding: 0,
                    resource: svo.read_source().as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: rm_uniforms.0.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: svo_tf.0.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: rm_buffers.depth.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 4,
                    resource: rm_buffers.color.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 5,
                    resource: page_table_binding,
                },
            ],
        );
        commands.insert_resource(SvoRayMarchComputeBindGroup(bg));
    }

    // Blit bind group
    {
        let layout =
            pipeline_cache.get_bind_group_layout(&blit_bind_group_layout());
        let bg = device.create_bind_group(
            "bg_svo_ray_march_blit",
            &layout,
            &[
                BindGroupEntry {
                    binding: 0,
                    resource: rm_buffers.color.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: rm_uniforms.0.as_entire_binding(),
                },
            ],
        );
        commands.insert_resource(SvoRayMarchBlitBindGroup(bg));
    }
}

// ---------------------------------------------------------------------------
// Render node
// ---------------------------------------------------------------------------

/// Render graph node that:
/// 1. Dispatches the SVO ray march compute shader.
/// 2. Blits the result to the current `ViewTarget`.
pub struct SvoRayMarchNode {
    view_query: bevy::ecs::query::QueryState<&'static ViewTarget>,
}

impl FromWorld for SvoRayMarchNode {
    fn from_world(world: &mut World) -> Self {
        Self {
            view_query: bevy::ecs::query::QueryState::new(world),
        }
    }
}

impl Node for SvoRayMarchNode {
    fn update(
        &mut self,
        world: &mut World,
    ) {
        self.view_query.update_archetypes(world);
    }

    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), NodeRunError> {
        let Some(compute_pipeline_res) =
            world.get_resource::<SvoRayMarchComputePipeline>()
        else {
            return Ok(());
        };
        let Some(blit_pipeline_res) =
            world.get_resource::<SvoRayMarchBlitPipeline>()
        else {
            return Ok(());
        };
        let Some(compute_bg) =
            world.get_resource::<SvoRayMarchComputeBindGroup>()
        else {
            return Ok(());
        };
        let Some(blit_bg) = world.get_resource::<SvoRayMarchBlitBindGroup>()
        else {
            return Ok(());
        };
        let Some(pipeline_cache) = world.get_resource::<PipelineCache>() else {
            return Ok(());
        };
        let Some(rm_buffers) = world.get_resource::<SvoRayMarchBuffers>()
        else {
            return Ok(());
        };

        let Some(compute_pipeline) =
            pipeline_cache.get_compute_pipeline(compute_pipeline_res.0)
        else {
            return Ok(());
        };
        let Some(blit_pipeline) =
            pipeline_cache.get_render_pipeline(blit_pipeline_res.0)
        else {
            return Ok(());
        };

        let binding = self.view_query.query_manual(world);
        let Ok(view_target) = binding.single() else {
            return Ok(());
        };

        let width = rm_buffers.width.max(1);
        let height = rm_buffers.height.max(1);
        let wg_x = (width + 7) / 8;
        let wg_y = (height + 7) / 8;

        let encoder = render_context.command_encoder();

        // --- Compute pass: ray march ---
        {
            let mut cpass =
                encoder.begin_compute_pass(&ComputePassDescriptor {
                    label: Some("svo_ray_march_compute"),
                    timestamp_writes: None,
                });
            cpass.set_pipeline(compute_pipeline);
            cpass.set_bind_group(0, &compute_bg.0, &[]);
            cpass.dispatch_workgroups(wg_x, wg_y, 1);
        }

        // --- Render pass: blit to view target ---
        {
            let color_attachment = view_target.get_color_attachment();
            let mut rpass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("svo_ray_march_blit"),
                color_attachments: &[Some(color_attachment)],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            rpass.set_pipeline(blit_pipeline);
            rpass.set_bind_group(1, &blit_bg.0, &[]); // blit shader uses @group(1)
            rpass.draw(0..3, 0..1);
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ray_march_uniform_size() {
        assert_eq!(std::mem::size_of::<RayMarchUniformData>(), 224);
    }

    #[test]
    fn dispatch_dimensions() {
        // Verify workgroup count formula for common resolutions.
        let (w, h) = (1920u32, 1080u32);
        let wg_x = (w + 7) / 8;
        let wg_y = (h + 7) / 8;
        assert_eq!(wg_x, 240);
        assert_eq!(wg_y, 135);
        // Every pixel is covered.
        assert!(wg_x * 8 >= w);
        assert!(wg_y * 8 >= h);
    }
}
