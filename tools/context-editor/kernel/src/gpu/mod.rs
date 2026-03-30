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
//! SvoDoubleBuffer      ─── FRONT buffer ──▶ GPU reads (gaussian gen, EWA, ...)
//! (Resource)           ─── BACK  buffer ──▶ WASM writes dirty octree regions
//!                          swap() ──────▶   pointer flip, < 0.01 ms
//!
//! SplatBuffers         ─── per-frame Gaussian pipeline buffers (no re-alloc)
//!
//! DoubleBindGroups     ─── pre-built BindGroups for front and back SVO.
//!                          active_svo_group(current_is_front) selects the right one.
//! ```

use bevy::prelude::{Commands, Query, Res, Resource, Window};
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

/// Default maximum Gaussians generated per frame.
pub const MAX_GAUSSIANS: u32 = 1_048_576; // 1 M

/// Tile side length in pixels for the tiled forward+ renderer.
pub const TILE_SIZE: u32 = 16;

/// Maximum active-list entries (splat-tile overlaps).
///
/// Each sorted splat can appear in multiple tiles via AABB overlap.  This
/// limit must fit into the 20-bit offset field of packed `TileData`.
pub const MAX_ACTIVE_ENTRIES: u32 = MAX_GAUSSIANS;

/// Byte stride of `GaussianData` in the GPU buffer.
///
/// WGSL layout:
/// - `position:   vec3f`           → 12 bytes
/// - `opacity:    f32`             →  4 bytes
/// - `covariance: array<f32, 6>`   → 24 bytes  (upper-triangle of 3×3 Σ)
/// - `sh_coeffs:  array<f32, 48>`  → 192 bytes (degree-3 SH, 3 channels)
pub const GAUSSIAN_DATA_STRIDE: u64 = 232;

/// Byte stride of `ProjectedGaussian` after EWA projection.
///
/// WGSL layout:
/// - `center_screen: vec2f`  →  8 bytes
/// - `cov2d_inv:     vec3f`  → 12 bytes  (a, b, c of 2×2 inverse)
/// - `depth:         f32`    →  4 bytes
/// - `color:         vec3f`  → 12 bytes  (SH-evaluated view-dependent RGB)
/// - `opacity:       f32`    →  4 bytes
pub const PROJECTED_GAUSSIAN_STRIDE: u64 = 40;

/// Byte stride of packed `TileData` — single `u32`: `(offset << 12) | count`.
///
/// - Bits 12–31 (20 bits): offset into sorted array (max 1,048,576 = `MAX_GAUSSIANS`)
/// - Bits  0–11 (12 bits): splat count per tile (max 4,095 with overflow guard)
pub const TILE_DATA_STRIDE: u64 = 4;

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
// SplatBuffers
// ---------------------------------------------------------------------------

/// Pre-allocated per-frame GPU buffers for the voxel splatting pipeline.
///
/// All buffers are sized at startup to `max_splats` and `tile_count` so no
/// allocation occurs on the render-critical path.
#[derive(Resource, Clone)]
pub struct SplatBuffers {
    /// Voxel splat kernel output — one `VoxelSplat` per occupied leaf voxel.
    pub splats: Buffer,
    /// EWA-projected screen-space ellipses (`ProjectedGaussian[]`).
    pub projected: Buffer,
    /// Radix sort keys: `tile_id (20 bit) | depth (12 bit)` per splat.
    pub sort_keys: Buffer,
    /// Radix sort values: indices into `projected[]`.
    pub sort_values: Buffer,
    /// Radix sort ping-pong scratch (keys).
    pub sort_scratch_keys: Buffer,
    /// Radix sort ping-pong scratch (values).
    pub sort_scratch_values: Buffer,
    /// Per-workgroup 16-digit histograms for the 8 radix passes
    /// (size = workgroups × 8 passes × 16 digits × 4 bytes).
    pub histograms: Buffer,
    /// Per-tile packed `TileData`: `(offset << 12) | count`.
    pub tile_data: Buffer,
    /// Per-tile atomic counters used by the count-tile-overlaps pass.
    pub tile_counts: Buffer,
    /// Per-tile atomic write heads used by the scatter-to-tiles pass.
    pub tile_write_heads: Buffer,
    /// Active list: splat indices written by scatter, read by rasteriser.
    pub active_list: Buffer,
    /// Atomic u32 counter: number of splats emitted by the kernel.
    pub splat_count: Buffer,
    /// Maximum splats this allocation supports.
    pub max_splats: u32,
    /// Tile grid dimensions at the configured viewport size.
    pub tiles_x: u32,
    pub tiles_y: u32,
}

impl SplatBuffers {
    /// Allocate all buffers for up to `max_splats` and the given viewport.
    pub fn new(
        device: &RenderDevice,
        max_splats: u32,
        viewport_width: u32,
        viewport_height: u32,
    ) -> Self {
        let n = max_splats as u64;
        let tiles_x = (viewport_width + TILE_SIZE - 1) / TILE_SIZE;
        let tiles_y = (viewport_height + TILE_SIZE - 1) / TILE_SIZE;
        let tile_count = (tiles_x * tiles_y) as u64;
        // Histogram: workgroups × 8 radix passes × 16 digits × 4 bytes
        let wg_count = (n + 255) / 256;
        let histogram_size = wg_count * 8 * 16 * 4;

        let rw = BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC;
        let atomic = BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC;

        macro_rules! buf {
            ($label:expr, $size:expr, $usage:expr) => {
                device.create_buffer(&BufferDescriptor {
                    label: Some($label),
                    size: $size,
                    usage: $usage,
                    mapped_at_creation: false,
                })
            };
        }

        Self {
            splats: buf!("splats", n * crate::splat::VOXEL_SPLAT_STRIDE, rw),
            projected: buf!("projected", n * crate::splat::PROJECTED_SPLAT_STRIDE, rw),
            sort_keys: buf!("sort_keys", n * 4, rw),
            sort_values: buf!("sort_values", n * 4, rw),
            sort_scratch_keys: buf!("sort_scratch_keys", n * 4, rw),
            sort_scratch_values: buf!("sort_scratch_values", n * 4, rw),
            histograms: buf!("radix_histograms", histogram_size, rw),
            tile_data: buf!("tile_data", tile_count * TILE_DATA_STRIDE, rw),
            tile_counts: buf!("tile_counts", tile_count * 4, rw),
            tile_write_heads: buf!("tile_write_heads", tile_count * 4, rw),
            active_list: buf!("active_list", MAX_ACTIVE_ENTRIES as u64 * 4, rw),
            splat_count: buf!("splat_count", 4, atomic),
            max_splats,
            tiles_x,
            tiles_y,
        }
    }
}

/// Create the [`SplatBuffers`] resource from the primary window dimensions.
///
/// Runs once (guards against double-init) and requires `RenderDevice` to
/// be available in the main world.
pub fn init_splat_buffers(
    mut commands: Commands,
    device: Option<Res<RenderDevice>>,
    existing: Option<Res<SplatBuffers>>,
    windows: Query<&Window>,
) {
    if existing.is_some() {
        return;
    }
    let Some(device) = device else { return };
    let Ok(window) = windows.single() else { return };

    let w = window.physical_width().max(1);
    let h = window.physical_height().max(1);

    commands.insert_resource(SplatBuffers::new(&device, MAX_GAUSSIANS, w, h));
}

impl ExtractResource for SplatBuffers {
    type Source = SplatBuffers;
    fn extract_resource(source: &Self::Source) -> Self {
        source.clone()
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
        assert_eq!(TILE_DATA_STRIDE, 4); // packed: (offset << 12) | count
        assert_eq!(OCTREE_NODE_SIZE, 8);
    }
}
