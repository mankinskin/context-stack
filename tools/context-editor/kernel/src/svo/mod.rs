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
use bytemuck::{Pod, Zeroable};

pub mod upload;

// ---------------------------------------------------------------------------
// OctreeNode
// ---------------------------------------------------------------------------

/// A single node in the Sparse Voxel Octree.
///
/// Stored in a flat `Vec<OctreeNode>` that is uploaded verbatim to the GPU.
/// 8 bytes per node (2 × u32) for cache-line efficiency.
///
/// WGSL struct (must stay in sync):
/// ```wgsl
/// struct OctreeNode {
///     child_pointer: u32,  // lower 8 bits = child bitmask; upper 24 = first-child slot index
///     color_data:    u32,  // R8 G8 B8 + roughness5 + metallic1 + reserved2
/// }
/// ```
#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Pod, Zeroable)]
pub struct OctreeNode {
    /// Lower 8 bits: bitmask of which of the 8 children exist.
    /// Upper 24 bits: index (into `VoxelWorld::nodes`) of the first child slot.
    pub child_pointer: u32,
    /// Packed material data. For leaf nodes: R8G8B8 color + roughness/metallic flags.
    pub color_data: u32,
}

impl OctreeNode {
    pub const fn leaf(color_data: u32) -> Self {
        Self { child_pointer: 0, color_data }
    }

    pub fn is_leaf(&self) -> bool {
        self.child_mask() == 0
    }

    pub fn child_mask(&self) -> u8 {
        (self.child_pointer & 0xFF) as u8
    }

    pub fn first_child_index(&self) -> usize {
        (self.child_pointer >> 8) as usize
    }
}

// ---------------------------------------------------------------------------
// VoxelMaterial
// ---------------------------------------------------------------------------

/// Material packed into an `OctreeNode::color_data` u32.
///
/// Bit layout:
/// ```text
///   Bits  0–7:  R (8 bits)
///   Bits  8–15: G (8 bits)
///   Bits 16–23: B (8 bits)
///   Bits 24–28: Roughness (5 bits, 0–31 → 0.0–1.0)
///   Bit  29:    Metallic  (1 bit, 0 = dielectric, 1 = metallic)
///   Bits 30–31: Reserved
/// ```
#[derive(Clone, Copy, Debug, Default)]
pub struct VoxelMaterial {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    /// Perceptual roughness quantised to 5 bits (0–31).
    /// Values above 31 are clamped.
    pub roughness: u8,
    /// Metallic flag (binary: dielectric or metal).
    pub metallic: bool,
}

impl VoxelMaterial {
    pub const fn new(r: u8, g: u8, b: u8, roughness: u8) -> Self {
        Self { r, g, b, roughness, metallic: false }
    }

    pub const fn new_metallic(r: u8, g: u8, b: u8, roughness: u8, metallic: bool) -> Self {
        Self { r, g, b, roughness, metallic }
    }

    /// Pack into a u32 matching the WGSL `unpack_material()` bit layout.
    pub fn pack(self) -> u32 {
        let rough_5 = (self.roughness.min(31)) as u32;
        let metal_bit = if self.metallic { 1u32 } else { 0u32 };
        (self.r as u32)
            | (self.g as u32) << 8
            | (self.b as u32) << 16
            | rough_5 << 24
            | metal_bit << 29
    }

    /// Unpack from a u32 (inverse of [`pack`]).
    pub fn unpack(v: u32) -> Self {
        Self {
            r: v as u8,
            g: (v >> 8) as u8,
            b: (v >> 16) as u8,
            roughness: ((v >> 24) & 0x1F) as u8,
            metallic: ((v >> 29) & 1) != 0,
        }
    }
}

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
        }
    }

    // -----------------------------------------------------------------------
    // Manipulation API
    // -----------------------------------------------------------------------

    /// Set the voxel at `pos` to `material`.
    ///
    /// Subdivides the octree down to `max_depth` as needed and marks the
    /// modified node dirty for GPU upload.
    pub fn set_voxel(&mut self, pos: IVec3, material: VoxelMaterial) {
        let node_idx = self.descend_and_allocate(pos.as_uvec3(), 0, self.root_index as usize, 0);
        self.nodes[node_idx].color_data = material.pack();
        self.mark_dirty(node_idx);
    }

    /// Remove the voxel at `pos` (set to empty/transparent).
    pub fn remove_voxel(&mut self, pos: IVec3) {
        if let Some(node_idx) = self.descend_to(pos) {
            self.nodes[node_idx].color_data = 0;
            self.mark_dirty(node_idx);
        }
    }

    /// Paint all voxels within `radius` of `center` with `material`.
    ///
    /// Returns the number of voxels modified.
    pub fn apply_sdf_brush(&mut self, center: Vec3, radius: f32, material: VoxelMaterial) -> u32 {
        let mut count = 0u32;
        let min = (center - Vec3::splat(radius)).floor().as_ivec3();
        let max = (center + Vec3::splat(radius)).ceil().as_ivec3();
        for z in min.z..=max.z {
            for y in min.y..=max.y {
                for x in min.x..=max.x {
                    let pos = IVec3::new(x, y, z);
                    let dist = (pos.as_vec3() + Vec3::splat(0.5) - center).length();
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
    pub fn carve_sdf_brush(&mut self, center: Vec3, radius: f32) -> u32 {
        let mut count = 0u32;
        let min = (center - Vec3::splat(radius)).floor().as_ivec3();
        let max = (center + Vec3::splat(radius)).ceil().as_ivec3();
        for z in min.z..=max.z {
            for y in min.y..=max.y {
                for x in min.x..=max.x {
                    let pos = IVec3::new(x, y, z);
                    let dist = (pos.as_vec3() + Vec3::splat(0.5) - center).length();
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

    fn mark_dirty(&mut self, node_idx: usize) {
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
    // Traversal
    // -----------------------------------------------------------------------

    /// Traverse to the leaf node at the given position, returning its index.
    ///
    /// Returns `None` if the position is out of bounds or the node is empty.
    pub fn descend_to(&self, pos: IVec3) -> Option<usize> {
        if pos.x < 0 || pos.y < 0 || pos.z < 0 {
            return None;
        }
        let mut idx = self.root_index as usize;
        let mut size = 1u32 << self.max_depth;
        let (mut ox, mut oy, mut oz) = (0u32, 0u32, 0u32);

        for _ in 0..self.max_depth {
            let node = &self.nodes[idx];
            if node.is_leaf() {
                return Some(idx);
            }
            size >>= 1;
            let cx = (pos.x as u32).wrapping_sub(ox) >= size;
            let cy = (pos.y as u32).wrapping_sub(oy) >= size;
            let cz = (pos.z as u32).wrapping_sub(oz) >= size;
            if cx { ox += size; }
            if cy { oy += size; }
            if cz { oz += size; }
            let child_bit = child_index(cx, cy, cz);
            if node.child_mask() & (1 << child_bit) == 0 {
                return None; // child slot empty
            }
            idx = node.first_child_index() + child_bit;
        }
        Some(idx)
    }

    /// CPU-side ray–octree intersection.
    ///
    /// Returns `(hit_position, surface_normal)` of the first occupied voxel,
    /// or `None` if the ray misses or exceeds `max_dist`.
    pub fn raycast(
        &self,
        origin: Vec3,
        direction: Vec3,
        max_dist: f32,
    ) -> Option<(Vec3, Vec3)> {
        let dir = direction.normalize();
        let step = 0.5f32; // step along the ray — refineable
        let mut t = 0.0f32;
        while t < max_dist {
            let p = origin + dir * t;
            let cell = p.floor().as_ivec3();
            if self.descend_to(cell).map(|i| self.nodes[i].color_data != 0).unwrap_or(false) {
                // Approximate surface normal via central differences
                let eps = 0.5;
                let normal = Vec3::new(
                    self.density(cell + IVec3::X) - self.density(cell - IVec3::X),
                    self.density(cell + IVec3::Y) - self.density(cell - IVec3::Y),
                    self.density(cell + IVec3::Z) - self.density(cell - IVec3::Z),
                ) * eps;
                let normal = if normal.length_squared() > 0.0 {
                    -normal.normalize()
                } else {
                    Vec3::Y
                };
                return Some((p, normal));
            }
            t += step;
        }
        None
    }

    fn density(&self, pos: IVec3) -> f32 {
        self.descend_to(pos)
            .map(|i| if self.nodes[i].color_data != 0 { 1.0 } else { 0.0 })
            .unwrap_or(0.0)
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
            // Allocate 8 new child slots
            let first_child = self.nodes.len();
            self.nodes.extend_from_slice(&[OctreeNode::default(); 8]);
            let new_mask = child_mask | bit;
            self.nodes[node_idx].child_pointer =
                (first_child as u32) << 8 | new_mask as u32;
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
fn child_index(cx: bool, cy: bool, cz: bool) -> usize {
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

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn red() -> VoxelMaterial { VoxelMaterial::new(255, 0, 0, 16) }

    #[test]
    fn material_pack_roundtrip() {
        let m = VoxelMaterial::new(10, 20, 30, 25);
        let u = VoxelMaterial::unpack(m.pack());
        assert_eq!(u.r, m.r);
        assert_eq!(u.g, m.g);
        assert_eq!(u.b, m.b);
        assert_eq!(u.roughness, m.roughness);
        assert_eq!(u.metallic, false);
    }

    #[test]
    fn material_pack_roundtrip_metallic() {
        let m = VoxelMaterial::new_metallic(200, 150, 100, 31, true);
        let u = VoxelMaterial::unpack(m.pack());
        assert_eq!(u.r, 200);
        assert_eq!(u.g, 150);
        assert_eq!(u.b, 100);
        assert_eq!(u.roughness, 31);
        assert!(u.metallic);
    }

    #[test]
    fn material_roughness_clamped_to_5_bits() {
        // Values > 31 should be clamped to 31 on pack
        let m = VoxelMaterial::new(0, 0, 0, 255);
        let u = VoxelMaterial::unpack(m.pack());
        assert_eq!(u.roughness, 31);
    }

    #[test]
    fn set_voxel_marks_dirty() {
        let mut world = VoxelWorld::new(4);
        world.set_voxel(IVec3::ZERO, red());
        let ranges = world.take_dirty_ranges();
        assert!(!ranges.is_empty(), "set_voxel must mark dirty ranges");
        // take clears
        assert!(world.take_dirty_ranges().is_empty(), "take_dirty_ranges must clear state");
    }

    #[test]
    fn set_then_descend() {
        let mut world = VoxelWorld::new(4);
        world.set_voxel(IVec3::new(3, 2, 1), red());
        let idx = world.descend_to(IVec3::new(3, 2, 1));
        assert!(idx.is_some(), "descend_to must find voxel after set_voxel");
        assert_ne!(
            world.nodes[idx.unwrap()].color_data,
            0,
            "leaf must have non-zero color_data"
        );
    }

    #[test]
    fn apply_sdf_brush_sphere() {
        let mut world = VoxelWorld::new(5);
        let painted = world.apply_sdf_brush(Vec3::new(8.0, 8.0, 8.0), 3.0, red());
        assert!(painted > 0, "apply_sdf_brush must paint at least one voxel");
    }

    #[test]
    fn carve_sdf_brush() {
        let mut world = VoxelWorld::new(5);
        world.apply_sdf_brush(Vec3::new(8.0, 8.0, 8.0), 4.0, red());
        world.take_dirty_ranges(); // clear
        let carved = world.carve_sdf_brush(Vec3::new(8.0, 8.0, 8.0), 2.0);
        assert!(carved > 0, "carve_sdf_brush must carve at least one voxel");
        let ranges = world.take_dirty_ranges();
        assert!(!ranges.is_empty(), "carve must mark dirty ranges");
    }

    #[test]
    fn merge_ranges_basic() {
        let input = vec![(0, 8), (8, 16), (32, 40)];
        let merged = merge_ranges(&input);
        assert_eq!(merged, vec![(0, 16), (32, 40)]);
    }

    #[test]
    fn child_index_all_combinations() {
        // Verify all 8 combinations produce unique indices 0-7
        let mut seen = [false; 8];
        for cz in [false, true] {
            for cy in [false, true] {
                for cx in [false, true] {
                    let idx = child_index(cx, cy, cz);
                    assert!(!seen[idx], "duplicate child index");
                    seen[idx] = true;
                }
            }
        }
        assert!(seen.iter().all(|&b| b), "all 8 slots must be covered");
    }
}
