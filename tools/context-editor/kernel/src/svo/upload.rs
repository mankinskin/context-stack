//! Double-buffered SVO GPU upload systems.
//!
//! Implements T7b: Double-Buffered SVO Upload — BACK-Buffer Write + Swap System.
//!
//! # Systems (run in [`bevy::app::PostUpdate`] schedule)
//!
//! ```text
//! svo_resize_system  →  svo_upload_system  →  double_buffer_swap_system
//! ```
//!
//! - **`svo_resize_system`**: Recreates both buffers when the octree outgrows capacity.
//! - **`svo_upload_system`**: Writes dirty byte ranges to the BACK buffer.
//! - **`double_buffer_swap_system`**: Pointer-flips FRONT ↔ BACK (`< 0.01 ms`).
//!
//! The GPU reads the **FRONT** buffer; WASM writes to the **BACK** buffer.
//! After the swap, the Gaussian generator picks up fresh octree data next frame.
//! If an edit exceeds one frame, the GPU keeps rendering the stale FRONT at full FPS.

use bevy::{
    prelude::*,
    render::renderer::{RenderDevice, RenderQueue},
};
use bytemuck::cast_slice;

use crate::{gpu::SvoDoubleBuffer, svo::VoxelWorld};

// ---------------------------------------------------------------------------
// Flag: tracks whether svo_upload_system wrote data this frame
// ---------------------------------------------------------------------------

/// When `true`, `double_buffer_swap_system` performs the FRONT ↔ BACK swap.
/// Reset to `false` after each swap.
#[derive(Resource, Default)]
pub struct SvoUploadedThisFrame(pub bool);

// ---------------------------------------------------------------------------
// Bevy Plugin
// ---------------------------------------------------------------------------

/// Registers the three SVO upload systems into the Bevy app.
pub struct SvoUploadPlugin;

impl Plugin for SvoUploadPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SvoUploadedThisFrame>();
        app.add_systems(
            PostUpdate,
            (
                init_svo_buffer_system,
                (
                    svo_resize_system,
                    svo_upload_system,
                    double_buffer_swap_system,
                ).chain().run_if(|svo: Option<Res<crate::gpu::SvoDoubleBuffer>>| svo.is_some()),
            )
        );
    }
}

// ---------------------------------------------------------------------------  
// Systems
// ---------------------------------------------------------------------------  

/// Initialise the `SvoDoubleBuffer` once `RenderDevice` becomes available.
pub fn init_svo_buffer_system(
    mut commands: Commands,
    device: Option<Res<RenderDevice>>,
    svo_buffer: Option<Res<SvoDoubleBuffer>>,
) {
    if svo_buffer.is_none() {
        if let Some(device) = device {
            commands.insert_resource(SvoDoubleBuffer::new(&device, crate::gpu::SVO_CAPACITY_NODES));
        }
    }
}

/// Grow both GPU buffers when the octree overflows current capacity.
///
/// Doubling on resize amortises future allocations. After resize a full
/// re-upload is triggered implicitly: all nodes are dirtied by the caller.
pub fn svo_resize_system(
    voxel_world: Res<VoxelWorld>,
    mut svo_buffer: ResMut<SvoDoubleBuffer>,
    device: Res<RenderDevice>,
) {
    if voxel_world.nodes.len() > svo_buffer.capacity_nodes {
        let new_capacity = (voxel_world.nodes.len() * 2).max(crate::gpu::SVO_CAPACITY_NODES);
        *svo_buffer = SvoDoubleBuffer::new(&device, new_capacity);
        // On the next frame the upload system will be called with whatever
        // dirty ranges exist; callers are responsible for re-marking nodes
        // after an explicit resize.
    }
}

/// Write dirty octree regions to the **BACK** GPU buffer.
///
/// Uses `RenderQueue::write_buffer` for partial uploads — only modified
/// byte ranges are copied. For a 5-radius sphere brush (~500 voxels at
/// leaf depth) this is typically < 1 ms.
pub fn svo_upload_system(
    mut voxel_world: ResMut<VoxelWorld>,
    svo_buffer: Res<SvoDoubleBuffer>,
    render_queue: Res<RenderQueue>,
    mut uploaded_flag: ResMut<SvoUploadedThisFrame>,
) {
    let ranges = voxel_world.take_dirty_ranges();
    if ranges.is_empty() {
        return;
    }

    let node_bytes: &[u8] = cast_slice(&voxel_world.nodes);
    let target = svo_buffer.write_target(); // BACK buffer — GPU not reading this

    for (start, end) in ranges {
        let start = start.min(node_bytes.len());
        let end = end.min(node_bytes.len());
        if start >= end {
            continue;
        }
        render_queue.write_buffer(target, start as u64, &node_bytes[start..end]);
    }

    uploaded_flag.0 = true;
}

/// Swap BACK → FRONT so the Gaussian generator reads fresh data next frame.
///
/// This is a CPU-side pointer flip (`< 0.01 ms`). No GPU synchronisation is
/// needed because wgpu's command queue ordering guarantees that the upload
/// writes complete before the next frame's compute dispatch.
pub fn double_buffer_swap_system(
    mut svo_buffer: ResMut<SvoDoubleBuffer>,
    mut uploaded_flag: ResMut<SvoUploadedThisFrame>,
) {
    if uploaded_flag.0 {
        svo_buffer.swap();
        uploaded_flag.0 = false;
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use bevy::math::IVec3;
    use crate::svo::{VoxelMaterial, VoxelWorld};

    #[test]
    fn upload_system_clears_dirty_on_take() {
        let mut world = VoxelWorld::new(4);
        let mat = VoxelMaterial::new(255, 0, 0, 128);
        world.set_voxel(IVec3::ZERO, mat);
        world.set_voxel(IVec3::new(1, 0, 0), mat);

        let ranges = world.take_dirty_ranges();
        assert!(!ranges.is_empty());
        // All ranges must be non-zero length
        for (start, end) in &ranges {
            assert!(end > start, "range must be non-empty: ({start}, {end})");
        }
        // Second take must be empty
        assert!(world.take_dirty_ranges().is_empty());
    }

    #[test]
    fn ranges_do_not_exceed_node_byte_length() {
        let mut world = VoxelWorld::new(4);
        for z in 0..4i32 {
            for y in 0..4i32 {
                for x in 0..4i32 {
                    world.set_voxel(IVec3::new(x, y, z), VoxelMaterial::new(1, 2, 3, 4));
                }
            }
        }
        let total_bytes = world.nodes.len() * 8; // OctreeNode = 8 bytes
        for (start, end) in world.take_dirty_ranges() {
            assert!(
                end <= total_bytes,
                "range [{start}, {end}) exceeds node buffer size {total_bytes}"
            );
        }
    }
}
