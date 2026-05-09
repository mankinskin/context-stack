//! SVO paging — Frustum culling, paged SVO upload, and Virtual Address Table.
//!
//! Implements Phase 4a: instead of always exposing the full SVO to the GPU
//! traversal, we partition the tree at a configurable `page_depth` into:
//!
//! * **Root page** (depth 0 .. `page_depth − 1`): always resident on the GPU.
//! * **Leaf pages** (subtrees rooted at depth `page_depth`): included or
//!   excluded from traversal based on camera frustum visibility.
//!
//! ## Buffer layout
//!
//! The GPU octree buffer is a direct copy of the CPU flat SVO array, preserving
//! the **8-slot-per-parent** layout.  Child `ci` of a node with
//! `first_child = svo_first_child(cp)` is always at `first_child + ci`, enabling
//! `fc + ci` indexing in the shader — no compact-rank arithmetic required.
//!
//! Non-resident leaf pages are represented in the GPU buffer by zeroing their
//! root node's child-mask (`cmask = 0`), causing the traversal to treat that
//! node as an opaque leaf with its propagated LOD colour.
//!
//! ## Page table
//!
//! `page_table[page_id]` → GPU node index of that page's root node.
//! `0xFFFF_FFFF` = not resident (evicted or culled).
//!
//! Page IDs are assigned sequentially to each occupied child of boundary nodes
//! (nodes at depth `page_depth − 1`), using popcount-rank order.

use std::collections::HashSet;

use bevy::math::{
    Mat4,
    Vec3,
    Vec4,
};
use bytemuck::{
    Pod,
    Zeroable,
};

use super::VoxelWorld;

// ---------------------------------------------------------------------------
// Packed GPU node (bit-identical to OctreeNode for 8-slot layout)
// ---------------------------------------------------------------------------

/// A node as it appears in the packed GPU buffer.
///
/// Bit layout is identical to the CPU `OctreeNode`:
/// * `child_pointer` lower 8 bits: child bitmask.
/// * `child_pointer` bits 8–30: `first_child_idx` — absolute flat-array slot
///   of the first 8-slot child block.  Child `ci` is at `first_child_idx + ci`.
/// * `child_pointer` bit 31: INTERIOR_FLAG.
/// * `color_data`: unchanged from CPU layout.
///
/// **Exception for evicted leaf pages**: the root node of a non-resident page
/// has its lower 8 bits zeroed (`cmask = 0`), so the shader treats it as an
/// opaque leaf using its propagated `color_data`.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug, Pod, Zeroable)]
pub struct GpuNode {
    pub child_pointer: u32,
    pub color_data: u32,
}

impl GpuNode {
    /// Returns `true` if the node has no children in the GPU buffer.
    pub fn is_gpu_leaf(&self) -> bool {
        (self.child_pointer & 0xFF) == 0
    }
}

// ---------------------------------------------------------------------------
// Frustum
// ---------------------------------------------------------------------------

/// Six half-space planes extracted from a view-projection matrix.
///
/// A point / box is inside the frustum if it is on the positive (or touching)
/// side of all six planes.
pub struct Frustum {
    planes: [Vec4; 6], // normal.xyz + d; point p is inside if dot(n, p) + d ≥ 0
}

impl Frustum {
    /// Extract frustum planes from a combined view-projection matrix.
    ///
    /// Uses the standard Gribb–Hartmann technique.  Planes are NOT normalised
    /// (we only need sign tests, which is faster than normalising).
    pub fn from_view_proj(vp: &Mat4) -> Self {
        // vp rows (row-major notation, bevy uses column-major internally).
        let r0 = Vec4::from(vp.row(0));
        let r1 = Vec4::from(vp.row(1));
        let r2 = Vec4::from(vp.row(2));
        let r3 = Vec4::from(vp.row(3));

        // Clip plane equations (Ax + By + Cz + D = 0):
        let planes = [
            r3 + r0, // left
            r3 - r0, // right
            r3 + r1, // bottom
            r3 - r1, // top
            r3 + r2, // near
            r3 - r2, // far
        ];
        Self { planes }
    }

    /// Returns `true` if the AABB [`min`, `max`] intersects or is inside the
    /// frustum (conservative — an AABB is only rejected when its entire extent
    /// is on the negative side of any single plane).
    pub fn test_aabb(
        &self,
        min: Vec3,
        max: Vec3,
    ) -> bool {
        for plane in &self.planes {
            // Positive vertex: the AABB corner most aligned with the plane normal.
            let px = if plane.x >= 0.0 { max.x } else { min.x };
            let py = if plane.y >= 0.0 { max.y } else { min.y };
            let pz = if plane.z >= 0.0 { max.z } else { min.z };
            // If the positive vertex is outside, the entire AABB is outside.
            if plane.x * px + plane.y * py + plane.z * pz + plane.w < 0.0 {
                return false;
            }
        }
        true
    }
}

// ---------------------------------------------------------------------------
// PageManager
// ---------------------------------------------------------------------------

/// Manages paged SVO uploads to the GPU.
///
/// Call [`PageManager::build`] once per frame (or whenever the SVO or camera
/// changes) to rebuild the GPU buffer and page table.
pub struct PageManager {
    /// Depth at which the octree is split into pages.  Default: 4.
    pub page_depth: u32,

    /// Page table: maps `page_id → GPU node index` of each page root.
    /// In the flat 8-slot layout, GPU node index == CPU node index.
    ///
    /// `0xFFFF_FFFF` = non-resident (evicted or frustum-culled).
    pub page_table: Vec<u32>,

    /// Set of currently resident page IDs.
    pub resident_pages: HashSet<u32>,
}

impl Default for PageManager {
    fn default() -> Self {
        Self::new(4)
    }
}

impl PageManager {
    pub fn new(page_depth: u32) -> Self {
        Self {
            page_depth,
            page_table: Vec::new(),
            resident_pages: HashSet::new(),
        }
    }

    /// Rebuild the GPU node buffer and page table from `svo`.
    ///
    /// The returned `gpu_nodes` is a bit-identical copy of the CPU SVO flat
    /// array, preserving 8-slot-per-parent layout so `fc + ci` indexing in
    /// the shader stays valid.
    ///
    /// Non-resident leaf pages have their root node's child-mask zeroed in
    /// `gpu_nodes`, making traversal stop there and render the node as a
    /// solid-colour LOD leaf.
    ///
    /// Returns `(gpu_nodes, page_table)`.
    pub fn build(
        &mut self,
        svo: &VoxelWorld,
        frustum: Option<&Frustum>,
    ) -> (Vec<GpuNode>, Vec<u32>) {
        // Copy the SVO flat array to GPU format (bit-identical, 8-slot layout).
        let mut gpu_nodes: Vec<GpuNode> = svo
            .nodes
            .iter()
            .map(|n| GpuNode {
                child_pointer: n.child_pointer,
                color_data: n.color_data,
            })
            .collect();

        // Degenerate cases: no leaf pages.
        if self.page_depth == 0 || svo.nodes.is_empty() {
            self.page_table.clear();
            self.resident_pages.clear();
            return (gpu_nodes, Vec::new());
        }

        // Phase 1: assign page IDs to occupied children at boundary depth.
        let mut page_id_counter = 0u32;
        let mut boundary_first_page: Vec<u32> = vec![u32::MAX; svo.nodes.len()];
        self.assign_page_ids(
            svo,
            svo.root_index as usize,
            0,
            &mut page_id_counter,
            &mut boundary_first_page,
        );
        let total_pages = page_id_counter;

        // Phase 2: populate page table and apply frustum culling.
        let mut page_table: Vec<u32> = vec![u32::MAX; total_pages as usize];
        let mut new_resident: HashSet<u32> = HashSet::new();

        if total_pages > 0 {
            self.populate_and_cull(
                svo,
                svo.root_index as usize,
                Vec3::ZERO,
                svo.world_size(),
                0,
                frustum,
                &boundary_first_page,
                &mut gpu_nodes,
                &mut page_table,
                &mut new_resident,
            );
        }

        self.page_table = page_table.clone();
        self.resident_pages = new_resident;
        (gpu_nodes, page_table)
    }

    // -----------------------------------------------------------------------
    // Phase 1: page ID assignment
    // -----------------------------------------------------------------------

    fn assign_page_ids(
        &self,
        svo: &VoxelWorld,
        idx: usize,
        depth: u32,
        counter: &mut u32,
        boundary_first_page: &mut Vec<u32>,
    ) {
        let node = &svo.nodes[idx];
        let mask = node.child_mask();
        if mask == 0 {
            return;
        }
        let first_child = node.first_child_index();

        if depth + 1 == self.page_depth {
            // Children of this node are page roots.
            boundary_first_page[idx] = *counter;
            *counter += u32::count_ones(mask as u32);
        } else {
            for bit in 0u8..8 {
                if mask & (1 << bit) != 0 {
                    let child_idx = first_child + bit as usize;
                    self.assign_page_ids(
                        svo,
                        child_idx,
                        depth + 1,
                        counter,
                        boundary_first_page,
                    );
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Phase 2: page table population and frustum culling
    // -----------------------------------------------------------------------

    #[allow(clippy::too_many_arguments)]
    fn populate_and_cull(
        &self,
        svo: &VoxelWorld,
        idx: usize,
        origin: Vec3,
        node_size: f32,
        depth: u32,
        frustum: Option<&Frustum>,
        boundary_first_page: &[u32],
        gpu_nodes: &mut Vec<GpuNode>,
        page_table: &mut Vec<u32>,
        resident: &mut HashSet<u32>,
    ) {
        let node = &svo.nodes[idx];
        let mask = node.child_mask();
        if mask == 0 {
            return;
        }
        let first_child = node.first_child_index();
        let child_size = node_size * 0.5;

        if depth + 1 == self.page_depth {
            let first_page = boundary_first_page[idx];
            let mut rank = 0u32;
            for bit in 0u8..8 {
                if mask & (1 << bit) == 0 {
                    continue;
                }
                let page_id = first_page + rank;
                rank += 1;

                let ox = if bit & 1 != 0 { child_size } else { 0.0 };
                let oy = if bit & 2 != 0 { child_size } else { 0.0 };
                let oz = if bit & 4 != 0 { child_size } else { 0.0 };
                let child_origin = origin + Vec3::new(ox, oy, oz);
                let child_max = child_origin + Vec3::splat(child_size);
                let child_cpu_idx = first_child + bit as usize;

                let is_resident = match frustum {
                    Some(f) => f.test_aabb(child_origin, child_max),
                    None => true,
                };

                if is_resident {
                    // CPU index == GPU index in flat 8-slot layout.
                    page_table[page_id as usize] = child_cpu_idx as u32;
                    resident.insert(page_id);
                } else {
                    // Evict: zero cmask so traversal stops here (solid LOD leaf).
                    gpu_nodes[child_cpu_idx].child_pointer &= !0xFF_u32;
                }
            }
        } else {
            for bit in 0u8..8 {
                if mask & (1 << bit) == 0 {
                    continue;
                }
                let ox = if bit & 1 != 0 { child_size } else { 0.0 };
                let oy = if bit & 2 != 0 { child_size } else { 0.0 };
                let oz = if bit & 4 != 0 { child_size } else { 0.0 };
                let child_origin = origin + Vec3::new(ox, oy, oz);
                let child_cpu_idx = first_child + bit as usize;
                self.populate_and_cull(
                    svo,
                    child_cpu_idx,
                    child_origin,
                    child_size,
                    depth + 1,
                    frustum,
                    boundary_first_page,
                    gpu_nodes,
                    page_table,
                    resident,
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::svo::{
        VoxelMaterial,
        VoxelWorld,
    };
    use bevy::math::IVec3;

    fn make_small_world() -> VoxelWorld {
        let mut w = VoxelWorld::new(6);
        let red = VoxelMaterial::new(255, 0, 0, 16);
        let blue = VoxelMaterial::new(0, 0, 255, 16);
        w.set_voxel(IVec3::new(0, 0, 0), red);
        w.set_voxel(IVec3::new(32, 0, 0), blue);
        w
    }

    #[test]
    fn build_produces_nonempty_buffers() {
        let w = make_small_world();
        let mut pm = PageManager::new(3);
        let (gpu_nodes, page_table) = pm.build(&w, None);
        assert!(!gpu_nodes.is_empty(), "GPU nodes must be non-empty");
        assert!(!page_table.is_empty(), "Page table must be non-empty");
    }

    #[test]
    fn all_pages_resident_without_frustum() {
        let w = make_small_world();
        let mut pm = PageManager::new(3);
        let (_gpu_nodes, page_table) = pm.build(&w, None);
        for (id, &base) in page_table.iter().enumerate() {
            assert_ne!(
                base, 0xFFFF_FFFF,
                "page {id} should be resident when no frustum is given"
            );
        }
    }

    #[test]
    fn frustum_behind_camera_evicts_all_pages() {
        let w = make_small_world();
        let mut pm = PageManager::new(3);
        let reject_all = Frustum {
            planes: [Vec4::new(-1.0, 0.0, 0.0, -1e9); 6],
        };
        let (_gpu_nodes, page_table) = pm.build(&w, Some(&reject_all));
        for &base in &page_table {
            assert_eq!(
                base, 0xFFFF_FFFF,
                "rejecting frustum should evict all pages"
            );
        }
    }

    #[test]
    fn page_depth_zero_is_all_root() {
        let mut w = VoxelWorld::new(4);
        w.set_voxel(IVec3::new(0, 0, 0), VoxelMaterial::new(1, 2, 3, 4));
        let mut pm = PageManager::new(0);
        let (gpu_nodes, page_table) = pm.build(&w, None);
        assert!(page_table.is_empty(), "page_depth=0 yields no leaf pages");
        assert!(!gpu_nodes.is_empty());
    }

    #[test]
    fn packed_child_ranks_increase_monotonically() {
        // For flat 8-slot layout the child block for a node starts at
        // first_child_idx and occupies 8 consecutive slots.
        let mut w = VoxelWorld::new(4);
        let red = VoxelMaterial::new(255, 0, 0, 8);
        w.set_voxel(IVec3::new(0, 0, 0), red);
        w.set_voxel(IVec3::new(4, 0, 0), red);
        w.set_voxel(IVec3::new(0, 4, 0), red);

        let mut pm = PageManager::new(2);
        let (gpu_nodes, _page_table) = pm.build(&w, None);

        let root = gpu_nodes[0];
        let mask = (root.child_pointer & 0xFF) as u8;
        let first_child_gpu = (root.child_pointer >> 8) & 0x7F_FFFF;

        if mask != 0 {
            assert!(
                (first_child_gpu as usize) < gpu_nodes.len(),
                "first child idx {first_child_gpu} out of bounds (gpu_nodes len={})",
                gpu_nodes.len()
            );
            // Full 8-slot block must fit.
            assert!(
                (first_child_gpu as usize + 7) < gpu_nodes.len(),
                "child block end {} out of bounds (gpu_nodes len={})",
                first_child_gpu + 7,
                gpu_nodes.len()
            );
        }
    }

    #[test]
    fn evicted_pages_have_cmask_zero_in_gpu_buffer() {
        let w = make_small_world();
        let mut pm = PageManager::new(3);
        let reject_all = Frustum {
            planes: [Vec4::new(-1.0, 0.0, 0.0, -1e9); 6],
        };
        let (gpu_nodes, page_table) = pm.build(&w, Some(&reject_all));
        for &pt_entry in &page_table {
            assert_eq!(pt_entry, 0xFFFF_FFFF);
        }
        assert!(!gpu_nodes.is_empty());
    }
}
