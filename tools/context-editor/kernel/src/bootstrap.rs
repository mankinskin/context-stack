use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;

use crate::svo::VoxelWorld;
use crate::theme::{
    MaterialRef, MaterialRefMap, ThemePalette,
    theme_update_svo,
};
use crate::render::glass::GlassPanel;

pub struct BootstrapPlugin;

#[derive(Component)]
struct FlyCamera {
    yaw: f32,
    pitch: f32,
    move_speed: f32,
    look_sensitivity: f32,
}

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

        app.add_systems(
            Startup,
            (setup_baseline_scene, paint_palette_voxels, mark_runtime_ready).chain(),
        );
        app.add_systems(Update, (fly_camera_look, fly_camera_move, toggle_theme));
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
    // Floor / terrain
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(14.0, 0.3, 14.0))),
        MeshMaterial3d(materials.add(palette.voxel_terrain.to_standard_material())),
        Transform::from_xyz(0.0, -0.15, 0.0),
        PaletteMesh(MaterialRef::Terrain),
    ));

    // Primary cube
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(2.5, 2.5, 2.5))),
        MeshMaterial3d(materials.add(palette.voxel_primary.to_standard_material())),
        Transform::from_xyz(0.0, 1.5, 0.0),
        PaletteMesh(MaterialRef::Primary),
    ));

    // Secondary sphere
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(1.5).mesh().ico(4).unwrap())),
        MeshMaterial3d(materials.add(palette.voxel_secondary.to_standard_material())),
        Transform::from_xyz(5.0, 1.5, 0.0),
        PaletteMesh(MaterialRef::Secondary),
    ));

    // Highlight accent (metallic)
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.2, 1.2, 1.2))),
        MeshMaterial3d(materials.add(palette.voxel_highlight.to_standard_material())),
        Transform::from_xyz(-4.0, 0.6, 3.0),
        PaletteMesh(MaterialRef::Highlight),
    ));

    // Lights
    commands.spawn((
        PointLight {
            intensity: 200_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(6.0, 12.0, 6.0),
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 9_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(-4.0, 10.0, -6.0).looking_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y),
    ));

    // Camera
    commands.spawn((
        Camera3d::default(),
        bevy::core_pipeline::tonemapping::Tonemapping::None,
        Transform::from_xyz(0.0, 5.0, 15.0).looking_at(Vec3::ZERO, Vec3::Y),
        FlyCamera {
            yaw: 0.0,
            pitch: -0.3,
            move_speed: 12.0,
            look_sensitivity: 0.003,
        },
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
        Transform::from_xyz(0.0, 2.0, 4.0),
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
        Transform::from_xyz(5.0, 1.5, 2.5),
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
    // Paint palette-driven geometry into the SVO at arbitrary coordinates.
    // These are not yet visible (custom pipeline pending render-target hookup)
    // but the data path — palette → pack → color_data → dirty upload — is exercised.
    voxel_world.apply_sdf_brush(
        Vec3::new(128.0, 130.0, 128.0),
        4.0,
        palette.voxel_primary.to_voxel_material(),
    );
    voxel_world.apply_sdf_brush(
        Vec3::new(140.0, 130.0, 128.0),
        3.0,
        palette.voxel_secondary.to_voxel_material(),
    );
    voxel_world.apply_sdf_brush(
        Vec3::new(128.0, 125.0, 128.0),
        6.0,
        palette.voxel_terrain.to_voxel_material(),
    );
    voxel_world.apply_sdf_brush(
        Vec3::new(120.0, 130.0, 128.0),
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

fn fly_camera_look(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    mut camera: Query<(&mut Transform, &mut FlyCamera)>,
) {
    if !mouse_buttons.pressed(MouseButton::Left) {
        return;
    }

    let delta = mouse_motion.delta;
    if delta == Vec2::ZERO {
        return;
    }

    for (mut transform, mut fly) in &mut camera {
        fly.yaw -= delta.x * fly.look_sensitivity;
        fly.pitch = (fly.pitch - delta.y * fly.look_sensitivity).clamp(-1.54, 1.54);
        transform.rotation = Quat::from_euler(EulerRot::YXZ, fly.yaw, fly.pitch, 0.0);
    }
}

fn fly_camera_move(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut camera: Query<(&mut Transform, &FlyCamera)>,
) {
    for (mut transform, fly) in &mut camera {
        let forward = *transform.forward();
        let right = *transform.right();

        let mut move_dir = Vec3::ZERO;
        if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
            move_dir += forward;
        }
        if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
            move_dir -= forward;
        }
        if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
            move_dir += right;
        }
        if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
            move_dir -= right;
        }
        if keys.pressed(KeyCode::Space) {
            move_dir += Vec3::Y;
        }
        if keys.pressed(KeyCode::ShiftLeft) {
            move_dir -= Vec3::Y;
        }

        if move_dir != Vec3::ZERO {
            let speed_mult = if keys.pressed(KeyCode::ControlLeft) { 3.0 } else { 1.0 };
            transform.translation += move_dir.normalize() * fly.move_speed * speed_mult * time.delta_secs();
        }
    }
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
