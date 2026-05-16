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
        VoxelMaterial,
        VoxelWorld,
    },
};

mod noise;
mod plugin;

pub use noise::{
    fbm,
    noise2d,
    noise3d,
};
pub use plugin::WorldGenPlugin;

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
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests;
