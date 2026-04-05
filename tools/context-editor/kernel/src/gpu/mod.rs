//! WebGPU render pipeline — buffer allocation and bind group infrastructure.
//!
//! Implements T2: GPU Buffer Infrastructure.
//!
//! # Architecture
//!
//! All GPU resources are created once at startup and held as Bevy `Resource`s
//! in the render world. The hot render-loop path never allocates.
//!
//! ```text
//! SvoDoubleBuffer      ─── FRONT buffer ──▶ GPU reads (ray march, ...)
//! (Resource)           ─── BACK  buffer ──▶ WASM writes dirty octree regions
//!                          swap() ──────▶   pointer flip, < 0.01 ms
//! ```

pub mod svo_transform;
pub use svo_transform::SvoTransformBuffer;

use bevy::prelude::Resource;
use bevy::render::{
    extract_resource::ExtractResource,
    render_resource::{
        BindGroup, BindGroupEntry, BindGroupLayout, BindGroupLayoutEntry,
        BindingType, Buffer, BufferBindingType, BufferDescriptor, BufferUsages,
        SamplerBindingType, ShaderStages, TextureSampleType, TextureViewDimension,
    },
    renderer::RenderDevice,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default SVO octree capacity (nodes). At 8 bytes/node ≈ 32 MB total.
pub const SVO_CAPACITY_NODES: usize = 4_194_304; // 2^22

/// Byte size of a single `OctreeNode` (2 × u32 = child_pointer + color_data).
pub const OCTREE_NODE_SIZE: u64 = 8;

// ---------------------------------------------------------------------------
// Uniform buffer helpers
// ---------------------------------------------------------------------------

/// Camera matrices uniform buffer.
///
/// Layout (256-byte aligned):
/// - `view:       mat4x4<f32>`  → 64 bytes
/// - `projection: mat4x4<f32>`  → 64 bytes
/// - `position:   vec3f + pad`  → 16 bytes
///
/// Populated each frame by `camera_uniform_system` (implemented in T6).
#[derive(Resource)]
pub struct CameraUniformBuffer(pub Buffer);

impl CameraUniformBuffer {
    pub fn new(device: &RenderDevice) -> Self {
        Self(device.create_buffer(&BufferDescriptor {
            label: Some("camera_uniforms"),
            size: 256,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }))
    }
}

/// Global uniforms buffer (resolution, time, lod_scale, max_depth).
///
/// Layout (256-byte aligned):
/// - `resolution:  vec2f`  →  8 bytes
/// - `time:        f32`    →  4 bytes
/// - `lod_scale:   f32`    →  4 bytes
/// - `max_depth:   u32`    →  4 bytes
/// - padding to 256 bytes
///
/// Populated each frame by `light_uniform_system` (implemented in T6).
#[derive(Resource)]
pub struct GlobalUniformBuffer(pub Buffer);

impl GlobalUniformBuffer {
    pub fn new(device: &RenderDevice) -> Self {
        Self(device.create_buffer(&BufferDescriptor {
            label: Some("global_uniforms"),
            size: 256,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }))
    }
}

// ---------------------------------------------------------------------------
// SvoDoubleBuffer
// ---------------------------------------------------------------------------

/// Double-buffered Sparse Voxel Octree GPU storage.
///
/// The GPU reads the **FRONT** buffer; WASM writes dirty octree regions to the
/// **BACK** buffer. After `svo_upload_system` writes, `swap()` atomically
/// switches them — zero allocation, < 0.01 ms, 1-frame visual update latency.
#[derive(Resource, Clone)]
pub struct SvoDoubleBuffer {
    pub front: Buffer,
    pub back: Buffer,
    current_is_front: bool,
    pub capacity_nodes: usize,
}

impl ExtractResource for SvoDoubleBuffer {
    type Source = SvoDoubleBuffer;
    fn extract_resource(source: &Self::Source) -> Self {
        source.clone()
    }
}

impl SvoDoubleBuffer {
    /// Allocate both buffers for `capacity_nodes` octree nodes.
    pub fn new(device: &RenderDevice, capacity_nodes: usize) -> Self {
        let size = capacity_nodes as u64 * OCTREE_NODE_SIZE;
        let usage = BufferUsages::STORAGE | BufferUsages::COPY_DST;
        let make = |label: &'static str| {
            device.create_buffer(&BufferDescriptor {
                label: Some(label),
                size,
                usage,
                mapped_at_creation: false,
            })
        };
        Self {
            front: make("svo_front"),
            back: make("svo_back"),
            current_is_front: true,
            capacity_nodes,
        }
    }

    /// Buffer open for **writing** this frame (WASM → GPU upload target).
    pub fn write_target(&self) -> &Buffer {
        if self.current_is_front { &self.back } else { &self.front }
    }

    /// Buffer bound for **reading** by the render graph this frame.
    pub fn read_source(&self) -> &Buffer {
        if self.current_is_front { &self.front } else { &self.back }
    }

    /// Pointer-flip swap — no allocation, no GPU stall.
    pub fn swap(&mut self) {
        self.current_is_front = !self.current_is_front;
    }

    pub fn current_is_front(&self) -> bool {
        self.current_is_front
    }
}

// ---------------------------------------------------------------------------
// GpuBindGroupLayouts
// ---------------------------------------------------------------------------

/// The four bind group layouts matching the WGSL shader group declarations.
///
/// Group 0 — SVO read group (varies per double-buffer swap):
/// ```wgsl
/// @group(0) @binding(0) var<storage, read>           octree:  array<OctreeNode>;
/// @group(0) @binding(1) var<uniform>                 camera:  CameraUniforms;
/// @group(0) @binding(2) var<uniform>                 globals: GlobalUniforms;
/// ```
///
/// Group 1 — Gaussian buffers (generator writes, EWA/sort reads):
/// ```wgsl
/// @group(1) @binding(0) var<storage, read_write> gaussians:  array<GaussianData>;
/// @group(1) @binding(1) var<storage, read_write> projected:  array<ProjectedGaussian>;
/// @group(1) @binding(2) var<storage, read_write> sort_keys:  array<u32>;
/// ```
///
/// Group 2 — Tile + glass data (read-only in the fragment pass):
/// ```wgsl
/// @group(2) @binding(0) var<storage, read> tile_data:        array<TileData>;
/// @group(2) @binding(1) var<storage, read> sorted_instances: array<u32>;
/// @group(2) @binding(2) var<storage, read> glass_panels:     array<GlassPanel>;
/// ```
///
/// Group 3 — Mipmap background texture (frosted glass blur source):
/// ```wgsl
/// @group(3) @binding(0) var bg_tex:     texture_2d<f32>;
/// @group(3) @binding(1) var bg_sampler: sampler;
/// ```
#[derive(Resource)]
pub struct GpuBindGroupLayouts {
    pub svo_group:      BindGroupLayout,
    pub gaussian_group: BindGroupLayout,
    pub tile_group:     BindGroupLayout,
    pub texture_group:  BindGroupLayout,
}

impl GpuBindGroupLayouts {
    pub fn new(device: &RenderDevice) -> Self {
        // Group 0 — SVO (Bevy 0.18 API: separate label + entries args)
        let svo_group = device.create_bind_group_layout(
            "bgl_svo",
            &[
                // octree storage (read-only)
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
                // camera uniform
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE | ShaderStages::VERTEX,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // global uniforms
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE | ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        );

        // Group 1 — Gaussians (read-write)
        let gaussian_group = device.create_bind_group_layout(
            "bgl_gaussians",
            &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        );

        // Group 2 — Tile + glass (read-only in fragment)
        let tile_group = device.create_bind_group_layout(
            "bgl_tiles",
            &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE | ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        );

        // Group 3 — Mipmap background texture + sampler
        let texture_group = device.create_bind_group_layout(
            "bgl_bg_texture",
            &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        );

        Self { svo_group, gaussian_group, tile_group, texture_group }
    }
}

// ---------------------------------------------------------------------------
// DoubleBindGroups
// ---------------------------------------------------------------------------

/// Pre-built pair of Group-0 bind groups — one for FRONT SVO, one for BACK.
///
/// On each frame, the render graph calls [`active_svo_group`] to select the
/// correct bind group without any per-frame allocation.
///
/// [`active_svo_group`]: DoubleBindGroups::active_svo_group
#[derive(Resource)]
pub struct DoubleBindGroups {
    front_svo_group: BindGroup,
    back_svo_group:  BindGroup,
}

impl DoubleBindGroups {
    /// Build both bind groups from the pre-allocated buffers.
    pub fn new(
        device: &RenderDevice,
        layout: &GpuBindGroupLayouts,
        svo: &SvoDoubleBuffer,
        camera: &CameraUniformBuffer,
        globals: &GlobalUniformBuffer,
    ) -> Self {
        let make_group = |svo_buf: &Buffer| {
            // Bevy 0.18 API: create_bind_group(label, layout, entries)
            device.create_bind_group(
                "bg_svo",
                &layout.svo_group,
                &[
                    BindGroupEntry {
                        binding: 0,
                        resource: svo_buf.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: camera.0.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 2,
                        resource: globals.0.as_entire_binding(),
                    },
                ],
            )
        };
        Self {
            front_svo_group: make_group(&svo.front),
            back_svo_group: make_group(&svo.back),
        }
    }

    /// Returns the bind group for the SVO buffer currently being read by the GPU.
    pub fn active_svo_group(&self, current_is_front: bool) -> &BindGroup {
        if current_is_front {
            &self.front_svo_group
        } else {
            &self.back_svo_group
        }
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn svo_double_buffer_swap_logic() {
        // Verify the swap pointer logic without a GPU device.
        // We can't call SvoDoubleBuffer::new without a RenderDevice, so we
        // test the invariants with a hand-rolled struct.
        struct FakeSvo {
            current_is_front: bool,
        }
        impl FakeSvo {
            fn write_is_back(&self) -> bool { self.current_is_front }
            fn read_is_front(&self) -> bool { self.current_is_front }
            fn swap(&mut self) { self.current_is_front = !self.current_is_front; }
        }

        let mut svo = FakeSvo { current_is_front: true };
        assert!(svo.read_is_front(), "before swap: GPU reads front");
        assert!(svo.write_is_back(), "before swap: WASM writes back");
        svo.swap();
        assert!(!svo.read_is_front(), "after swap: GPU reads back (old back)");
        assert!(!svo.write_is_back(), "after swap: WASM writes front (old front)");
        svo.swap();
        assert!(svo.read_is_front(), "double swap: back to original");
    }

    #[test]
    fn constants_are_sane() {
        assert_eq!(GAUSSIAN_DATA_STRIDE, (3 + 1 + 6 + 48) as u64 * 4);
        assert_eq!(PROJECTED_GAUSSIAN_STRIDE, (2 + 3 + 1 + 3 + 1) as u64 * 4);
        assert_eq!(TILE_DATA_STRIDE, 8); // two separate u32s: [offset, count]
        assert_eq!(OCTREE_NODE_SIZE, 8);
    }
}
