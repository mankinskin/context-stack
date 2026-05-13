//! World Generation: procedural noise terrain, delta persistence, biomes, and regrowth.
//!
//! The world is generated deterministically from a seed. SpacetimeDB stores only
//! player-made deltas against the base terrain. Same seed = same world on every
//! client. Biome-driven material selection, cave carving, and structure placement
//! produce a rich open world without storing millions of voxel rows.

use bevy::prelude::*;

use crate::{
    multiplayer_backend::CHUNK_SIZE,
    svo::{
        OctreeNode,
        VoxelMaterial,
        VoxelWorld,
    },
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default world seed.
pub const DEFAULT_SEED: u64 = 0xDEAD_BEEF_CAFE_BABE;

/// Base terrain height.
pub const BASE_HEIGHT: f32 = 64.0;

/// Cave noise threshold — above this value, caves are carved.
pub const CAVE_THRESHOLD: f32 = 0.7;

/// Regrowth period in ticks (default: 1200 ticks = ~60 seconds at 20Hz).
pub const REGROWTH_TICKS: u64 = 1200;

// ---------------------------------------------------------------------------
// Material presets
// ---------------------------------------------------------------------------

pub const MATERIAL_GRASS: u32 = VoxelMaterial::new(60, 140, 40, 18).pack();
pub const MATERIAL_STONE: u32 = VoxelMaterial::new(128, 128, 130, 28).pack();
pub const MATERIAL_SAND: u32 = VoxelMaterial::new(210, 190, 140, 24).pack();
pub const MATERIAL_SNOW: u32 = VoxelMaterial::new(240, 245, 255, 8).pack();
pub const MATERIAL_DIRT: u32 = VoxelMaterial::new(100, 70, 40, 22).pack();
pub const MATERIAL_WATER: u32 = VoxelMaterial::new(30, 80, 180, 4).pack();

// ---------------------------------------------------------------------------
// Noise primitives
// ---------------------------------------------------------------------------

/// Simple hash-based pseudo-random noise (deterministic from seed).
///
/// Produces values in [-1.0, 1.0]. Not a true Simplex implementation but
/// shares the same API and determinism guarantees for procedural generation.
pub fn noise3d(
    seed: u64,
    x: f32,
    y: f32,
    z: f32,
) -> f32 {
    let ix = (x.floor() as i32) as u64;
    let iy = (y.floor() as i32) as u64;
    let iz = (z.floor() as i32) as u64;

    let fx = x - x.floor();
    let fy = y - y.floor();
    let fz = z - z.floor();

    // Smoothstep interpolation weights
    let ux = fx * fx * (3.0 - 2.0 * fx);
    let uy = fy * fy * (3.0 - 2.0 * fy);
    let uz = fz * fz * (3.0 - 2.0 * fz);

    // Hash at 8 corners
    let v000 = hash_to_float(seed, ix, iy, iz);
    let v100 = hash_to_float(seed, ix.wrapping_add(1), iy, iz);
    let v010 = hash_to_float(seed, ix, iy.wrapping_add(1), iz);
    let v110 = hash_to_float(seed, ix.wrapping_add(1), iy.wrapping_add(1), iz);
    let v001 = hash_to_float(seed, ix, iy, iz.wrapping_add(1));
    let v101 = hash_to_float(seed, ix.wrapping_add(1), iy, iz.wrapping_add(1));
    let v011 = hash_to_float(seed, ix, iy.wrapping_add(1), iz.wrapping_add(1));
    let v111 = hash_to_float(
        seed,
        ix.wrapping_add(1),
        iy.wrapping_add(1),
        iz.wrapping_add(1),
    );

    // Trilinear interpolation
    let x00 = lerp(v000, v100, ux);
    let x10 = lerp(v010, v110, ux);
    let x01 = lerp(v001, v101, ux);
    let x11 = lerp(v011, v111, ux);
    let y0 = lerp(x00, x10, uy);
    let y1 = lerp(x01, x11, uy);
    lerp(y0, y1, uz)
}

/// 2D noise (for biome selection).
pub fn noise2d(
    seed: u64,
    x: f32,
    z: f32,
) -> f32 {
    noise3d(seed, x, 0.0, z)
}

/// Fractal Brownian Motion — layered noise for terrain detail.
pub fn fbm(
    seed: u64,
    x: f32,
    y: f32,
    z: f32,
    octaves: u32,
) -> f32 {
    let mut value = 0.0f32;
    let mut amplitude = 1.0f32;
    let mut frequency = 1.0f32;
    let mut max_amp = 0.0f32;

    for i in 0..octaves {
        value += noise3d(
            seed.wrapping_add(i as u64 * 31337),
            x * frequency,
            y * frequency,
            z * frequency,
        ) * amplitude;
        max_amp += amplitude;
        amplitude *= 0.5;
        frequency *= 2.0;
    }

    if max_amp > 0.0 {
        value / max_amp
    } else {
        0.0
    }
}

fn hash_to_float(
    seed: u64,
    x: u64,
    y: u64,
    z: u64,
) -> f32 {
    let h = hash_combine(seed, x, y, z);
    // Map to [-1.0, 1.0]
    (h as f32 / u32::MAX as f32) * 2.0 - 1.0
}

fn hash_combine(
    seed: u64,
    x: u64,
    y: u64,
    z: u64,
) -> u32 {
    let mut h = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(x.wrapping_mul(1442695040888963407))
        .wrapping_add(y.wrapping_mul(2862933555777941757))
        .wrapping_add(z.wrapping_mul(3037000499));
    h ^= h >> 33;
    h = h.wrapping_mul(0xff51afd7ed558ccd);
    h ^= h >> 33;
    (h & 0xFFFFFFFF) as u32
}

fn lerp(
    a: f32,
    b: f32,
    t: f32,
) -> f32 {
    a + (b - a) * t
}

// ---------------------------------------------------------------------------
// Biomes
// ---------------------------------------------------------------------------

/// Biome classification based on 2D noise.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Biome {
    Snow,
    Forest,
    Desert,
    Mountain,
    Ocean,
}

impl Biome {
    /// Classify biome from a noise value in [-1.0, 1.0].
    pub fn from_noise(v: f32) -> Self {
        match v {
            v if v < -0.4 => Biome::Snow,
            v if v < 0.0 => Biome::Forest,
            v if v < 0.3 => Biome::Desert,
            v if v < 0.7 => Biome::Mountain,
            _ => Biome::Ocean,
        }
    }

    /// Surface material for this biome.
    pub fn surface_material(&self) -> u32 {
        match self {
            Biome::Snow => MATERIAL_SNOW,
            Biome::Forest => MATERIAL_GRASS,
            Biome::Desert => MATERIAL_SAND,
            Biome::Mountain => MATERIAL_STONE,
            Biome::Ocean => MATERIAL_WATER,
        }
    }

    /// Sub-surface material (below 2 voxels from surface).
    pub fn subsurface_material(&self) -> u32 {
        match self {
            Biome::Snow => MATERIAL_STONE,
            Biome::Forest => MATERIAL_DIRT,
            Biome::Desert => MATERIAL_SAND,
            Biome::Mountain => MATERIAL_STONE,
            Biome::Ocean => MATERIAL_STONE,
        }
    }

    /// Whether this biome supports resource regrowth.
    pub fn supports_regrowth(&self) -> bool {
        matches!(self, Biome::Forest | Biome::Snow)
    }
}

// ---------------------------------------------------------------------------
// World generator
// ---------------------------------------------------------------------------

/// Seed-deterministic world generator.
#[derive(Resource, Clone, Debug)]
pub struct WorldGenerator {
    pub seed: u64,
}

impl Default for WorldGenerator {
    fn default() -> Self {
        Self { seed: DEFAULT_SEED }
    }
}

impl WorldGenerator {
    pub fn new(seed: u64) -> Self {
        Self { seed }
    }

    /// Sample terrain density at world coordinates.
    ///
    /// Positive = solid, negative = air.
    pub fn sample_density(
        &self,
        x: f32,
        y: f32,
        z: f32,
    ) -> f32 {
        let terrain = noise3d(self.seed, x * 0.01, y * 0.02, z * 0.01);
        let mountains =
            fbm(self.seed.wrapping_add(1), x * 0.005, 0.0, z * 0.005, 4) * 80.0;
        let caves =
            noise3d(self.seed.wrapping_add(2), x * 0.05, y * 0.05, z * 0.05);

        let base_height = BASE_HEIGHT + terrain * 20.0 + mountains;
        let height_density = base_height - y;

        // Carve caves
        let cave_carve = if caves > CAVE_THRESHOLD { -10.0 } else { 0.0 };
        height_density + cave_carve
    }

    /// Sample biome at world xz coordinates.
    pub fn sample_biome(
        &self,
        x: f32,
        z: f32,
    ) -> Biome {
        let v = noise2d(self.seed.wrapping_add(100), x * 0.002, z * 0.002);
        Biome::from_noise(v)
    }

    /// Sample material at world coordinates given density.
    pub fn sample_material(
        &self,
        x: f32,
        y: f32,
        z: f32,
    ) -> u32 {
        let biome = self.sample_biome(x, z);
        let surface_height = BASE_HEIGHT
            + noise3d(self.seed, x * 0.01, 0.0, z * 0.01) * 20.0
            + fbm(self.seed.wrapping_add(1), x * 0.005, 0.0, z * 0.005, 4)
                * 80.0;

        if y > surface_height - 2.0 {
            biome.surface_material()
        } else {
            biome.subsurface_material()
        }
    }

    /// Generate voxels for a single chunk and populate the VoxelWorld.
    ///
    /// Returns the number of voxels generated.
    pub fn generate_chunk_into(
        &self,
        world: &mut VoxelWorld,
        cx: i32,
        cy: i32,
        cz: i32,
    ) -> u32 {
        let mut count = 0u32;
        let cs = CHUNK_SIZE as i32;
        let wx_base = cx * cs;
        let wy_base = cy * cs;
        let wz_base = cz * cs;

        for lz in 0..cs {
            for ly in 0..cs {
                for lx in 0..cs {
                    let wx = wx_base + lx;
                    let wy = wy_base + ly;
                    let wz = wz_base + lz;

                    let density =
                        self.sample_density(wx as f32, wy as f32, wz as f32);
                    if density > 0.0 {
                        let color = self
                            .sample_material(wx as f32, wy as f32, wz as f32);
                        let mat = VoxelMaterial::unpack(color);
                        world.set_voxel(IVec3::new(wx, wy, wz), mat);
                        count += 1;
                    }
                }
            }
        }
        count
    }
}

// ---------------------------------------------------------------------------
// Structure templates
// ---------------------------------------------------------------------------

/// A pre-defined voxel structure (tree, boulder, ruin, etc.).
#[derive(Clone, Debug)]
pub struct StructureTemplate {
    /// Voxels as (relative_offset, packed_color).
    pub voxels: Vec<(IVec3, u32)>,
    /// Name for debugging.
    pub name: String,
}

impl StructureTemplate {
    /// Stamp this structure into the world at the given anchor position.
    pub fn stamp(
        &self,
        world: &mut VoxelWorld,
        anchor: IVec3,
    ) {
        for (offset, color) in &self.voxels {
            let mat = VoxelMaterial::unpack(*color);
            world.set_voxel(anchor + *offset, mat);
        }
    }
}

/// Create a simple tree template.
pub fn tree_template() -> StructureTemplate {
    let trunk = VoxelMaterial::new(80, 50, 20, 26).pack();
    let leaves = VoxelMaterial::new(30, 120, 20, 14).pack();
    let mut voxels = Vec::new();

    // Trunk: 5 blocks tall
    for y in 0..5 {
        voxels.push((IVec3::new(0, y, 0), trunk));
    }

    // Canopy: 3×3×2 at top
    for dy in 0..2 {
        for dx in -1..=1 {
            for dz in -1..=1 {
                if dx == 0 && dz == 0 && dy == 0 {
                    continue; // trunk occupies this
                }
                voxels.push((IVec3::new(dx, 5 + dy, dz), leaves));
            }
        }
    }
    // Top cap
    voxels.push((IVec3::new(0, 7, 0), leaves));

    StructureTemplate {
        voxels,
        name: "tree".to_string(),
    }
}

/// Create a boulder template.
pub fn boulder_template() -> StructureTemplate {
    let stone = VoxelMaterial::new(140, 140, 145, 28).pack();
    let mut voxels = Vec::new();
    // Rough 3×2×3 boulder
    for dy in 0..2 {
        for dx in -1..=1 {
            for dz in -1..=1 {
                voxels.push((IVec3::new(dx, dy, dz), stone));
            }
        }
    }
    StructureTemplate {
        voxels,
        name: "boulder".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Regrowth tracking
// ---------------------------------------------------------------------------

/// A delta pending regrowth evaluation.
#[derive(Clone, Debug)]
pub struct RegrowthEntry {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub delta_tick: u64,
}

/// Tracks voxel regrowth state.
#[derive(Resource, Default)]
pub struct RegrowthTracker {
    pub entries: Vec<RegrowthEntry>,
}

impl RegrowthTracker {
    /// Check and remove entries that have passed the regrowth threshold.
    /// Returns positions that should regrow.
    pub fn tick_regrowth(
        &mut self,
        current_tick: u64,
    ) -> Vec<IVec3> {
        let mut regrown = Vec::new();
        self.entries.retain(|e| {
            if current_tick - e.delta_tick >= REGROWTH_TICKS {
                regrown.push(IVec3::new(e.x, e.y, e.z));
                false
            } else {
                true
            }
        });
        regrown
    }
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

/// Plugin registering world generation resources.
pub struct WorldGenPlugin;

impl Plugin for WorldGenPlugin {
    fn build(
        &self,
        app: &mut App,
    ) {
        app.init_resource::<WorldGenerator>();
        app.init_resource::<RegrowthTracker>();
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
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
        .map(|b| b.surface_material())
        .collect();
        // All unique
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
        // Well below base height → should be solid
        let d = gen.sample_density(0.0, 0.0, 0.0);
        assert!(d > 0.0, "deep underground should be solid, got {d}");
    }

    #[test]
    fn sample_density_above_surface_negative() {
        let gen = WorldGenerator::new(42);
        // Well above any terrain → should be air
        let d = gen.sample_density(0.0, 500.0, 0.0);
        assert!(d < 0.0, "high above terrain should be air, got {d}");
    }

    #[test]
    fn generate_chunk_produces_voxels() {
        let gen = WorldGenerator::new(42);
        let mut world = VoxelWorld::new(8);
        // Generate a chunk around sea level
        let count = gen.generate_chunk_into(&mut world, 0, 3, 0); // y=48..63
        assert!(count > 0, "chunk at surface level should have voxels");
    }

    #[test]
    fn generate_chunk_above_terrain_empty() {
        let gen = WorldGenerator::new(42);
        let mut world = VoxelWorld::new(8);
        // Generate a chunk way above terrain
        let count = gen.generate_chunk_into(&mut world, 0, 30, 0); // y=480..495
        assert_eq!(count, 0, "chunk far above terrain should be empty");
    }

    #[test]
    fn tree_template_valid() {
        let tree = tree_template();
        assert!(!tree.voxels.is_empty());
        // Has both trunk and leaf colors
        let colors: std::collections::HashSet<u32> =
            tree.voxels.iter().map(|(_, c)| *c).collect();
        assert!(
            colors.len() >= 2,
            "tree should have at least trunk + leaves"
        );
    }

    #[test]
    fn boulder_template_valid() {
        let boulder = boulder_template();
        assert!(!boulder.voxels.is_empty());
        assert_eq!(boulder.voxels.len(), 18); // 3×2×3
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
        assert_eq!(tracker.entries.len(), 1); // second entry still pending
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
}
