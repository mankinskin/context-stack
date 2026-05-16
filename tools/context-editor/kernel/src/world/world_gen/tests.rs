use super::*;

#[test]
fn noise3d_deterministic() {
    let a = noise3d(42, 1.5, 2.5, 3.5);
    let b = noise3d(42, 1.5, 2.5, 3.5);
    assert_eq!(a, b);
}

#[test]
fn noise3d_different_seeds() {
    let a = noise3d(1, 1.0, 2.0, 3.0);
    let b = noise3d(2, 1.0, 2.0, 3.0);
    assert_ne!(a, b);
}

#[test]
fn noise3d_in_range() {
    for i in 0..100 {
        let v = noise3d(42, i as f32 * 0.1, 0.0, 0.0);
        assert!(v >= -1.0 && v <= 1.0, "noise value {v} out of range");
    }
}

#[test]
fn fbm_deterministic() {
    let a = fbm(42, 1.0, 2.0, 3.0, 4);
    let b = fbm(42, 1.0, 2.0, 3.0, 4);
    assert_eq!(a, b);
}

#[test]
fn biome_classification() {
    assert_eq!(Biome::from_noise(-0.5), Biome::Snow);
    assert_eq!(Biome::from_noise(-0.1), Biome::Forest);
    assert_eq!(Biome::from_noise(0.1), Biome::Desert);
    assert_eq!(Biome::from_noise(0.5), Biome::Mountain);
    assert_eq!(Biome::from_noise(0.8), Biome::Ocean);
}

#[test]
fn biome_surface_materials_differ() {
    let mats: Vec<u32> = [
        Biome::Snow,
        Biome::Forest,
        Biome::Desert,
        Biome::Mountain,
        Biome::Ocean,
    ]
    .iter()
    .map(|biome| biome.surface_material())
    .collect();
    for i in 0..mats.len() {
        for j in (i + 1)..mats.len() {
            assert_ne!(mats[i], mats[j], "biome materials should differ");
        }
    }
}

#[test]
fn regrowth_only_forests_and_snow() {
    assert!(Biome::Forest.supports_regrowth());
    assert!(Biome::Snow.supports_regrowth());
    assert!(!Biome::Desert.supports_regrowth());
    assert!(!Biome::Mountain.supports_regrowth());
    assert!(!Biome::Ocean.supports_regrowth());
}

#[test]
fn sample_density_below_surface_positive() {
    let gen = WorldGenerator::new(42);
    let d = gen.sample_density(0.0, 0.0, 0.0);
    assert!(d > 0.0, "deep underground should be solid, got {d}");
}

#[test]
fn sample_density_above_surface_negative() {
    let gen = WorldGenerator::new(42);
    let d = gen.sample_density(0.0, 500.0, 0.0);
    assert!(d < 0.0, "high above terrain should be air, got {d}");
}

#[test]
fn generate_chunk_produces_voxels() {
    let gen = WorldGenerator::new(42);
    let mut world = VoxelWorld::new(8);
    let count = gen.generate_chunk_into(&mut world, 0, 3, 0);
    assert!(count > 0, "chunk at surface level should have voxels");
}

#[test]
fn generate_chunk_above_terrain_empty() {
    let gen = WorldGenerator::new(42);
    let mut world = VoxelWorld::new(8);
    let count = gen.generate_chunk_into(&mut world, 0, 30, 0);
    assert_eq!(count, 0, "chunk far above terrain should be empty");
}

#[test]
fn tree_template_valid() {
    let tree = tree_template();
    assert!(!tree.voxels.is_empty());
    let colors: std::collections::HashSet<u32> = tree.voxels.iter().map(|(_, color)| *color).collect();
    assert!(colors.len() >= 2, "tree should have at least trunk + leaves");
}

#[test]
fn boulder_template_valid() {
    let boulder = boulder_template();
    assert!(!boulder.voxels.is_empty());
    assert_eq!(boulder.voxels.len(), 18);
}

#[test]
fn regrowth_tracker_basic() {
    let mut tracker = RegrowthTracker::default();
    tracker.entries.push(RegrowthEntry {
        x: 1,
        y: 2,
        z: 3,
        delta_tick: 0,
    });
    tracker.entries.push(RegrowthEntry {
        x: 4,
        y: 5,
        z: 6,
        delta_tick: 1000,
    });

    let regrown = tracker.tick_regrowth(REGROWTH_TICKS + 1);
    assert_eq!(regrown.len(), 1);
    assert_eq!(regrown[0], IVec3::new(1, 2, 3));
    assert_eq!(tracker.entries.len(), 1);
}

#[test]
fn regrowth_tracker_none_expired() {
    let mut tracker = RegrowthTracker::default();
    tracker.entries.push(RegrowthEntry {
        x: 0,
        y: 0,
        z: 0,
        delta_tick: 100,
    });
    let regrown = tracker.tick_regrowth(200);
    assert!(regrown.is_empty());
}

#[test]
fn same_seed_same_density() {
    let gen1 = WorldGenerator::new(12345);
    let gen2 = WorldGenerator::new(12345);
    for i in 0..20 {
        let x = i as f32 * 3.7;
        let d1 = gen1.sample_density(x, 50.0, x * 0.5);
        let d2 = gen2.sample_density(x, 50.0, x * 0.5);
        assert_eq!(d1, d2, "same seed must produce identical density");
    }
}
