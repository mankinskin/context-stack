use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

use crate::svo::{VoxelMaterial, VoxelWorld};
use crate::theme::{
    MaterialRef, MaterialRefMap, ThemePalette,
    theme_update_svo,
};
use crate::render::glass::GlassPanel;
use crate::character::{CharacterController, CharacterPlugin};
use crate::world_gen::{
    tree_template, boulder_template,
    MATERIAL_GRASS, MATERIAL_STONE, MATERIAL_DIRT, MATERIAL_SAND, MATERIAL_WATER,
};

/// Scene center in voxel/world coordinates (middle of 256³ SVO).
const SCENE_X: f32 = 128.0;
const SCENE_Z: f32 = 128.0;
/// Floor surface Y level (matches WorldGenerator base height).
const FLOOR_Y: f32 = 64.0;

pub struct BootstrapPlugin;

/// Tags a mesh whose [`StandardMaterial`] is driven by a palette slot.
#[derive(Component)]
struct PaletteMesh(MaterialRef);

/// Extract sRGBA components from a palette Color (always created via `Color::srgba`).
fn color_to_rgba(color: &Color) -> [f32; 4] {
    match color {
        Color::Srgba(c) => [c.red, c.green, c.blue, c.alpha],
        _ => [0.5, 0.5, 0.5, 1.0],
    }
}

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
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    palette: Res<ThemePalette>,
) {
    // Floor / terrain (with physics collider) — centered in SVO world
    // Keep as invisible physics ground plane; voxel terrain is rendered by the
    // splat pipeline from the SVO data painted in paint_palette_voxels().
    commands.spawn((
        Transform::from_xyz(SCENE_X, FLOOR_Y - 0.15, SCENE_Z),
        RigidBody::Fixed,
        Collider::cuboid(40.0, 0.15, 40.0),
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
        Transform::from_xyz(SCENE_X + 6.0, FLOOR_Y + 20.0, SCENE_Z + 6.0),
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 9_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(SCENE_X - 4.0, FLOOR_Y + 18.0, SCENE_Z - 6.0)
            .looking_at(Vec3::new(SCENE_X, FLOOR_Y + 1.0, SCENE_Z), Vec3::Y),
    ));

    // Camera — first-person character controller with physics
    commands.spawn((
        Camera3d::default(),
        bevy::core_pipeline::tonemapping::Tonemapping::None,
        Transform::from_xyz(SCENE_X, FLOOR_Y + 5.0, SCENE_Z + 15.0)
            .looking_at(Vec3::new(SCENE_X, FLOOR_Y + 1.0, SCENE_Z), Vec3::Y),
        CharacterController {
            yaw: 0.0,
            pitch: -0.3,
            vertical_velocity: 0.0,
        },
        RigidBody::KinematicPositionBased,
        Collider::capsule_y(0.4, 0.3),
        KinematicCharacterController::default(),
    ));

    // Glass panel 1 — clear glass in front of the primary cube
    let glass_tint = color_to_rgba(&palette.glass_tint);
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(3.0, 2.0, 0.05))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: palette.glass_tint,
            alpha_mode: AlphaMode::Blend,
            ..default()
        })),
        Transform::from_xyz(SCENE_X, FLOOR_Y + 2.0, SCENE_Z + 4.0),
        GlassPanel {
            ior: 1.5,
            tint: glass_tint,
            blur_roughness: 0.0,
            corner_radius: 0.15,
            half_size: Vec3::new(1.5, 1.0, 0.025),
            caustic_strength: 5.0,
            chromatic_spread: 1.2,
        },
    ));

    // Glass panel 2 — frosted glass near the secondary sphere
    let frosted_tint = color_to_rgba(&palette.glass_frosted_tint);
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(2.0, 2.5, 0.05))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: palette.glass_frosted_tint,
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.8,
            ..default()
        })),
        Transform::from_xyz(SCENE_X + 5.0, FLOOR_Y + 1.5, SCENE_Z + 2.5),
        GlassPanel {
            ior: 1.33,
            tint: frosted_tint,
            blur_roughness: 0.4,
            corner_radius: 0.2,
            half_size: Vec3::new(1.0, 1.25, 0.025),
            caustic_strength: 3.0,
            chromatic_spread: 0.0,
        },
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
    let cx = SCENE_X as i32; // 128
    let cz = SCENE_Z as i32; // 128
    let fy = FLOOR_Y as i32; // 64

    // --- Ground slab: 60×3×60 centered at (cx, fy, cz) -----------------------
    let grass = VoxelMaterial::unpack(MATERIAL_GRASS);
    let dirt = VoxelMaterial::unpack(MATERIAL_DIRT);
    let stone = VoxelMaterial::unpack(MATERIAL_STONE);

    let half_extent = 30;
    for x in (cx - half_extent)..(cx + half_extent) {
        for z in (cz - half_extent)..(cz + half_extent) {
            // Top layer: grass
            voxel_world.set_voxel(IVec3::new(x, fy, z), grass);
            // Sub-surface: dirt
            voxel_world.set_voxel(IVec3::new(x, fy - 1, z), dirt);
            // Bedrock
            voxel_world.set_voxel(IVec3::new(x, fy - 2, z), stone);
        }
    }

    // --- Stone tower (hollow, 5×12×5 with doorway) ----------------------------
    let tower_x = cx + 10;
    let tower_z = cz - 8;
    for y in (fy + 1)..=(fy + 12) {
        for dx in -2..=2 {
            for dz in -2..=2 {
                let is_wall = dx == -2 || dx == 2 || dz == -2 || dz == 2;
                // Doorway on south face, 2 blocks wide, 3 tall
                let is_door = dz == -2 && dx == 0 && y <= fy + 3;
                if is_wall && !is_door {
                    voxel_world.set_voxel(IVec3::new(tower_x + dx, y, tower_z + dz), stone);
                }
            }
        }
    }
    // Tower battlement ring
    for dx in -2..=2 {
        for dz in -2..=2 {
            let is_edge = dx == -2 || dx == 2 || dz == -2 || dz == 2;
            let is_corner = (dx == -2 || dx == 2) && (dz == -2 || dz == 2);
            if is_edge && (is_corner || (dx + dz) % 2 == 0) {
                voxel_world.set_voxel(
                    IVec3::new(tower_x + dx, fy + 13, tower_z + dz),
                    stone,
                );
            }
        }
    }

    // --- Stone arch (gateway) -------------------------------------------------
    let arch_x = cx - 6;
    let arch_z = cz + 4;
    // Two pillars
    for y in (fy + 1)..=(fy + 6) {
        voxel_world.set_voxel(IVec3::new(arch_x - 2, y, arch_z), stone);
        voxel_world.set_voxel(IVec3::new(arch_x + 2, y, arch_z), stone);
    }
    // Lintel
    for dx in -2..=2 {
        voxel_world.set_voxel(IVec3::new(arch_x + dx, fy + 7, arch_z), stone);
    }

    // --- Stepped pyramid (sand, 7 layers) -------------------------------------
    let sand = VoxelMaterial::unpack(MATERIAL_SAND);
    let pyr_x = cx - 12;
    let pyr_z = cz - 12;
    for layer in 0..7 {
        let r = 6 - layer;
        let y = fy + 1 + layer;
        for dx in -r..=r {
            for dz in -r..=r {
                voxel_world.set_voxel(IVec3::new(pyr_x + dx, y, pyr_z + dz), sand);
            }
        }
    }

    // --- Water pool -----------------------------------------------------------
    let water = VoxelMaterial::unpack(MATERIAL_WATER);
    let pool_x = cx + 8;
    let pool_z = cz + 10;
    for dx in -3..=3 {
        for dz in -3..=3 {
            // Carve 2 blocks deep, fill with water
            voxel_world.set_voxel(IVec3::new(pool_x + dx, fy, pool_z + dz), water);
            voxel_world.set_voxel(IVec3::new(pool_x + dx, fy - 1, pool_z + dz), water);
        }
    }

    // --- Stone wall with windows (along X axis) -------------------------------
    let wall_z = cz + 16;
    for x in (cx - 15)..=(cx + 15) {
        for y in (fy + 1)..=(fy + 5) {
            // Window gaps every 6 blocks, 2 wide, at heights 3-4
            let rel_x = (x - (cx - 15)) % 6;
            let is_window = (rel_x == 2 || rel_x == 3) && (y == fy + 3 || y == fy + 4);
            if !is_window {
                voxel_world.set_voxel(IVec3::new(x, y, wall_z), stone);
            }
        }
    }

    // --- Trees (use template from world_gen) ----------------------------------
    let tree = tree_template();
    tree.stamp(&mut voxel_world, IVec3::new(cx + 5, fy + 1, cz + 6));
    tree.stamp(&mut voxel_world, IVec3::new(cx - 3, fy + 1, cz + 10));
    tree.stamp(&mut voxel_world, IVec3::new(cx + 15, fy + 1, cz + 3));
    tree.stamp(&mut voxel_world, IVec3::new(cx - 18, fy + 1, cz - 5));

    // --- Boulders (use template from world_gen) -------------------------------
    let boulder = boulder_template();
    boulder.stamp(&mut voxel_world, IVec3::new(cx + 12, fy + 1, cz - 3));
    boulder.stamp(&mut voxel_world, IVec3::new(cx - 8, fy + 1, cz - 15));

    // --- Palette demo spheres (original) --------------------------------------
    voxel_world.apply_sdf_brush(
        Vec3::new(SCENE_X, FLOOR_Y + 8.0, SCENE_Z),
        4.0,
        palette.voxel_primary.to_voxel_material(),
    );
    voxel_world.apply_sdf_brush(
        Vec3::new(SCENE_X + 12.0, FLOOR_Y + 8.0, SCENE_Z),
        3.0,
        palette.voxel_secondary.to_voxel_material(),
    );
    voxel_world.apply_sdf_brush(
        Vec3::new(SCENE_X - 12.0, FLOOR_Y + 8.0, SCENE_Z),
        2.0,
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
