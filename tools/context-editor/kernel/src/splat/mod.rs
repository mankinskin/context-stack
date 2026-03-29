//! Voxel Splatting — visual layer generated from the SVO every frame.
//!
//! # Architecture
//!
//! Each occupied SVO leaf is projected to screen as a **voxel splat** using a
//! ray-box SDF distance function with `smoothstep` soft edges, filtered with
//! screen-space EWA derivatives (`fwidth`) for anti-aliasing.
//!
//! ## Data types (T6a)
//!
//! - [`VoxelSplat`] — GPU struct emitted by the compute kernel (one per leaf).
//! - [`SplatParams`] — per-frame uniform driving LOD and position reconstruction.
//!
//! ## Pipeline stages (T6b–T6d)
//!
//! - Sort key construction (T6b)
//! - GPU radix sort (T6c)
//! - Tiled rasteriser with Cook-Torrance PBR (T6d + T6e)

use bytemuck::{Pod, Zeroable};

// ---------------------------------------------------------------------------
// VoxelSplat — mirrors the WGSL struct in voxel_splat_kernel.wgsl
// ---------------------------------------------------------------------------

/// GPU-side voxel splat emitted by the compute kernel.
///
/// One per occupied SVO leaf node. 24 bytes (6 × u32), stored in a
/// `storage` buffer read by downstream sort and rasterise passes.
///
/// WGSL layout:
/// ```wgsl
/// struct VoxelSplat {
///     center_ws:       vec3f,
///     half_extent:     f32,
///     material_packed: u32,
///     _pad:            u32,
/// }
/// ```
#[repr(C)]
#[derive(Clone, Copy, Default, Debug, Pod, Zeroable)]
pub struct VoxelSplat {
    /// World-space center of the voxel.
    pub center_ws: [f32; 3],
    /// Half the side length of the axis-aligned voxel box.
    pub half_extent: f32,
    /// Packed material from `OctreeNode::color_data` (R8 G8 B8 + roughness5 + metallic1 + reserved2).
    pub material_packed: u32,
    pub _pad: u32,
}

/// Byte stride of a single [`VoxelSplat`] in the GPU buffer.
pub const VOXEL_SPLAT_STRIDE: u64 = std::mem::size_of::<VoxelSplat>() as u64; // 24

// ---------------------------------------------------------------------------
// SplatParams — per-frame uniform for the compute kernel
// ---------------------------------------------------------------------------

/// Uniform buffer driving the voxel splat kernel.
///
/// WGSL layout:
/// ```wgsl
/// struct SplatParams {
///     camera_pos:  vec3f,
///     total_nodes: u32,
///     lod_scale:   f32,
///     max_depth:   u32,
///     world_size:  f32,
///     _pad:        f32,
/// }
/// ```
#[repr(C)]
#[derive(Clone, Copy, Default, Debug, Pod, Zeroable)]
pub struct SplatParams {
    pub camera_pos: [f32; 3],
    pub total_nodes: u32,
    pub lod_scale: f32,
    pub max_depth: u32,
    pub world_size: f32,
    pub _pad: f32,
}

/// Byte size of [`SplatParams`] uniform (32 bytes).
pub const SPLAT_PARAMS_SIZE: u64 = std::mem::size_of::<SplatParams>() as u64;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voxel_splat_size_matches_wgsl() {
        // 3 × f32 + 1 × f32 + 1 × u32 + 1 × u32 = 6 × 4 = 24 bytes
        assert_eq!(std::mem::size_of::<VoxelSplat>(), 24);
        assert_eq!(VOXEL_SPLAT_STRIDE, 24);
    }

    #[test]
    fn splat_params_size_matches_wgsl() {
        // 3 × f32 + u32 + f32 + u32 + f32 + f32 = 8 × 4 = 32 bytes
        assert_eq!(std::mem::size_of::<SplatParams>(), 32);
        assert_eq!(SPLAT_PARAMS_SIZE, 32);
    }

    #[test]
    fn voxel_splat_is_pod() {
        // Ensure bytemuck cast works without panic
        let bytes = [0u8; 24];
        let _: &VoxelSplat = bytemuck::from_bytes(&bytes);
    }

    #[test]
    fn splat_params_is_pod() {
        let bytes = [0u8; 32];
        let _: &SplatParams = bytemuck::from_bytes(&bytes);
    }

    /// CPU-side verification of the shader's leaf-counting logic:
    /// occupied leaves (child_mask == 0 && color_data != 0) should produce
    /// exactly one splat each.
    #[test]
    fn expected_splat_count_matches_leaf_nodes() {
        use crate::svo::{VoxelWorld, VoxelMaterial};
        use bevy::math::Vec3;

        let mut world = VoxelWorld::new(4);
        let mat = VoxelMaterial::new(200, 100, 50, 16);

        // Paint a small sphere — known to produce a deterministic set of leaves
        let painted = world.apply_sdf_brush(Vec3::new(4.0, 4.0, 4.0), 2.0, mat);
        assert!(painted > 0);

        // Count occupied leaf nodes (mirrors the shader's skip logic)
        let leaf_count = world
            .nodes
            .iter()
            .filter(|n| n.is_leaf() && n.color_data != 0)
            .count();

        assert_eq!(
            leaf_count, painted as usize,
            "every painted voxel must correspond to exactly one occupied leaf"
        );
    }
}
