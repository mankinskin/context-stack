use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;

pub struct BootstrapPlugin;

#[derive(Component)]
struct FlyCamera {
    yaw: f32,
    pitch: f32,
    move_speed: f32,
    look_sensitivity: f32,
}

impl Plugin for BootstrapPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(Color::srgb(0.03, 0.04, 0.08)));
        app.add_systems(Startup, (setup_baseline_scene, mark_runtime_ready));
        app.add_systems(Update, (fly_camera_look, fly_camera_move));
    }
}

// Baseline scene guarantees visible geometry while custom graph stages are in progress.
fn setup_baseline_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(2.5, 2.5, 2.5))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.2, 0.7, 0.9),
            metallic: 0.1,
            perceptual_roughness: 0.35,
            ..default()
        })),
        Transform::from_xyz(0.0, 1.5, 0.0),
    ));

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
