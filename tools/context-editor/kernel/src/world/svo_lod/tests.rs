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
    let red = VoxelMaterial::new(200, 50, 80, 10);
    world.set_voxel(IVec3::ZERO, red);
    propagate_lod_colors(&mut world);
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
    let depth = effective_lod_depth(10, Vec2::new(0.5, 0.5), &occluders, &params);
    assert_eq!(depth, 10 - params.glass_lod_reduction);
}

#[test]
fn effective_lod_clear_glass_no_reduction() {
    let params = LodParams::default();
    let mut occluders = GlassOccluders::default();
    occluders.occluders.push(GlassOccluder {
        screen_min: Vec2::ZERO,
        screen_max: Vec2::ONE,
        blur_strength: 0.3,
    });
    let depth = effective_lod_depth(10, Vec2::new(0.5, 0.5), &occluders, &params);
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
    let depth = effective_lod_depth(10, Vec2::new(0.5, 0.5), &occluders, &params);
    assert_eq!(depth, 10);
}

#[test]
fn lod_depth_zero_distance() {
    assert_eq!(lod_depth_for_distance(0.0, 3, 10, 5.0, 500.0, 0.25), 10);
}
