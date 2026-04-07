//! First-person character controller with physics-based movement.
//!
//! Uses `bevy_rapier3d`'s kinematic character controller for gravity,
//! grounding, wall sliding, and jumping. Mouse look uses pointer lock
//! on WASM targets.

use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

use crate::debug_overlay::is_free_fly_enabled;

const GRAVITY: f32 = 20.0;
const FLY_THRUST: f32 = 25.0;
const MOVE_SPEED: f32 = 8.0;
const SPRINT_MULT: f32 = 2.0;
const MOUSE_SENSITIVITY: f32 = 0.003;

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

/// Marks the player camera entity for first-person character control.
#[derive(Component)]
pub struct CharacterController {
    pub yaw: f32,
    pub pitch: f32,
    pub vertical_velocity: f32,
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct CharacterPlugin;

impl Plugin for CharacterPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (character_look, character_movement, pointer_lock_system),
        );
    }
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// Mouse-look rotation. Active when pointer is locked (WASM) or when the
/// left mouse button is held (native fallback).
fn character_look(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    mut query: Query<(&mut Transform, &mut CharacterController)>,
) {
    let delta = mouse_motion.delta;
    if delta == Vec2::ZERO {
        return;
    }

    let locked = is_pointer_locked();
    let mouse_held = mouse_buttons.pressed(MouseButton::Left);
    if !locked && !mouse_held {
        return;
    }

    for (mut transform, mut ctrl) in &mut query {
        ctrl.yaw -= delta.x * MOUSE_SENSITIVITY;
        ctrl.pitch = (ctrl.pitch - delta.y * MOUSE_SENSITIVITY).clamp(-1.54, 1.54);
        transform.rotation = Quat::from_euler(EulerRot::YXZ, ctrl.yaw, ctrl.pitch, 0.0);
    }
}

/// Physics-based WASD + gravity + jump movement, or unconstrained free-fly
/// when [`crate::debug_overlay::is_free_fly_enabled`] returns `true`.
///
/// Free-fly controls:
/// - `WASD` / `Arrow` — move along camera forward / right axes (no gravity)
/// - `Q` / `E` — move down / up in world-space
/// - `Shift` — sprint multiplier
fn character_movement(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut query: Query<(
        &mut CharacterController,
        &mut KinematicCharacterController,
        &Transform,
        Option<&KinematicCharacterControllerOutput>,
    )>,
) {
    for (mut ctrl, mut kcc, transform, output) in &mut query {
        let forward = *transform.forward();
        let right   = *transform.right();
        let up      = Vec3::Y;

        let mut move_dir = Vec3::ZERO;
        if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp)    { move_dir += forward; }
        if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown)  { move_dir -= forward; }
        if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) { move_dir += right; }
        if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft)  { move_dir -= right; }

        let speed = MOVE_SPEED
            * if keys.pressed(KeyCode::ShiftLeft) { SPRINT_MULT } else { 1.0 };

        let displacement = if is_free_fly_enabled() {
            // --- Free-fly mode: full 3-axis movement, no gravity ---
            // Keep the horizontal contribution from the camera-forward direction.
            // Q = descend, E = ascend (world-space Y).
            if keys.pressed(KeyCode::KeyQ) { move_dir -= up; }
            if keys.pressed(KeyCode::KeyE) { move_dir += up; }

            // Reset vertical velocity so physics mode resumes correctly on toggle.
            ctrl.vertical_velocity = 0.0;

            if move_dir.length_squared() > 0.0 {
                move_dir.normalize() * speed * time.delta_secs()
            } else {
                Vec3::ZERO
            }
        } else {
            // --- Physics mode: gravity + ground detection ---
            move_dir.y = 0.0;
            if move_dir.length_squared() > 0.0 {
                move_dir = move_dir.normalize();
            }

            let grounded = output.map_or(false, |o| o.grounded);

            if keys.pressed(KeyCode::Space) {
                ctrl.vertical_velocity += FLY_THRUST * time.delta_secs();
            }
            ctrl.vertical_velocity -= GRAVITY * time.delta_secs();
            if grounded && ctrl.vertical_velocity < 0.0 {
                ctrl.vertical_velocity = 0.0;
            }

            move_dir * speed * time.delta_secs()
                + Vec3::Y * ctrl.vertical_velocity * time.delta_secs()
        };

        kcc.translation = Some(displacement);
    }
}

/// Request pointer lock on canvas click, release on Escape.
fn pointer_lock_system(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    if mouse.just_pressed(MouseButton::Left) {
        request_pointer_lock();
    }
    if keys.just_pressed(KeyCode::Escape) {
        exit_pointer_lock();
    }
}

// ---------------------------------------------------------------------------
// Pointer lock helpers (WASM)
// ---------------------------------------------------------------------------

fn is_pointer_locked() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        return web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.pointer_lock_element())
            .is_some();
    }
    #[cfg(not(target_arch = "wasm32"))]
    false
}

fn request_pointer_lock() {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(canvas) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.query_selector("#bevy-canvas").ok().flatten())
        {
            canvas.request_pointer_lock();
        }
    }
}

fn exit_pointer_lock() {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
            doc.exit_pointer_lock();
        }
    }
}
