use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

use crate::svo::{VoxelMaterial, VoxelWorld};
use crate::theme::{
    MaterialRef, MaterialRefMap, ThemePalette,
    theme_update_svo,
};
use crate::character::{CharacterController, CharacterPlugin};
use crate::world_gen::{
    tree_template, boulder_template,
    MATERIAL_GRASS, MATERIAL_STONE, MATERIAL_DIRT, MATERIAL_SAND, MATERIAL_WATER,
};

/// Scene center in voxel/world coordinates (middle of 1024³ SVO).
const SCENE_X: f32 = 512.0;
const SCENE_Z: f32 = 512.0;
/// Floor surface Y level — physics collider sits here.
const FLOOR_Y: f32 = 256.0;
/// SVO ground reference — a few voxels below the physics floor so
/// the visual ground surface sits beneath the character's feet.
const SVO_GROUND_Y: f32 = 253.0;

pub struct BootstrapPlugin;

/// Tags a mesh whose [`StandardMaterial`] is driven by a palette slot.
#[derive(Component)]
struct PaletteMesh(MaterialRef);


impl Plugin for BootstrapPlugin {
    fn build(&self, app: &mut App) {
        let palette = ThemePalette::dark_default();
        app.insert_resource(ClearColor(palette.ambient_color));
        app.insert_resource(palette);
        app.insert_resource(MaterialRefMap::default());

        app.add_plugins(CharacterPlugin);

        app.add_systems(
            Startup,
            (setup_baseline_scene, paint_palette_voxels, mark_runtime_ready).chain(),
        );
        app.add_systems(Update, toggle_theme);
        app.add_systems(
            PostUpdate,
            (theme_update_svo, sync_palette_materials, sync_clear_color),
        );
    }
}

// ---------------------------------------------------------------------------
// Scene setup — multiple objects driven by palette materials
// ---------------------------------------------------------------------------

fn setup_baseline_scene(
    mut commands: Commands,
) {
    // Floor / terrain (with physics collider) — centered in SVO world
    // Keep as invisible physics ground plane; voxel terrain is rendered by the
    // splat pipeline from the SVO data painted in paint_palette_voxels().
    commands.spawn((
        Transform::from_xyz(SCENE_X, FLOOR_Y - 0.15, SCENE_Z),
        RigidBody::Fixed,
        Collider::cuboid(160.0, 0.15, 160.0),
    ));

    // Primary, secondary, highlight objects are now voxelized SDFs
    // in the SVO (see paint_palette_voxels). No forward-rendered meshes here.

    // Lights
    commands.spawn((
        PointLight {
            intensity: 200_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(SCENE_X + 24.0, FLOOR_Y + 80.0, SCENE_Z + 24.0),
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 9_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(SCENE_X - 16.0, FLOOR_Y + 72.0, SCENE_Z - 24.0)
            .looking_at(Vec3::new(SCENE_X, FLOOR_Y + 4.0, SCENE_Z), Vec3::Y),
    ));

    // Camera — first-person character controller with physics
    commands.spawn((
        Camera3d::default(),
        bevy::core_pipeline::tonemapping::Tonemapping::None,
        Transform::from_xyz(SCENE_X, FLOOR_Y + 20.0, SCENE_Z + 60.0)
            .looking_at(Vec3::new(SCENE_X, FLOOR_Y + 4.0, SCENE_Z), Vec3::Y),
        CharacterController {
            yaw: 0.0,
            pitch: -0.3,
            vertical_velocity: 0.0,
        },
        RigidBody::KinematicPositionBased,
        Collider::capsule_y(0.8, 0.3),
        KinematicCharacterController::default(),
    ));

}

// ---------------------------------------------------------------------------
// SVO voxel painting — data layer for the custom render pipeline
// ---------------------------------------------------------------------------

fn paint_palette_voxels(
    mut voxel_world: ResMut<VoxelWorld>,
    palette: Res<ThemePalette>,
    mut ref_map: ResMut<MaterialRefMap>,
) {
    let cx = SCENE_X as i32; // 512
    let cz = SCENE_Z as i32; // 512
    let fy = SVO_GROUND_Y as i32; // 253 — visual ground below physics floor

    // --- Ground slab: 240×12×240 centered at (cx, fy, cz) --------------------
    let grass = VoxelMaterial::unpack(MATERIAL_GRASS);
    let dirt = VoxelMaterial::unpack(MATERIAL_DIRT);
    let stone = VoxelMaterial::unpack(MATERIAL_STONE);

    let half_extent = 120;
    for x in (cx - half_extent)..(cx + half_extent) {
        for z in (cz - half_extent)..(cz + half_extent) {
            // Top 4 layers: grass
            for dy in 0..4 {
                voxel_world.set_voxel(IVec3::new(x, fy - dy, z), grass);
            }
            // Next 4 layers: dirt
            for dy in 4..8 {
                voxel_world.set_voxel(IVec3::new(x, fy - dy, z), dirt);
            }
            // Bottom 4 layers: stone
            for dy in 8..12 {
                voxel_world.set_voxel(IVec3::new(x, fy - dy, z), stone);
            }
        }
    }

    // --- Stone tower (hollow, 20×48×20 with doorway) -------------------------
    let tower_x = cx + 40;
    let tower_z = cz - 32;
    for y in (fy + 1)..=(fy + 48) {
        for dx in -8_i32..=8 {
            for dz in -8_i32..=8 {
                let is_wall = dx <= -7 || dx >= 7 || dz <= -7 || dz >= 7;
                // Doorway on south face, 4 blocks wide, 12 tall
                let is_door = dz <= -7 && dx.abs() <= 1 && y <= fy + 12;
                if is_wall && !is_door {
                    voxel_world.set_voxel(IVec3::new(tower_x + dx, y, tower_z + dz), stone);
                }
            }
        }
    }
    // Tower battlement ring
    for dx in -8..=8 {
        for dz in -8..=8 {
            let is_edge = dx <= -7 || dx >= 7 || dz <= -7 || dz >= 7;
            let is_corner = (dx <= -7 || dx >= 7) && (dz <= -7 || dz >= 7);
            if is_edge && (is_corner || (dx + dz) % 2 == 0) {
                for dy in 0..4 {
                    voxel_world.set_voxel(
                        IVec3::new(tower_x + dx, fy + 49 + dy, tower_z + dz),
                        stone,
                    );
                }
            }
        }
    }

    // --- Stone arch (gateway) -------------------------------------------------
    let arch_x = cx - 24;
    let arch_z = cz + 16;
    // Two pillars (4 voxels thick)
    for y in (fy + 1)..=(fy + 24) {
        for t in 0..4 {
            for w in 0..4 {
                voxel_world.set_voxel(IVec3::new(arch_x - 8 + t, y, arch_z + w), stone);
                voxel_world.set_voxel(IVec3::new(arch_x + 5 + t, y, arch_z + w), stone);
            }
        }
    }
    // Lintel
    for dx in -8..=8 {
        for t in 0..4 {
            for w in 0..4 {
                voxel_world.set_voxel(IVec3::new(arch_x + dx, fy + 25 + t, arch_z + w), stone);
            }
        }
    }

    // --- Stepped pyramid (sand, 28 layers) ------------------------------------
    let sand = VoxelMaterial::unpack(MATERIAL_SAND);
    let pyr_x = cx - 48;
    let pyr_z = cz - 48;
    for layer in 0..28 {
        let r = 24 - layer;
        let y = fy + 1 + layer;
        for dx in -r..=r {
            for dz in -r..=r {
                voxel_world.set_voxel(IVec3::new(pyr_x + dx, y, pyr_z + dz), sand);
            }
        }
    }

    // --- Water pool -----------------------------------------------------------
    let water = VoxelMaterial::unpack(MATERIAL_WATER);
    let pool_x = cx + 32;
    let pool_z = cz + 40;
    for dx in -12..=12 {
        for dz in -12..=12 {
            // Carve 8 blocks deep, fill with water
            for dy in 0..8 {
                voxel_world.set_voxel(IVec3::new(pool_x + dx, fy - dy, pool_z + dz), water);
            }
        }
    }

    // --- Stone wall with windows (along X axis) -------------------------------
    let wall_z = cz + 64;
    for x in (cx - 60)..=(cx + 60) {
        for y in (fy + 1)..=(fy + 20) {
            // Window gaps every 24 blocks, 8 wide, at heights 9-16
            let rel_x = (x - (cx - 60)) % 24;
            let rel_y = y - fy;
            let is_window = (rel_x >= 8 && rel_x <= 15) && (rel_y >= 9 && rel_y <= 16);
            if !is_window {
                for w in 0..4 {
                    voxel_world.set_voxel(IVec3::new(x, y, wall_z + w), stone);
                }
            }
        }
    }

    // --- Trees (use template from world_gen) ----------------------------------
    let tree = tree_template();
    tree.stamp(&mut voxel_world, IVec3::new(cx + 20, fy + 1, cz + 24));
    tree.stamp(&mut voxel_world, IVec3::new(cx - 12, fy + 1, cz + 40));
    tree.stamp(&mut voxel_world, IVec3::new(cx + 60, fy + 1, cz + 12));
    tree.stamp(&mut voxel_world, IVec3::new(cx - 72, fy + 1, cz - 20));

    // --- Boulders (use template from world_gen) -------------------------------
    let boulder = boulder_template();
    boulder.stamp(&mut voxel_world, IVec3::new(cx + 48, fy + 1, cz - 12));
    boulder.stamp(&mut voxel_world, IVec3::new(cx - 32, fy + 1, cz - 60));

    // --- Palette demo spheres (original) --------------------------------------
    voxel_world.apply_sdf_brush(
        Vec3::new(SCENE_X, SVO_GROUND_Y + 32.0, SCENE_Z),
        16.0,
        palette.voxel_primary.to_voxel_material(),
    );
    voxel_world.apply_sdf_brush(
        Vec3::new(SCENE_X + 48.0, SVO_GROUND_Y + 32.0, SCENE_Z),
        12.0,
        palette.voxel_secondary.to_voxel_material(),
    );
    voxel_world.apply_sdf_brush(
        Vec3::new(SCENE_X - 48.0, SVO_GROUND_Y + 32.0, SCENE_Z),
        8.0,
        palette.voxel_highlight.to_voxel_material(),
    );

    // Build the ref map so theme changes can re-pack these voxels.
    *ref_map = MaterialRefMap::build_from_world(&voxel_world, &palette);
}

// ---------------------------------------------------------------------------
// Theme toggle — press T to cycle dark → light → high-contrast
// ---------------------------------------------------------------------------

fn toggle_theme(
    keys: Res<ButtonInput<KeyCode>>,
    mut palette: ResMut<ThemePalette>,
    mut theme_idx: Local<usize>,
) {
    if keys.just_pressed(KeyCode::KeyT) {
        *theme_idx = (*theme_idx + 1) % 3;
        *palette = match *theme_idx {
            1 => ThemePalette::light_default(),
            2 => ThemePalette::high_contrast(),
            _ => ThemePalette::dark_default(),
        };
    }
}

// ---------------------------------------------------------------------------
// Palette → StandardMaterial sync (visual validation via Bevy's renderer)
// ---------------------------------------------------------------------------

fn sync_palette_materials(
    palette: Res<ThemePalette>,
    query: Query<(&PaletteMesh, &MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if !palette.is_changed() {
        return;
    }
    for (pm, mat_handle) in &query {
        let def = palette.resolve(pm.0);
        if let Some(mat) = materials.get_mut(&mat_handle.0) {
            mat.base_color = def.to_bevy_color();
            mat.metallic = if def.metallic { 1.0 } else { 0.0 };
            mat.perceptual_roughness = def.roughness;
        }
    }
}

fn sync_clear_color(
    palette: Res<ThemePalette>,
    mut clear_color: ResMut<ClearColor>,
) {
    if !palette.is_changed() {
        return;
    }
    clear_color.0 = palette.ambient_color;
}

fn mark_runtime_ready() {
    if let Some(window) = web_sys::window() {
        if let Some(document) = window.document() {
            if let Some(body) = document.body() {
                let _ = body.set_attribute("data-bevy-ready", "true");
            }
        }
    }
}
