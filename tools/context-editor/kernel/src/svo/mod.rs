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
pub mod paging;

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
    /// Bit 31 of `child_pointer`: marks a leaf node as fully surrounded on all 6 faces.
    /// Interior voxels are skipped by the splat kernel — they can never be seen.
    pub const INTERIOR_FLAG: u32 = 0x8000_0000;

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
        // Mask out INTERIOR_FLAG (bit 31) before extracting the 23-bit child index.
        ((self.child_pointer & 0x7FFF_FF00) >> 8) as usize
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
///   Bits 30–31: SDF type  (0=box, 1=sphere, 2=svo-sampled, 3=torus/procedural)
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
    /// SDF shape type encoded in bits 30–31:
    /// 0 = box (default), 1 = sphere, 2 = svo-sampled, 3 = torus.
    pub sdf_type: u8,
}

impl VoxelMaterial {
    pub const fn new(r: u8, g: u8, b: u8, roughness: u8) -> Self {
        Self { r, g, b, roughness, metallic: false, sdf_type: 0 }
    }

    pub const fn new_metallic(r: u8, g: u8, b: u8, roughness: u8, metallic: bool) -> Self {
        Self { r, g, b, roughness, metallic, sdf_type: 0 }
    }

    /// Create a material with an explicit SDF shape type.
    pub const fn new_sdf(r: u8, g: u8, b: u8, roughness: u8, sdf_type: u8) -> Self {
        Self { r, g, b, roughness, metallic: false, sdf_type }
    }

    /// Pack into a u32 matching the WGSL `unpack_material()` bit layout.
    pub const fn pack(self) -> u32 {
        let r = self.roughness;
        let rough_5 = (if r < 31 { r } else { 31 }) as u32;
        let metal_bit = if self.metallic { 1u32 } else { 0u32 };
        let sdf_bits = (self.sdf_type as u32 & 3u32) << 30;
        (self.r as u32)
            | (self.g as u32) << 8
            | (self.b as u32) << 16
            | rough_5 << 24
            | metal_bit << 29
            | sdf_bits
    }

    /// Unpack from a u32 (inverse of [`pack`]).
    pub fn unpack(v: u32) -> Self {
        Self {
            r: v as u8,
            g: (v >> 8) as u8,
            b: (v >> 16) as u8,
            roughness: ((v >> 24) & 0x1F) as u8,
            metallic: ((v >> 29) & 1) != 0,
            sdf_type: ((v >> 30) & 3) as u8,
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
    pub fn set_voxel(&mut self, pos: IVec3, material: VoxelMaterial) {
        let node_idx = self.descend_and_allocate(pos.as_uvec3(), 0, self.root_index as usize, 0);
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
    pub fn remove_voxel(&mut self, pos: IVec3) {
        if let Some(node_idx) = self.descend_to(pos) {
            self.nodes[node_idx].color_data = 0;
            self.nodes[node_idx].child_pointer &= !OctreeNode::INTERIOR_FLAG;
            self.mark_dirty(node_idx);
            self.needs_color_propagation = true;

            // Neighbors can no longer be interior — clear their flags.
            let max_c = (1i32 << self.max_depth) - 1;
            let mut to_clear: Vec<usize> = Vec::new();
            for &dir in &[IVec3::X, -IVec3::X, IVec3::Y, -IVec3::Y, IVec3::Z, -IVec3::Z] {
                let np = pos + dir;
                if np.x < 0 || np.y < 0 || np.z < 0
                    || np.x > max_c || np.y > max_c || np.z > max_c {
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
    // Interior flag helpers (Phase 4)
    // -----------------------------------------------------------------------

    /// Check whether the voxel at `pos` is occupied (non-zero color_data).
    /// Out-of-bounds positions are treated as empty.
    pub fn is_occupied_at(&self, pos: IVec3) -> bool {
        let max_c = (1i32 << self.max_depth) - 1;
        if pos.x < 0 || pos.y < 0 || pos.z < 0
            || pos.x > max_c || pos.y > max_c || pos.z > max_c
        {
            return false;
        }
        self.descend_to(pos)
            .map(|i| self.nodes[i].color_data != 0)
            .unwrap_or(false)
    }

    /// Returns `true` if all 6 axis-aligned neighbors of `pos` are occupied.
    fn all_face_neighbors_occupied(&self, pos: IVec3) -> bool {
        for &dir in &[IVec3::X, -IVec3::X, IVec3::Y, -IVec3::Y, IVec3::Z, -IVec3::Z] {
            if !self.is_occupied_at(pos + dir) {
                return false;
            }
        }
        true
    }

    /// Recalculate the INTERIOR_FLAG for `pos` and its 6 face-neighbors.
    ///
    /// Called after every `set_voxel` so the splat kernel can skip voxels
    /// that are guaranteed not to be visible.
    fn reclassify_interior_around(&mut self, pos: IVec3) {
        let dirs = [
            IVec3::ZERO,
            IVec3::X, -IVec3::X,
            IVec3::Y, -IVec3::Y,
            IVec3::Z, -IVec3::Z,
        ];
        let max_c = (1i32 << self.max_depth) - 1;

        // Collect (node_index, new_interior_flag) without holding a mutable borrow.
        let updates: Vec<(usize, bool)> = dirs
            .iter()
            .filter_map(|&d| {
                let cp = pos + d;
                if cp.x < 0 || cp.y < 0 || cp.z < 0
                    || cp.x > max_c || cp.y > max_c || cp.z > max_c
                {
                    return None;
                }
                let idx = self.descend_to(cp)?;
                if self.nodes[idx].color_data == 0 {
                    return None; // not occupied
                }
                let interior = self.all_face_neighbors_occupied(cp);
                Some((idx, interior))
            })
            .collect();

        for (idx, interior) in updates {
            if interior {
                self.nodes[idx].child_pointer |= OctreeNode::INTERIOR_FLAG;
            } else {
                self.nodes[idx].child_pointer &= !OctreeNode::INTERIOR_FLAG;
            }
            self.mark_dirty(idx);
        }
    }

    /// One-shot pass to classify every leaf node as interior or surface.
    ///
    /// Call this once after bulk world generation (e.g. at the end of
    /// bootstrap) to avoid per-voxel overhead during initial placement.
    pub fn recompute_all_interior_flags(&mut self) {
        let node_count = self.nodes.len();
        let max_c = (1i32 << self.max_depth) - 1;

        // We need world-space positions for every leaf node. Use a DFS.
        // Collect (node_idx, world_pos) for all occupied leaves.
        let mut leaves: Vec<(usize, IVec3)> = Vec::new();
        self.collect_occupied_leaves(
            self.root_index as usize,
            IVec3::ZERO,
            1i32 << self.max_depth,
            &mut leaves,
        );
        let _ = (node_count, max_c);

        let updates: Vec<(usize, bool)> = leaves
            .iter()
            .map(|&(idx, pos)| (idx, self.all_face_neighbors_occupied(pos)))
            .collect();

        for (idx, interior) in updates {
            if interior {
                self.nodes[idx].child_pointer |= OctreeNode::INTERIOR_FLAG;
            } else {
                self.nodes[idx].child_pointer &= !OctreeNode::INTERIOR_FLAG;
            }
            self.mark_dirty(idx);
        }
    }

    fn collect_occupied_leaves(
        &self,
        idx: usize,
        origin: IVec3,
        size: i32,
        out: &mut Vec<(usize, IVec3)>,
    ) {
        let node = &self.nodes[idx];
        if node.is_leaf() {
            if node.color_data != 0 {
                let half = size / 2;
                out.push((idx, origin + IVec3::splat(half)));
            }
            return;
        }
        let child_mask  = node.child_mask();
        let first_child = node.first_child_index();
        let child_half  = size / 2;
        for slot in 0u8..8 {
            if child_mask & (1 << slot) == 0 {
                continue;
            }
            let child_offset = (child_mask & ((1 << slot) - 1)).count_ones() as usize;
            let child_idx    = first_child + child_offset;
            let dx = if slot & 1 != 0 { child_half } else { 0 };
            let dy = if slot & 2 != 0 { child_half } else { 0 };
            let dz = if slot & 4 != 0 { child_half } else { 0 };
            self.collect_occupied_leaves(
                child_idx,
                origin + IVec3::new(dx, dy, dz),
                child_half,
                out,
            );
        }
    }

    // -----------------------------------------------------------------------
    // Dirty-range tracking
    // -----------------------------------------------------------------------

    pub fn mark_dirty(&mut self, node_idx: usize) {
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
    // Position precomputation for GPU
    // -----------------------------------------------------------------------

    /// World-space position of the SVO root's min corner.
    ///
    /// Currently always `(0, 0, 0)`. Exposed as a getter so the ray march
    /// shader can be initialised from the correct origin if the root relocates.
    pub fn origin(&self) -> Vec3 {
        Vec3::ZERO
    }

    /// Total side length of the SVO root cube in world units (`2^max_depth`).
    ///
    /// Each leaf node covers exactly 1×1×1 world-unit cell.
    pub fn world_size(&self) -> f32 {
        (1u32 << self.max_depth) as f32
    }

    /// Compute world-space positions for every node, returned as `[x, y, z, half_extent]`.
    ///
    /// The GPU cannot use recursion, so we precompute positions on the CPU via
    /// a DFS traversal and upload them as a storage buffer.
    pub fn compute_node_positions(&self) -> Vec<[f32; 4]> {
        let world_size = (1u32 << self.max_depth) as f32;
        let mut positions = vec![[0.0f32; 4]; self.nodes.len()];
        self.fill_positions(
            self.root_index as usize,
            0.0, 0.0, 0.0,
            world_size,
            &mut positions,
        );
        positions
    }

    fn fill_positions(
        &self,
        idx: usize,
        ox: f32, oy: f32, oz: f32,
        size: f32,
        out: &mut Vec<[f32; 4]>,
    ) {
        let half = size * 0.5;
        // Store center of this cell
        out[idx] = [ox + half, oy + half, oz + half, half];

        let node = &self.nodes[idx];
        let mask = node.child_mask();
        if mask == 0 {
            return; // leaf
        }
        let first_child = node.first_child_index();
        let child_size = half;
        for slot in 0..8usize {
            if mask & (1 << slot) == 0 {
                continue;
            }
            let cx = if slot & 1 != 0 { ox + half } else { ox };
            let cy = if slot & 2 != 0 { oy + half } else { oy };
            let cz = if slot & 4 != 0 { oz + half } else { oz };
            self.fill_positions(first_child + slot, cx, cy, cz, child_size, out);
        }
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
