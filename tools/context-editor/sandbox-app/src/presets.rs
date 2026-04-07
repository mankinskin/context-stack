//! World preset functions for the sandbox-app.
//!
//! Each function has the signature `fn(&mut bevy::prelude::World)` and is
//! registered with the kernel via [`context_editor_kernel::register_world_presets`]
//! before calling [`context_editor_kernel::launch`].
//!
//! Preset 0 — "Default Scene" is the rich bootstrap scene defined in
//! [`super::bootstrap::paint_default_scene`].
//!
//! Presets 1–4 are pure terrain variants that centre on the same scene
//! coordinates so the player spawns in a sensible location.

use bevy::prelude::*;
use context_editor_kernel::svo::{VoxelMaterial, VoxelWorld};

use crate::bootstrap::{SCENE_X, SCENE_Z, SVO_GROUND_Y};

// ---------------------------------------------------------------------------
// Preset 1 — Terrain
// ---------------------------------------------------------------------------

/// Rolling noise hills — grass, dirt, stone, sand biomes.
pub fn paint_terrain(world: &mut World) {
    let mut vw = world.resource_mut::<VoxelWorld>();
    let cx = SCENE_X as i32;
    let cz = SCENE_Z as i32;
    let fy = SVO_GROUND_Y as i32;

    let half = 120_i32;
    let grass = VoxelMaterial::new(60, 140, 40, 18);
    let dirt  = VoxelMaterial::new(100, 70, 40, 22);
    let stone = VoxelMaterial::new(128, 128, 130, 28);
    let sand  = VoxelMaterial::new(210, 190, 140, 24);

    for x in (cx - half)..(cx + half) {
        for z in (cz - half)..(cz + half) {
            let dx = (x - cx) as f32;
            let dz = (z - cz) as f32;
            let h = ((dx * 0.04).sin() * 3.0
                + (dz * 0.05).cos() * 3.0
                + ((dx * 0.015 + dz * 0.02).sin() * 5.0)) as i32;
            let surface_y = fy + h;
            let top_mat = if h >= 4 { stone } else if h <= -2 { sand } else { grass };
            vw.set_voxel(IVec3::new(x, surface_y, z), top_mat);
            for dy in 1..4_i32 {
                vw.set_voxel(IVec3::new(x, surface_y - dy, z), dirt);
            }
            for dy in 4..12_i32 {
                vw.set_voxel(IVec3::new(x, surface_y - dy, z), stone);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Preset 2 — Flat
// ---------------------------------------------------------------------------

/// Uniform grass/dirt slab — good for building and testing.
pub fn paint_flat(world: &mut World) {
    let mut vw = world.resource_mut::<VoxelWorld>();
    let cx = SCENE_X as i32;
    let cz = SCENE_Z as i32;
    let fy = SVO_GROUND_Y as i32;

    let half = 120_i32;
    let grass = VoxelMaterial::new(60, 140, 40, 18);
    let dirt  = VoxelMaterial::new(100, 70, 40, 22);

    for x in (cx - half)..(cx + half) {
        for z in (cz - half)..(cz + half) {
            for dy in 0..2_i32 {
                vw.set_voxel(IVec3::new(x, fy - dy, z), grass);
            }
            for dy in 2..6_i32 {
                vw.set_voxel(IVec3::new(x, fy - dy, z), dirt);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Preset 3 — Caves
// ---------------------------------------------------------------------------

/// Sine-wave surface with carved cave tunnels.
pub fn paint_caves(world: &mut World) {
    let mut vw = world.resource_mut::<VoxelWorld>();
    let cx = SCENE_X as i32;
    let cz = SCENE_Z as i32;
    let fy = SVO_GROUND_Y as i32;

    let half = 96_i32;
    let stone = VoxelMaterial::new(128, 128, 130, 28);
    let grass = VoxelMaterial::new(60, 140, 40, 18);

    for x in (cx - half)..(cx + half) {
        for z in (cz - half)..(cz + half) {
            let h_offset = ((x as f32 * 0.08).sin() * (z as f32 * 0.06).cos() * 4.0) as i32;
            let surface_y = fy + h_offset;
            vw.set_voxel(IVec3::new(x, surface_y, z), grass);
            for dy in 1..14_i32 {
                let y = surface_y - dy;
                let is_cave = ((x as f32 * 0.15).sin().abs()
                    + (z as f32 * 0.12).cos().abs()) > 1.6
                    && dy >= 4 && dy <= 9;
                if !is_cave {
                    vw.set_voxel(IVec3::new(x, y, z), stone);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Preset 4 — Empty
// ---------------------------------------------------------------------------

/// Cleared world — VoxelWorld is already reset by the kernel before this runs.
pub fn paint_empty(_world: &mut World) {}
