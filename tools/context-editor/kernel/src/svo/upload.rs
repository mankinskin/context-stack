//! Double-buffered SVO GPU upload systems.
//!
//! Implements T7b: Double-Buffered SVO Upload — BACK-Buffer Write + Swap System.
//! Extended by Phase 4a with paged upload via [`PageManager`].
//!
//! # Systems (run in [`bevy::app::PostUpdate`] schedule)
//!
//! ```text
//! init_svo_buffer_system
//! init_page_table_system
//! svo_resize_system  →  svo_paged_upload_system  →  double_buffer_swap_system
//! ```
//!
//! - **`svo_resize_system`**: Recreates both buffers when the octree outgrows capacity.
//! - **`svo_paged_upload_system`**: Runs color propagation if needed, builds the
//!   compact packed GPU buffer + page table via [`PageManager`], and uploads both.
//! - **`double_buffer_swap_system`**: Pointer-flips FRONT ↔ BACK (`< 0.01 ms`).

use bevy::{
    prelude::*,
    render::renderer::{RenderDevice, RenderQueue},
};
use bytemuck::cast_slice;

use crate::gpu::{SvoDoubleBuffer, SvoPageTableBuffer};
use crate::svo::VoxelWorld;
use crate::svo::paging::PageManager;

pub use paging_resource::SvoPageManagerResource;

// ---------------------------------------------------------------------------
// PageManager Bevy resource wrapper
// ---------------------------------------------------------------------------

mod paging_resource {
    use super::PageManager;
    use bevy::prelude::Resource;

    /// Wraps [`PageManager`] as a Bevy resource so it can be accessed from systems.
    #[derive(Resource)]
    pub struct SvoPageManagerResource(pub PageManager);

    impl Default for SvoPageManagerResource {
        fn default() -> Self {
            Self(PageManager::new(4))
        }
    }
}

// ---------------------------------------------------------------------------
// Flag: tracks whether svo_paged_upload_system wrote data this frame
// ---------------------------------------------------------------------------

/// When `true`, `double_buffer_swap_system` performs the FRONT ↔ BACK swap.
/// Reset to `false` after each swap.
#[derive(Resource, Default)]
pub struct SvoUploadedThisFrame(pub bool);

// ---------------------------------------------------------------------------
// Bevy Plugin
// ---------------------------------------------------------------------------

/// Registers the SVO upload systems into the Bevy app.
pub struct SvoUploadPlugin;

impl Plugin for SvoUploadPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SvoUploadedThisFrame>();
        app.init_resource::<SvoPageManagerResource>();
        app.add_systems(
            PostUpdate,
            (
                init_svo_buffer_system,
                init_page_table_system,
                (
                    svo_resize_system,
                    svo_paged_upload_system,
                    double_buffer_swap_system,
                )
                .chain()
                .run_if(|svo: Option<Res<crate::gpu::SvoDoubleBuffer>>| svo.is_some()),
            ),
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
            commands.insert_resource(SvoDoubleBuffer::new(
                &device,
                crate::gpu::SVO_CAPACITY_NODES,
            ));
        }
    }
}

/// Initialise the `SvoPageTableBuffer` once `RenderDevice` becomes available.
pub fn init_page_table_system(
    mut commands: Commands,
    device: Option<Res<RenderDevice>>,
    existing: Option<Res<SvoPageTableBuffer>>,
) {
    if existing.is_none() {
        if let Some(device) = device {
            commands.insert_resource(SvoPageTableBuffer::new(&device, 0));
        }
    }
}

/// Grow both GPU buffers when the octree overflows current capacity.
///
/// Doubling on resize amortises future allocations.  The packed GPU buffer
/// is always ≤ the flat CPU buffer, so comparing against `nodes.len()` is
/// conservative and correct.
pub fn svo_resize_system(
    voxel_world: Res<VoxelWorld>,
    mut svo_buffer: ResMut<SvoDoubleBuffer>,
    device: Res<RenderDevice>,
) {
    if voxel_world.nodes.len() > svo_buffer.capacity_nodes {
        let new_capacity =
            (voxel_world.nodes.len() * 2).max(crate::gpu::SVO_CAPACITY_NODES);
        *svo_buffer = SvoDoubleBuffer::new(&device, new_capacity);
    }
}

/// Rebuild the packed GPU buffer + page table and upload both.
///
/// Phase 4a paged upload:
/// 1. If color propagation is needed (voxel modified), run `propagate_lod_colors`.
/// 2. Build compact packed GPU nodes + page table via `PageManager`.
/// 3. Upload packed nodes to the BACK buffer of `SvoDoubleBuffer`.
/// 4. Upload page table to `SvoPageTableBuffer`.
///
/// If no voxels are dirty and color propagation is not pending, nothing
/// is uploaded and the swap is skipped.
pub fn svo_paged_upload_system(
    mut voxel_world: ResMut<VoxelWorld>,
    svo_buffer: Option<Res<SvoDoubleBuffer>>,
    page_table_buf: Option<Res<SvoPageTableBuffer>>,
    render_queue: Res<RenderQueue>,
    device: Res<RenderDevice>,
    mut uploaded_flag: ResMut<SvoUploadedThisFrame>,
    mut page_manager: ResMut<SvoPageManagerResource>,
    mut commands: Commands,
) {
    // Drain dirty ranges to check if anything changed.
    let dirty = voxel_world.take_dirty_ranges();
    let color_propagation_needed = voxel_world.needs_color_propagation;

    if dirty.is_empty() && !color_propagation_needed {
        return; // nothing to do
    }

    // Step 1: propagate average colors up the tree for LOD rendering.
    if color_propagation_needed {
        crate::world::svo_lod::propagate_lod_colors(&mut voxel_world);
        voxel_world.needs_color_propagation = false;
    }

    // Step 2: build packed GPU buffer + page table (no frustum culling for
    // correctness; Phase 4a frustum culling is applied here when camera data
    // is available — for now all pages are always resident).
    let (gpu_nodes, page_table_data) = page_manager.0.build(&voxel_world, None);
    let packed_bytes: &[u8] = cast_slice(&gpu_nodes);

    // Step 3: upload packed nodes to BACK buffer.
    let Some(svo_buffer) = svo_buffer else { return };
    let target = svo_buffer.write_target();

    // Ensure the buffer is large enough for the packed nodes.
    let packed_node_count = gpu_nodes.len();
    if packed_node_count > svo_buffer.capacity_nodes {
        // This shouldn't happen (packed ≤ flat), but handle gracefully.
        return;
    }

    render_queue.write_buffer(target, 0, packed_bytes);

    // Step 4: upload page table.  Recreate the buffer if the page count grew.
    let need_new_pt_buf = page_table_buf
        .as_ref()
        .map(|b| b.capacity < page_table_data.len())
        .unwrap_or(true);

    let pt_size = page_table_data.len();
    if need_new_pt_buf && pt_size > 0 {
        commands.insert_resource(SvoPageTableBuffer::new(&device, pt_size));
    }

    if !page_table_data.is_empty() {
        if let Some(pt_buf) = page_table_buf {
            let pt_bytes: &[u8] = cast_slice(&page_table_data);
            render_queue.write_buffer(&pt_buf.buffer, 0, pt_bytes);
        }
    }

    uploaded_flag.0 = true;
}

/// Swap BACK → FRONT so the Gaussian generator reads fresh data next frame.
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
        for (start, end) in &ranges {
            assert!(end > start, "range must be non-empty: ({start}, {end})");
        }
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

    #[test]
    fn needs_color_propagation_set_on_set_voxel() {
        let mut world = VoxelWorld::new(4);
        assert!(!world.needs_color_propagation);
        world.set_voxel(IVec3::ZERO, VoxelMaterial::new(255, 0, 0, 16));
        assert!(world.needs_color_propagation);
    }
}
