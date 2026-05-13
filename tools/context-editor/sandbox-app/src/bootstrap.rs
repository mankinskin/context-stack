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
                    voxel_world.set_voxel(
                        IVec3::new(tower_x + dx, y, tower_z + dz),
                        stone,
                    );
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
                voxel_world.set_voxel(
                    IVec3::new(arch_x - 8 + t, y, arch_z + w),
                    stone,
                );
                voxel_world.set_voxel(
                    IVec3::new(arch_x + 5 + t, y, arch_z + w),
                    stone,
                );
            }
        }
    }
    // Lintel
    for dx in -8..=8 {
        for t in 0..4 {
            for w in 0..4 {
                voxel_world.set_voxel(
                    IVec3::new(arch_x + dx, fy + 25 + t, arch_z + w),
                    stone,
                );
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
                voxel_world
                    .set_voxel(IVec3::new(pyr_x + dx, y, pyr_z + dz), sand);
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
                voxel_world.set_voxel(
                    IVec3::new(pool_x + dx, fy - dy, pool_z + dz),
                    water,
                );
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
            let is_window =
                (rel_x >= 8 && rel_x <= 15) && (rel_y >= 9 && rel_y <= 16);
            if !is_window {
                for w in 0..4 {
                    voxel_world.set_voxel(IVec3::new(x, y, wall_z + w), stone);
                }
            }
        }
    }

    // --- Trees (use template from world_gen) ----------------------------------
    let tree = tree_template();
    tree.stamp(voxel_world, IVec3::new(cx + 20, fy + 1, cz + 24));
    tree.stamp(voxel_world, IVec3::new(cx - 12, fy + 1, cz + 40));
    tree.stamp(voxel_world, IVec3::new(cx + 60, fy + 1, cz + 12));
    tree.stamp(voxel_world, IVec3::new(cx - 72, fy + 1, cz - 20));

    // --- Boulders (use template from world_gen) -------------------------------
    let boulder = boulder_template();
    boulder.stamp(voxel_world, IVec3::new(cx + 48, fy + 1, cz - 12));
    boulder.stamp(voxel_world, IVec3::new(cx - 32, fy + 1, cz - 60));

    // --- SDF-type showcase (Phase 2a/2b visual verification) ------------------
    // These voxels demonstrate sphere SDF (type 1), torus SDF (type 3), and
    // metallic reflections.  Placed at ground level, visible from the default
    // camera position.

    // Sphere-type voxels: a 7×4×3 block of blue-white spheroids to the left.
    // cx-8 → viewing angle ≈11° from camera centre, well within any FOV.
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
                voxel_world.set_voxel(
                    IVec3::new(cx - 8 + dx, fy + dy, cz + 30 + dz),
                    sphere_mat,
                );
            }
        }
    }

    // Torus-type voxels: a 7×4×3 block of amber tori to the right.
    // cx+8 → same ~11° from centre, symmetric with spheres.
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
                voxel_world.set_voxel(
                    IVec3::new(cx + 8 + dx, fy + dy, cz + 30 + dz),
                    torus_mat,
                );
            }
        }
    }

    // Metallic-reflective wall: a 17×14×2 silver slab as a backdrop behind
    // the showcase blocks, close enough to show reflections in ray march mode.
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
                voxel_world.set_voxel(
                    IVec3::new(cx + dx, fy + dy, cz + 18 + dz),
                    metallic_mat,
                );
            }
        }
    }

    // --- Palette demo spheres -------------------------------------------------
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
