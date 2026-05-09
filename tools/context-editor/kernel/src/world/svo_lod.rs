//! SVO LOD Management & GPU Streaming.
//!
//! Implements dynamic Level-of-Detail for the Sparse Voxel Octree:
//! - Parent nodes accumulate average color from descendants for distant rendering.
//! - Camera-distance–based LOD selection limits traversal depth.
//! - LRU chunk cache manages GPU VRAM budget for large worlds.
//! - Glass panel occlusion allows early LOD reduction behind blurred UI.

use bevy::prelude::*;
use std::collections::VecDeque;

use crate::svo::{
    OctreeNode,
    VoxelMaterial,
    VoxelWorld,
};

// ---------------------------------------------------------------------------
// LOD selection
// ---------------------------------------------------------------------------

/// Compute the target octree traversal depth for a given camera distance.
///
/// Near the camera (distance ≤ `near`) we use `max_depth`.
/// Far away (distance ≥ `far`) we use `min_depth`.
/// In between we interpolate logarithmically.
pub fn lod_depth_for_distance(
    distance: f32,
    min_depth: u32,
    max_depth: u32,
    near: f32,
    far: f32,
    detail_factor: f32,
) -> u32 {
    if distance <= near {
        return max_depth;
    }
    if distance >= far {
        return min_depth;
    }
    // target = max_depth - log2(distance * detail_factor)
    let raw = (max_depth as f32) - (distance * detail_factor).log2();
    raw.clamp(min_depth as f32, max_depth as f32).round() as u32
}

/// Parameters controlling LOD depth selection.
#[derive(Resource, Clone, Debug)]
pub struct LodParams {
    /// Minimum traversal depth (coarsest LOD for distant geometry).
    pub min_depth: u32,
    /// Maximum traversal depth (finest LOD near the camera, usually == SVO max_depth).
    pub max_depth: u32,
    /// Distance at which full detail is used.
    pub near_distance: f32,
    /// Distance at which minimum depth is used.
    pub far_distance: f32,
    /// Multiplier applied to distance before log2. Higher = coarser LOD sooner.
    pub detail_factor: f32,
    /// When true, glass-panel occlusion reduces LOD behind blurred panels.
    pub glass_occlusion_enabled: bool,
    /// LOD depth reduction applied behind frosted-glass UI panels.
    pub glass_lod_reduction: u32,
}

impl Default for LodParams {
    fn default() -> Self {
        Self {
            min_depth: 3,
            max_depth: 10,
            near_distance: 20.0,
            far_distance: 2000.0,
            detail_factor: 0.25,
            glass_occlusion_enabled: true,
            glass_lod_reduction: 4,
        }
    }
}

// ---------------------------------------------------------------------------
// Average-color accumulation for parent nodes
// ---------------------------------------------------------------------------

/// Compute the average color of a parent node from its children.
///
/// Only non-empty children (color_data != 0) contribute.
/// Returns 0 if all children are empty.
pub fn compute_avg_color(children: &[OctreeNode]) -> u32 {
    let (mut r_sum, mut g_sum, mut b_sum, mut rough_sum) =
        (0u32, 0u32, 0u32, 0u32);
    let mut count = 0u32;

    for child in children {
        if child.color_data == 0 {
            continue;
        }
        let mat = VoxelMaterial::unpack(child.color_data);
        r_sum += mat.r as u32;
        g_sum += mat.g as u32;
        b_sum += mat.b as u32;
        rough_sum += mat.roughness as u32;
        count += 1;
    }

    if count == 0 {
        return 0;
    }

    let avg = VoxelMaterial::new(
        (r_sum / count) as u8,
        (g_sum / count) as u8,
        (b_sum / count) as u8,
        (rough_sum / count) as u8,
    );
    avg.pack()
}

/// Propagate average colors up the octree from leaves to root.
///
/// After calling this, every internal node's `color_data` represents the
/// average color of its descendants, suitable for coarse LOD rendering.
pub fn propagate_lod_colors(world: &mut VoxelWorld) {
    propagate_node(world, world.root_index as usize);
}

fn propagate_node(
    world: &mut VoxelWorld,
    idx: usize,
) -> u32 {
    let node = world.nodes[idx];
    if node.is_leaf() {
        return node.color_data;
    }

    let mask = node.child_mask();
    let first = node.first_child_index();
    let mut child_colors = [OctreeNode::default(); 8];

    for bit in 0..8u8 {
        if mask & (1 << bit) != 0 {
            let child_idx = first + bit as usize;
            let child_color = propagate_node(world, child_idx);
            child_colors[bit as usize].color_data = child_color;
        }
    }

    let avg = compute_avg_color(&child_colors);
    world.nodes[idx].color_data = avg;
    avg
}

// ---------------------------------------------------------------------------
// LRU Chunk Cache
// ---------------------------------------------------------------------------

/// Identifier for a spatial chunk at a given LOD level.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChunkId {
    /// Chunk grid coordinates.
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// LOD level (depth) this chunk represents.
    pub lod_level: u32,
}

/// A loaded chunk's data ready for GPU upload.
#[derive(Clone, Debug)]
pub struct ChunkData {
    pub id: ChunkId,
    /// Byte offset in the GPU buffer where this chunk is stored.
    pub buffer_offset: usize,
    /// Size in bytes of this chunk's node data.
    pub byte_size: usize,
}

/// LRU cache managing which chunks are loaded in GPU VRAM.
///
/// Maintains a fixed VRAM budget and evicts least-recently-used chunks
/// when capacity is exceeded.
#[derive(Resource, Clone, Debug)]
pub struct ChunkLruCache {
    /// Maximum VRAM budget in bytes.
    pub budget_bytes: usize,
    /// Current bytes used.
    pub used_bytes: usize,
    /// Chunks ordered from most-recently-used (front) to least-recently-used (back).
    pub entries: VecDeque<ChunkData>,
}

impl ChunkLruCache {
    pub fn new(budget_bytes: usize) -> Self {
        Self {
            budget_bytes,
            used_bytes: 0,
            entries: VecDeque::new(),
        }
    }

    /// Touch a chunk, moving it to the front (most recently used).
    ///
    /// Returns true if the chunk was already cached.
    pub fn touch(
        &mut self,
        id: ChunkId,
    ) -> bool {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            let entry = self.entries.remove(pos).unwrap();
            self.entries.push_front(entry);
            true
        } else {
            false
        }
    }

    /// Insert a chunk into the cache. Evicts LRU entries if needed.
    ///
    /// Returns the list of evicted chunk IDs.
    pub fn insert(
        &mut self,
        data: ChunkData,
    ) -> Vec<ChunkId> {
        let mut evicted = Vec::new();

        // Evict from the back until we have space
        while self.used_bytes + data.byte_size > self.budget_bytes
            && !self.entries.is_empty()
        {
            if let Some(old) = self.entries.pop_back() {
                self.used_bytes = self.used_bytes.saturating_sub(old.byte_size);
                evicted.push(old.id);
            }
        }

        self.used_bytes += data.byte_size;
        self.entries.push_front(data);
        evicted
    }

    /// Remove a specific chunk from the cache.
    pub fn remove(
        &mut self,
        id: ChunkId,
    ) -> bool {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            let entry = self.entries.remove(pos).unwrap();
            self.used_bytes = self.used_bytes.saturating_sub(entry.byte_size);
            true
        } else {
            false
        }
    }

    /// Number of chunks currently cached.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ---------------------------------------------------------------------------
// LOD streaming resource
// ---------------------------------------------------------------------------

/// Tracks which chunks need loading or eviction each frame.
#[derive(Resource, Default)]
pub struct LodStreamingState {
    /// Chunks that need to be loaded this frame (high priority).
    pub pending_loads: Vec<ChunkId>,
    /// Chunks that were evicted this frame.
    pub evicted: Vec<ChunkId>,
    /// Camera position used for last LOD update.
    pub last_camera_pos: Vec3,
    /// Maximum number of chunk loads per frame (bandwidth budget).
    pub max_loads_per_frame: usize,
}

impl LodStreamingState {
    pub fn new(max_loads_per_frame: usize) -> Self {
        Self {
            max_loads_per_frame,
            ..Default::default()
        }
    }
}

// ---------------------------------------------------------------------------
// Glass occlusion
// ---------------------------------------------------------------------------

/// Represents a frosted-glass UI region that can trigger LOD reduction.
#[derive(Clone, Debug)]
pub struct GlassOccluder {
    /// Screen-space min corner (normalized 0..1).
    pub screen_min: Vec2,
    /// Screen-space max corner (normalized 0..1).
    pub screen_max: Vec2,
    /// Blur strength (0.0 = clear, 1.0 = fully frosted).
    pub blur_strength: f32,
}

impl GlassOccluder {
    /// Returns true if this occluder covers the given screen-space point.
    pub fn contains(
        &self,
        point: Vec2,
    ) -> bool {
        point.x >= self.screen_min.x
            && point.x <= self.screen_max.x
            && point.y >= self.screen_min.y
            && point.y <= self.screen_max.y
    }
}

/// Resource tracking active glass occluders for LOD optimization.
#[derive(Resource, Default)]
pub struct GlassOccluders {
    pub occluders: Vec<GlassOccluder>,
}

/// Calculate effective LOD depth considering glass occlusion.
///
/// If the screen-space point is behind a frosted glass panel with sufficient
/// blur strength, reduce the LOD depth to save GPU compute.
pub fn effective_lod_depth(
    base_depth: u32,
    screen_point: Vec2,
    occluders: &GlassOccluders,
    params: &LodParams,
) -> u32 {
    if !params.glass_occlusion_enabled {
        return base_depth;
    }

    for occ in &occluders.occluders {
        if occ.contains(screen_point) && occ.blur_strength > 0.5 {
            return base_depth.saturating_sub(params.glass_lod_reduction);
        }
    }

    base_depth
}

// ---------------------------------------------------------------------------
// Bevy systems
// ---------------------------------------------------------------------------

/// System: propagate LOD colors up the octree whenever dirty ranges exist.
fn lod_propagation_system(mut world: ResMut<VoxelWorld>) {
    // Only propagate when there are dirty nodes (the upload system has already
    // drained dirty_ranges, so we use node count as a heuristic — in a real
    // implementation this would track a separate "lod_dirty" flag).
    if world.nodes.len() > 1 {
        propagate_lod_colors(&mut world);
    }
}

/// System: update LOD streaming state based on camera position.
fn lod_streaming_system(
    camera_q: Query<&GlobalTransform, With<Camera3d>>,
    mut streaming: ResMut<LodStreamingState>,
    lod_params: Res<LodParams>,
) {
    let Ok(cam_transform) = camera_q.single() else {
        return;
    };
    streaming.last_camera_pos = cam_transform.translation();

    // Clear per-frame state
    streaming.pending_loads.clear();
    streaming.evicted.clear();
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

/// Plugin registering SVO LOD management and GPU streaming systems.
pub struct SvoLodPlugin;

impl Plugin for SvoLodPlugin {
    fn build(
        &self,
        app: &mut App,
    ) {
        app.init_resource::<LodParams>();
        app.insert_resource(ChunkLruCache::new(64 * 1024 * 1024)); // 64 MiB default
        app.insert_resource(LodStreamingState::new(4));
        app.init_resource::<GlassOccluders>();

        app.add_systems(Update, (lod_propagation_system, lod_streaming_system));
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lod_depth_at_near_returns_max() {
        assert_eq!(lod_depth_for_distance(1.0, 3, 10, 5.0, 500.0, 0.25), 10);
    }

    #[test]
    fn lod_depth_at_far_returns_min() {
        assert_eq!(lod_depth_for_distance(600.0, 3, 10, 5.0, 500.0, 0.25), 3);
    }

    #[test]
    fn lod_depth_mid_range_between_min_and_max() {
        let d = lod_depth_for_distance(50.0, 3, 10, 5.0, 500.0, 0.25);
        assert!(
            d >= 3 && d <= 10,
            "mid-range LOD depth must be in [3, 10], got {d}"
        );
    }

    #[test]
    fn compute_avg_color_all_empty_returns_zero() {
        let children = [OctreeNode::default(); 8];
        assert_eq!(compute_avg_color(&children), 0);
    }

    #[test]
    fn compute_avg_color_uniform_children() {
        let mat = VoxelMaterial::new(100, 100, 100, 16);
        let packed = mat.pack();
        let children: Vec<OctreeNode> = (0..4)
            .map(|_| OctreeNode::leaf(packed))
            .chain((0..4).map(|_| OctreeNode::default()))
            .collect();
        let avg = compute_avg_color(&children);
        let unpacked = VoxelMaterial::unpack(avg);
        assert_eq!(unpacked.r, 100);
        assert_eq!(unpacked.g, 100);
        assert_eq!(unpacked.b, 100);
    }

    #[test]
    fn compute_avg_color_mixed_children() {
        let red = VoxelMaterial::new(200, 0, 0, 10).pack();
        let blue = VoxelMaterial::new(0, 0, 200, 10).pack();
        let children = [
            OctreeNode::leaf(red),
            OctreeNode::leaf(blue),
            OctreeNode::default(),
            OctreeNode::default(),
            OctreeNode::default(),
            OctreeNode::default(),
            OctreeNode::default(),
            OctreeNode::default(),
        ];
        let avg = compute_avg_color(&children);
        let unpacked = VoxelMaterial::unpack(avg);
        assert_eq!(unpacked.r, 100);
        assert_eq!(unpacked.g, 0);
        assert_eq!(unpacked.b, 100);
    }

    #[test]
    fn propagate_lod_colors_single_level() {
        let mut world = VoxelWorld::new(1);
        // Set a voxel so the root has at least one child
        let red = VoxelMaterial::new(200, 50, 80, 10);
        world.set_voxel(IVec3::ZERO, red);
        propagate_lod_colors(&mut world);
        // Root should now have a non-zero avg color
        let root_color = world.nodes[world.root_index as usize].color_data;
        assert_ne!(
            root_color, 0,
            "root must have averaged color after propagation"
        );
    }

    #[test]
    fn chunk_lru_insert_and_touch() {
        let mut cache = ChunkLruCache::new(1024);
        let id = ChunkId {
            x: 0,
            y: 0,
            z: 0,
            lod_level: 5,
        };
        let data = ChunkData {
            id,
            buffer_offset: 0,
            byte_size: 256,
        };
        let evicted = cache.insert(data);
        assert!(evicted.is_empty());
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.used_bytes, 256);

        // Touch should return true (found)
        assert!(cache.touch(id));
    }

    #[test]
    fn chunk_lru_eviction_on_budget() {
        let mut cache = ChunkLruCache::new(512);
        for i in 0..3 {
            let id = ChunkId {
                x: i,
                y: 0,
                z: 0,
                lod_level: 3,
            };
            let data = ChunkData {
                id,
                buffer_offset: (i as usize) * 256,
                byte_size: 256,
            };
            cache.insert(data);
        }
        // Budget is 512, inserting 3 × 256 = 768 → first chunk should be evicted
        assert_eq!(cache.len(), 2);
        assert!(cache.used_bytes <= 512);
    }

    #[test]
    fn chunk_lru_remove() {
        let mut cache = ChunkLruCache::new(1024);
        let id = ChunkId {
            x: 1,
            y: 2,
            z: 3,
            lod_level: 7,
        };
        let data = ChunkData {
            id,
            buffer_offset: 0,
            byte_size: 100,
        };
        cache.insert(data);
        assert!(cache.remove(id));
        assert_eq!(cache.len(), 0);
        assert_eq!(cache.used_bytes, 0);
    }

    #[test]
    fn glass_occluder_contains() {
        let occ = GlassOccluder {
            screen_min: Vec2::new(0.2, 0.3),
            screen_max: Vec2::new(0.8, 0.7),
            blur_strength: 0.9,
        };
        assert!(occ.contains(Vec2::new(0.5, 0.5)));
        assert!(!occ.contains(Vec2::new(0.1, 0.5)));
        assert!(!occ.contains(Vec2::new(0.5, 0.9)));
    }

    #[test]
    fn effective_lod_behind_glass_reduces_depth() {
        let params = LodParams::default();
        let mut occluders = GlassOccluders::default();
        occluders.occluders.push(GlassOccluder {
            screen_min: Vec2::ZERO,
            screen_max: Vec2::ONE,
            blur_strength: 0.8,
        });
        let depth =
            effective_lod_depth(10, Vec2::new(0.5, 0.5), &occluders, &params);
        assert_eq!(depth, 10 - params.glass_lod_reduction);
    }

    #[test]
    fn effective_lod_clear_glass_no_reduction() {
        let params = LodParams::default();
        let mut occluders = GlassOccluders::default();
        occluders.occluders.push(GlassOccluder {
            screen_min: Vec2::ZERO,
            screen_max: Vec2::ONE,
            blur_strength: 0.3, // below threshold
        });
        let depth =
            effective_lod_depth(10, Vec2::new(0.5, 0.5), &occluders, &params);
        assert_eq!(depth, 10);
    }

    #[test]
    fn effective_lod_disabled_glass_no_reduction() {
        let mut params = LodParams::default();
        params.glass_occlusion_enabled = false;
        let mut occluders = GlassOccluders::default();
        occluders.occluders.push(GlassOccluder {
            screen_min: Vec2::ZERO,
            screen_max: Vec2::ONE,
            blur_strength: 1.0,
        });
        let depth =
            effective_lod_depth(10, Vec2::new(0.5, 0.5), &occluders, &params);
        assert_eq!(depth, 10);
    }

    #[test]
    fn lod_depth_zero_distance() {
        assert_eq!(lod_depth_for_distance(0.0, 3, 10, 5.0, 500.0, 0.25), 10);
    }
}
