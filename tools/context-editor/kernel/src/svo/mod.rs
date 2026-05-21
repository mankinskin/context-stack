//! Sparse Voxel Octree (SVO) — structural authority for physics and rendering.
//!
//! Implements T7a: VoxelWorld API — Octree Data Structure, Manipulation, and
//! Dirty-Range Tracking.
//!
//! # Design
//!
//! The SVO is the **single source of truth** for world geometry:
//! - Physics collision uses SVO ray queries.
//! - Gaussian splatting reads the FRONT GPU buffer (see [`crate::gpu`]).
//! - Editing APIs write to [`VoxelWorld::nodes`] and mark dirty byte ranges.
//!
//! Gaussians are **ephemeral** — regenerated from the SVO every frame.

use bevy::prelude::*;

mod interior;
#[cfg(test)]
mod tests;
mod traversal;
mod types;

pub mod paging;
pub mod upload;

pub use types::{
    OctreeNode,
    VoxelMaterial,
};

// ---------------------------------------------------------------------------
// VoxelWorld
// ---------------------------------------------------------------------------

/// Bevy resource holding the full Sparse Voxel Octree.
///
/// All mutation goes through the manipulation API, which marks dirty byte
/// ranges for the GPU upload system (T7b).
#[derive(Resource)]
pub struct VoxelWorld {
    /// Flat array of all octree nodes.
    pub nodes: Vec<OctreeNode>,
    /// Index of the root node in `nodes`.
    pub root_index: u32,
    /// Maximum octree depth (leaf level).
    pub max_depth: u32,
    /// Pending dirty byte ranges `(byte_start, byte_end_exclusive)`.
    pub dirty_ranges: Vec<(usize, usize)>,
    /// Set when any voxel is added/removed; cleared after `propagate_colors_up()`
    /// runs in the upload system (Phase 4a).
    pub needs_color_propagation: bool,
}

impl Default for VoxelWorld {
    fn default() -> Self {
        Self::new(8)
    }
}

impl VoxelWorld {
    /// Create an empty octree with the given max depth.
    pub fn new(max_depth: u32) -> Self {
        let root = OctreeNode::default();
        Self {
            nodes: vec![root],
            root_index: 0,
            max_depth,
            dirty_ranges: Vec::new(),
            needs_color_propagation: false,
        }
    }

    // -----------------------------------------------------------------------
    // Manipulation API
    // -----------------------------------------------------------------------

    /// Set the voxel at `pos` to `material`.
    ///
    /// Subdivides the octree down to `max_depth` as needed and marks the
    /// modified node dirty for GPU upload.
    ///
    /// Also updates the INTERIOR_FLAG on this voxel and all 6 face-neighbors:
    /// a voxel is interior if all 6 axis-aligned neighbors are occupied.
    pub fn set_voxel(
        &mut self,
        pos: IVec3,
        material: VoxelMaterial,
    ) {
        let node_idx = self.descend_and_allocate(
            pos.as_uvec3(),
            0,
            self.root_index as usize,
            0,
        );
        self.nodes[node_idx].color_data = material.pack();
        // Clear any stale interior flag from before (will be recalculated).
        self.nodes[node_idx].child_pointer &= !OctreeNode::INTERIOR_FLAG;
        self.mark_dirty(node_idx);
        self.reclassify_interior_around(pos);
        // Internal node average colors need refreshing after geometry changes.
        self.needs_color_propagation = true;
    }

    /// Remove the voxel at `pos` (set to empty/transparent).
    ///
    /// Clears the INTERIOR_FLAG from all 6 face-neighbors since they are now
    /// exposed to empty space.
    pub fn remove_voxel(
        &mut self,
        pos: IVec3,
    ) {
        if let Some(node_idx) = self.descend_to(pos) {
            self.nodes[node_idx].color_data = 0;
            self.nodes[node_idx].child_pointer &= !OctreeNode::INTERIOR_FLAG;
            self.mark_dirty(node_idx);
            self.needs_color_propagation = true;

            // Neighbors can no longer be interior — clear their flags.
            let max_c = (1i32 << self.max_depth) - 1;
            let mut to_clear: Vec<usize> = Vec::new();
            for &dir in &[
                IVec3::X,
                -IVec3::X,
                IVec3::Y,
                -IVec3::Y,
                IVec3::Z,
                -IVec3::Z,
            ] {
                let np = pos + dir;
                if np.x < 0
                    || np.y < 0
                    || np.z < 0
                    || np.x > max_c
                    || np.y > max_c
                    || np.z > max_c
                {
                    continue;
                }
                if let Some(nidx) = self.descend_to(np) {
                    if self.nodes[nidx].color_data != 0 {
                        to_clear.push(nidx);
                    }
                }
            }
            for nidx in to_clear {
                self.nodes[nidx].child_pointer &= !OctreeNode::INTERIOR_FLAG;
                self.mark_dirty(nidx);
            }
        }
    }

    /// Paint all voxels within `radius` of `center` with `material`.
    ///
    /// Returns the number of voxels modified.
    pub fn apply_sdf_brush(
        &mut self,
        center: Vec3,
        radius: f32,
        material: VoxelMaterial,
    ) -> u32 {
        let mut count = 0u32;
        let min = (center - Vec3::splat(radius)).floor().as_ivec3();
        let max = (center + Vec3::splat(radius)).ceil().as_ivec3();
        for z in min.z..=max.z {
            for y in min.y..=max.y {
                for x in min.x..=max.x {
                    let pos = IVec3::new(x, y, z);
                    let dist =
                        (pos.as_vec3() + Vec3::splat(0.5) - center).length();
                    if dist <= radius {
                        self.set_voxel(pos, material);
                        count += 1;
                    }
                }
            }
        }
        count
    }

    /// Remove all voxels within `radius` of `center`.
    ///
    /// Returns the number of voxels cleared.
    pub fn carve_sdf_brush(
        &mut self,
        center: Vec3,
        radius: f32,
    ) -> u32 {
        let mut count = 0u32;
        let min = (center - Vec3::splat(radius)).floor().as_ivec3();
        let max = (center + Vec3::splat(radius)).ceil().as_ivec3();
        for z in min.z..=max.z {
            for y in min.y..=max.y {
                for x in min.x..=max.x {
                    let pos = IVec3::new(x, y, z);
                    let dist =
                        (pos.as_vec3() + Vec3::splat(0.5) - center).length();
                    if dist <= radius {
                        self.remove_voxel(pos);
                        count += 1;
                    }
                }
            }
        }
        count
    }

    // -----------------------------------------------------------------------
    // Dirty-range tracking
    // -----------------------------------------------------------------------

    pub fn mark_dirty(
        &mut self,
        node_idx: usize,
    ) {
        const STRIDE: usize = 8; // size_of::<OctreeNode>()
        let byte_start = node_idx * STRIDE;
        self.dirty_ranges.push((byte_start, byte_start + STRIDE));
    }

    /// Drain dirty ranges, returning merged sorted ranges for GPU upload.
    ///
    /// Clears internal state. Call once per frame before `svo_upload_system`.
    pub fn take_dirty_ranges(&mut self) -> Vec<(usize, usize)> {
        if self.dirty_ranges.is_empty() {
            return Vec::new();
        }
        self.dirty_ranges.sort_by_key(|r| r.0);
        let merged = merge_ranges(&self.dirty_ranges);
        self.dirty_ranges.clear();
        merged
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Descend from `node_idx` towards `pos`, allocating child slots as needed.
    fn descend_and_allocate(
        &mut self,
        pos: UVec3,
        depth: u32,
        node_idx: usize,
        origin_bits: u32,
    ) -> usize {
        if depth >= self.max_depth {
            return node_idx;
        }
        let half = 1u32 << (self.max_depth - depth - 1);
        let cx = (pos.x >> (self.max_depth - depth - 1)) & 1 != 0;
        let cy = (pos.y >> (self.max_depth - depth - 1)) & 1 != 0;
        let cz = (pos.z >> (self.max_depth - depth - 1)) & 1 != 0;
        let _ = half; // used above via shift
        let slot = child_index(cx, cy, cz);
        let bit = 1u8 << slot;

        let child_mask = self.nodes[node_idx].child_mask();
        if child_mask & bit == 0 {
            // Reuse existing 8-slot block if children already exist,
            // otherwise allocate a fresh block.
            let first_child = if child_mask == 0 {
                let fc = self.nodes.len();
                self.nodes.extend_from_slice(&[OctreeNode::default(); 8]);
                fc
            } else {
                self.nodes[node_idx].first_child_index()
            };
            let new_mask = child_mask | bit;
            self.nodes[node_idx].child_pointer =
                (first_child as u32) << 8 | new_mask as u32;
            // Mark the parent dirty so the GPU buffer gets the updated child_pointer.
            // The ray march shader traverses the tree hierarchically and needs correct
            // child_mask + first_child data at every internal node, not just leaves.
            self.mark_dirty(node_idx);
        }

        let first_child = self.nodes[node_idx].first_child_index();
        let child_idx = first_child + slot;
        self.descend_and_allocate(pos, depth + 1, child_idx, origin_bits)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Compute the 0-7 child slot index from three axis flags.
///
/// Convention: bit 0 = X, bit 1 = Y, bit 2 = Z.
#[inline]
fn child_index(
    cx: bool,
    cy: bool,
    cz: bool,
) -> usize {
    (cx as usize) | ((cy as usize) << 1) | ((cz as usize) << 2)
}

/// Merge overlapping/adjacent byte ranges.
fn merge_ranges(sorted: &[(usize, usize)]) -> Vec<(usize, usize)> {
    let mut out: Vec<(usize, usize)> = Vec::new();
    for &(start, end) in sorted {
        if let Some(last) = out.last_mut() {
            if start <= last.1 {
                last.1 = last.1.max(end);
                continue;
            }
        }
        out.push((start, end));
    }
    out
}
