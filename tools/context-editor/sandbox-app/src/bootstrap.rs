use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

use context_editor_kernel::{
    character::{
        CharacterController,
        CharacterPlugin,
    },
    particle_splat::{
        ParticleEmitter,
        ParticleSystem,
    },
    svo::{
        VoxelMaterial,
        VoxelWorld,
    },
    theme::{
        theme_update_svo,
        MaterialRef,
        MaterialRefMap,
        ThemePalette,
    },
    world_gen::{
        boulder_template,
        tree_template,
        MATERIAL_DIRT,
        MATERIAL_GRASS,
        MATERIAL_SAND,
        MATERIAL_STONE,
        MATERIAL_WATER,
    },
};

/// Scene center in voxel/world coordinates (middle of 4096³ SVO).
pub const SCENE_X: f32 = 2048.0;
pub const SCENE_Z: f32 = 2048.0;
/// Floor surface Y level — physics collider sits here.
pub const FLOOR_Y: f32 = 1024.0;
/// SVO ground reference — a few voxels below the physics floor so
/// the visual ground surface sits beneath the character's feet.
pub const SVO_GROUND_Y: f32 = 1021.0;

pub struct BootstrapPlugin;

/// Tags a mesh whose [`StandardMaterial`] is driven by a palette slot.
#[derive(Component)]
struct PaletteMesh(MaterialRef);

impl Plugin for BootstrapPlugin {
    fn build(
        &self,
        app: &mut App,
    ) {
        let palette = ThemePalette::dark_default();
        app.insert_resource(ClearColor(palette.ambient_color));
        app.insert_resource(palette);
        app.insert_resource(MaterialRefMap::default());

        app.add_plugins(CharacterPlugin);

        app.add_systems(
            Startup,
            (setup_baseline_scene, mark_runtime_ready).chain(),
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

fn setup_baseline_scene(mut commands: Commands) {
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

fn paint_ground_slab(
    voxel_world: &mut VoxelWorld,
    cx: i32,
    cz: i32,
    fy: i32,
    grass: VoxelMaterial,
    dirt: VoxelMaterial,
    stone: VoxelMaterial,
) {
    let half_extent = 120;
    for x in (cx - half_extent)..(cx + half_extent) {
        for z in (cz - half_extent)..(cz + half_extent) {
            for dy in 0..4 {
                voxel_world.set_voxel(IVec3::new(x, fy - dy, z), grass);
            }
            for dy in 4..8 {
                voxel_world.set_voxel(IVec3::new(x, fy - dy, z), dirt);
            }
            for dy in 8..12 {
                voxel_world.set_voxel(IVec3::new(x, fy - dy, z), stone);
            }
        }
    }
}

fn paint_stone_tower(
    voxel_world: &mut VoxelWorld,
    tower_x: i32,
    tower_z: i32,
    fy: i32,
    stone: VoxelMaterial,
) {
    paint_stone_tower_walls(voxel_world, tower_x, tower_z, fy, stone);
    paint_stone_tower_battlements(voxel_world, tower_x, tower_z, fy, stone);
}

fn paint_stone_tower_walls(
    voxel_world: &mut VoxelWorld,
    tower_x: i32,
    tower_z: i32,
    fy: i32,
    stone: VoxelMaterial,
) {
    for y in (fy + 1)..=(fy + 48) {
        for dx in -8_i32..=8 {
            for dz in -8_i32..=8 {
                let is_wall = dx <= -7 || dx >= 7 || dz <= -7 || dz >= 7;
                let is_door = dz <= -7 && dx.abs() <= 1 && y <= fy + 12;
                if is_wall && !is_door {
                    voxel_world.set_voxel(IVec3::new(tower_x + dx, y, tower_z + dz), stone);
                }
            }
        }
    }
}

fn paint_stone_tower_battlements(
    voxel_world: &mut VoxelWorld,
    tower_x: i32,
    tower_z: i32,
    fy: i32,
    stone: VoxelMaterial,
) {
    for dx in -8..=8 {
        for dz in -8..=8 {
            if should_place_battlement(dx, dz) {
                for dy in 0..4 {
                    voxel_world.set_voxel(
                        IVec3::new(tower_x + dx, fy + 49 + dy, tower_z + dz),
                        stone,
                    );
                }
            }
        }
    }
}

fn should_place_battlement(
    dx: i32,
    dz: i32,
) -> bool {
    let is_edge = dx <= -7 || dx >= 7 || dz <= -7 || dz >= 7;
    let is_corner = (dx <= -7 || dx >= 7) && (dz <= -7 || dz >= 7);
    is_edge && (is_corner || (dx + dz) % 2 == 0)
}

fn paint_stone_arch(
    voxel_world: &mut VoxelWorld,
    arch_x: i32,
    arch_z: i32,
    fy: i32,
    stone: VoxelMaterial,
) {
    for y in (fy + 1)..=(fy + 24) {
        for t in 0..4 {
            for w in 0..4 {
                voxel_world.set_voxel(IVec3::new(arch_x - 8 + t, y, arch_z + w), stone);
                voxel_world.set_voxel(IVec3::new(arch_x + 5 + t, y, arch_z + w), stone);
            }
        }
    }
    for dx in -8..=8 {
        for t in 0..4 {
            for w in 0..4 {
                voxel_world.set_voxel(IVec3::new(arch_x + dx, fy + 25 + t, arch_z + w), stone);
            }
        }
    }
}

fn paint_sand_pyramid(
    voxel_world: &mut VoxelWorld,
    pyr_x: i32,
    pyr_z: i32,
    fy: i32,
    sand: VoxelMaterial,
) {
    for layer in 0..28 {
        let r = 24 - layer;
        let y = fy + 1 + layer;
        for dx in -r..=r {
            for dz in -r..=r {
                voxel_world.set_voxel(IVec3::new(pyr_x + dx, y, pyr_z + dz), sand);
            }
        }
    }
}

fn paint_water_pool(
    voxel_world: &mut VoxelWorld,
    pool_x: i32,
    pool_z: i32,
    fy: i32,
    water: VoxelMaterial,
) {
    for dx in -12..=12 {
        for dz in -12..=12 {
            for dy in 0..8 {
                voxel_world.set_voxel(IVec3::new(pool_x + dx, fy - dy, pool_z + dz), water);
            }
        }
    }
}

fn paint_window_wall(
    voxel_world: &mut VoxelWorld,
    cx: i32,
    cz: i32,
    fy: i32,
    stone: VoxelMaterial,
) {
    let wall_z = cz + 64;
    for x in (cx - 60)..=(cx + 60) {
        for y in (fy + 1)..=(fy + 20) {
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
}

fn paint_templates(
    voxel_world: &mut VoxelWorld,
    cx: i32,
    cz: i32,
    fy: i32,
) {
    let tree = tree_template();
    tree.stamp(voxel_world, IVec3::new(cx + 20, fy + 1, cz + 24));
    tree.stamp(voxel_world, IVec3::new(cx - 12, fy + 1, cz + 40));
    tree.stamp(voxel_world, IVec3::new(cx + 60, fy + 1, cz + 12));
    tree.stamp(voxel_world, IVec3::new(cx - 72, fy + 1, cz - 20));

    let boulder = boulder_template();
    boulder.stamp(voxel_world, IVec3::new(cx + 48, fy + 1, cz - 12));
    boulder.stamp(voxel_world, IVec3::new(cx - 32, fy + 1, cz - 60));
}

fn paint_showcase_voxels(
    voxel_world: &mut VoxelWorld,
    cx: i32,
    cz: i32,
    fy: i32,
) {
    let sphere_mat = VoxelMaterial {
        r: 80,
        g: 160,
        b: 230,
        roughness: 6,
        metallic: false,
        sdf_type: 1,
    };
    for dy in 1..=4 {
        for dx in -3_i32..=3 {
            for dz in 0..3_i32 {
                voxel_world.set_voxel(IVec3::new(cx - 8 + dx, fy + dy, cz + 30 + dz), sphere_mat);
            }
        }
    }

    let torus_mat = VoxelMaterial {
        r: 230,
        g: 130,
        b: 40,
        roughness: 6,
        metallic: false,
        sdf_type: 3,
    };
    for dy in 1..=4 {
        for dx in -3_i32..=3 {
            for dz in 0..3_i32 {
                voxel_world.set_voxel(IVec3::new(cx + 8 + dx, fy + dy, cz + 30 + dz), torus_mat);
            }
        }
    }

    let metallic_mat = VoxelMaterial {
        r: 192,
        g: 192,
        b: 200,
        roughness: 2,
        metallic: true,
        sdf_type: 0,
    };
    for dy in 1..=14 {
        for dx in -8_i32..=8 {
            for dz in 0..2_i32 {
                voxel_world.set_voxel(IVec3::new(cx + dx, fy + dy, cz + 18 + dz), metallic_mat);
            }
        }
    }
}

fn paint_palette_demo(
    voxel_world: &mut VoxelWorld,
    palette: &ThemePalette,
) {
    voxel_world.apply_sdf_brush(
        Vec3::new(SCENE_X, SVO_GROUND_Y, SCENE_Z),
        16.0,
        palette.voxel_primary.to_voxel_material(),
    );
    voxel_world.apply_sdf_brush(
        Vec3::new(SCENE_X + 48.0, SVO_GROUND_Y, SCENE_Z),
        12.0,
        palette.voxel_secondary.to_voxel_material(),
    );
    voxel_world.apply_sdf_brush(
        Vec3::new(SCENE_X - 48.0, SVO_GROUND_Y, SCENE_Z),
        8.0,
        palette.voxel_highlight.to_voxel_material(),
    );
}

// ---------------------------------------------------------------------------
// SVO voxel painting — data layer for the custom render pipeline
//
// Called by the preset registry as preset 0 ("Default Scene").
// ---------------------------------------------------------------------------

pub fn paint_default_scene(world: &mut bevy::prelude::World) {
    let palette = world.resource::<ThemePalette>().clone();
    let mut voxel_world = world.resource_mut::<VoxelWorld>();
    let voxel_world = &mut *voxel_world;

    let cx = SCENE_X as i32;
    let cz = SCENE_Z as i32;
    let fy = SVO_GROUND_Y as i32;

    // --- Ground slab: 240×12×240 centered at (cx, fy, cz) --------------------
    let grass = VoxelMaterial::unpack(MATERIAL_GRASS);
    let dirt = VoxelMaterial::unpack(MATERIAL_DIRT);
    let stone = VoxelMaterial::unpack(MATERIAL_STONE);
    let sand = VoxelMaterial::unpack(MATERIAL_SAND);
    let water = VoxelMaterial::unpack(MATERIAL_WATER);
    paint_ground_slab(voxel_world, cx, cz, fy, grass, dirt, stone);
    paint_stone_tower(voxel_world, cx + 40, cz - 32, fy, stone);
    paint_stone_arch(voxel_world, cx - 24, cz + 16, fy, stone);
    paint_sand_pyramid(voxel_world, cx - 48, cz - 48, fy, sand);
    paint_water_pool(voxel_world, cx + 32, cz + 40, fy, water);
    paint_window_wall(voxel_world, cx, cz, fy, stone);
    paint_templates(voxel_world, cx, cz, fy);
    paint_showcase_voxels(voxel_world, cx, cz, fy);
    paint_palette_demo(voxel_world, &palette);

    // Rebuild the ref map so theme changes can re-pack palette voxels.
    // Drop the VoxelWorld borrow first so world is available again.
    let new_ref_map = MaterialRefMap::build_from_world(voxel_world, &palette);
    drop(voxel_world);
    *world.resource_mut::<MaterialRefMap>() = new_ref_map;
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

/// Seeds persistent ambient particle emitters so the particle pipeline is
/// exercised immediately on startup. Spawns slow-drifting cyan and magenta
/// ember particles above the scene centre.
pub fn seed_ambient_emitter(mut system: ResMut<ParticleSystem>) {
    // Camera settles at y≈257. Emitters at y=261, z=540 are safely in frustum.
    // scale=1.0 matches SVO leaf-voxel size; slow upward drift + jitter spreads
    // particles into a visible cloud rather than a single stacked pile.
    system.emitters.push(ParticleEmitter {
        origin: Vec3::new(512.0, 261.0, 540.0),
        rate: 20,
        color: [0.1, 0.9, 1.0, 1.0], // bright cyan
        scale: 1.0,
        initial_velocity: Vec3::new(0.0, 1.0, 0.0),
        lifetime: 6.0,
    });
    // Magenta cluster slightly right for visual separation.
    system.emitters.push(ParticleEmitter {
        origin: Vec3::new(518.0, 261.0, 537.0),
        rate: 15,
        color: [1.0, 0.1, 0.8, 1.0], // bright magenta
        scale: 1.0,
        initial_velocity: Vec3::new(0.3, 1.2, 0.0),
        lifetime: 6.0,
    });
}
