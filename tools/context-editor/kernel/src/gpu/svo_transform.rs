//! SVO Transform Uniform — world-to-SVO coordinate mapping uploaded each frame.
//!
//! Phase 1a of the SVO ray march epic. Creates and uploads a GPU uniform buffer
//! containing the SVO root origin, world size, and max_depth that the ray march
//! shader needs to transform world-space rays into normalised [0,1]³ SVO space.

use bevy::prelude::*;
use bevy::render::{
    extract_resource::ExtractResource,
    render_resource::{Buffer, BufferDescriptor, BufferUsages},
    renderer::{RenderDevice, RenderQueue},
};
use bytemuck::{Pod, Zeroable};

use crate::svo::VoxelWorld;

// ---------------------------------------------------------------------------
// GPU-side struct (must match `svo_common.wgsl` SvoTransform layout)
// ---------------------------------------------------------------------------

/// Packed data uploaded to the `svo_transform` uniform binding.
///
/// Layout (32 bytes, padded to 256 for the uniform buffer allocation):
/// ```text
/// offset  0: origin.x        f32
/// offset  4: origin.y        f32
/// offset  8: origin.z        f32
/// offset 12: world_size      f32
/// offset 16: inv_world_size  f32
/// offset 20: max_depth       u32
/// offset 24: _pad[2]         u32 × 2
/// ```
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct SvoTransformData {
    /// World-space position of the SVO root's min corner.
    pub origin: [f32; 3],
    /// Total side length of the SVO root cube in world units (= 2^max_depth).
    pub world_size: f32,
    /// Precomputed `1.0 / world_size` for fast normalisation in the shader.
    pub inv_world_size: f32,
    /// Maximum octree depth (= leaf level).
    pub max_depth: u32,
    pub _pad: [u32; 2],
}

const _: () = assert!(
    std::mem::size_of::<SvoTransformData>() == 32,
    "SvoTransformData must be 32 bytes"
);

// ---------------------------------------------------------------------------
// SvoTransformBuffer — GPU uniform buffer resource
// ---------------------------------------------------------------------------

/// GPU uniform buffer holding [`SvoTransformData`], updated each frame.
///
/// Extracted to the render sub-app each frame so render nodes can read it.
#[derive(Resource, Clone)]
pub struct SvoTransformBuffer(pub Buffer);

impl ExtractResource for SvoTransformBuffer {
    type Source = SvoTransformBuffer;
    fn extract_resource(source: &Self::Source) -> Self {
        source.clone()
    }
}

impl SvoTransformBuffer {
    pub fn new(device: &RenderDevice) -> Self {
        // Allocate 256 bytes for uniform buffer alignment requirements.
        Self(device.create_buffer(&BufferDescriptor {
            label: Some("svo_transform_uniform"),
            size: 256,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }))
    }
}

// ---------------------------------------------------------------------------
// Init / update systems
// ---------------------------------------------------------------------------

/// One-shot system: create `SvoTransformBuffer` once `RenderDevice` is ready.
pub fn init_svo_transform(
    mut commands: Commands,
    device: Option<Res<RenderDevice>>,
    existing: Option<Res<SvoTransformBuffer>>,
) {
    if existing.is_some() {
        return;
    }
    let Some(device) = device else { return };
    commands.insert_resource(SvoTransformBuffer::new(&device));
}

/// Per-frame system: compute and upload the SVO transform from `VoxelWorld`.
///
/// The SVO root covers a cube of side `2^max_depth` world units with its
/// min-corner at the world origin `(0, 0, 0)`.
pub fn update_svo_transform(
    voxel_world: Option<Res<VoxelWorld>>,
    transform_buf: Option<Res<SvoTransformBuffer>>,
    render_queue: Option<Res<RenderQueue>>,
) {
    let Some(voxel_world) = voxel_world else { return };
    let Some(transform_buf) = transform_buf else { return };
    let Some(render_queue) = render_queue else { return };

    let origin = voxel_world.origin();
    let world_size = voxel_world.world_size();
    let data = SvoTransformData {
        origin: [origin.x, origin.y, origin.z],
        world_size,
        inv_world_size: 1.0 / world_size,
        max_depth: voxel_world.max_depth,
        _pad: [0; 2],
    };

    render_queue.write_buffer(&transform_buf.0, 0, bytemuck::bytes_of(&data));
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn svo_transform_data_size() {
        assert_eq!(std::mem::size_of::<SvoTransformData>(), 32);
    }

    #[test]
    fn world_to_svo_origin_maps_to_zero() {
        // Simulate the WGSL `world_to_svo` function: (p - origin) * inv_world_size
        let world_size = 256.0_f32;
        let origin = [0.0_f32, 0.0, 0.0];
        let inv = 1.0 / world_size;
        let p = [0.0_f32, 0.0, 0.0]; // origin
        let svo = [(p[0] - origin[0]) * inv, (p[1] - origin[1]) * inv, (p[2] - origin[2]) * inv];
        assert_eq!(svo, [0.0, 0.0, 0.0]);

        let p_far = [world_size, world_size, world_size];
        let svo_far = [
            (p_far[0] - origin[0]) * inv,
            (p_far[1] - origin[1]) * inv,
            (p_far[2] - origin[2]) * inv,
        ];
        assert_eq!(svo_far, [1.0, 1.0, 1.0]);
    }

    #[test]
    fn bit_decode_first_child_masks_interior_flag() {
        // first_child_index = (child_pointer >> 8) & 0x7FFFFF
        // Interior flag is bit 31 → after >> 8, bit 23. Mask removes it.
        // Encode: INTERIOR_FLAG | (first_child=5 << 8) | child_mask=5
        let interior_node_child_pointer: u32 = 0x8000_0000 | (5 << 8) | 5; // INTERIOR_FLAG + first_child=5, mask=5
        let first_child = (interior_node_child_pointer >> 8) & 0x7F_FFFF;
        assert_eq!(first_child, 5);

        let child_mask = interior_node_child_pointer & 0xFF;
        assert_eq!(child_mask, 5);
    }
}
