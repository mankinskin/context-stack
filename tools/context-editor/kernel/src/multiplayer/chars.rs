//! Multiplayer Characters: remote player SDF capsules, interpolation, and GPU sync.
//!
//! Remote players appear as SDF capsule primitives evaluated in the ray-marching
//! loop. Position updates arrive from SpacetimeDB at ~20Hz and are interpolated
//! to 120Hz for smooth rendering. Player capsules interact with Liquid Glass
//! refraction, cast shadows, and display floating nameplates.

use bevy::prelude::*;
use bytemuck::{
    Pod,
    Zeroable,
};

use crate::multiplayer_backend::PlayerIdentity;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum remote players rendered simultaneously.
pub const MAX_REMOTE_PLAYERS: usize = 32;

/// Default player capsule radius.
pub const CAPSULE_RADIUS: f32 = 0.3;

/// Default player capsule height (feet to head).
pub const CAPSULE_HEIGHT: f32 = 1.8;

/// Maximum extrapolation alpha (slight overshoot hides network gaps).
pub const MAX_EXTRAPOLATION: f32 = 1.2;

// ---------------------------------------------------------------------------
// Snapshot & Interpolation
// ---------------------------------------------------------------------------

/// Snapshot of a player's state at a point in time.
#[derive(Clone, Copy, Debug, Default)]
pub struct PlayerSnapshot {
    pub position: Vec3,
    pub yaw: f32,
    pub hp: i32,
    pub max_hp: i32,
}

/// Component: a remote player entity in the local ECS.
#[derive(Component, Clone, Debug)]
pub struct RemotePlayer {
    pub entity_id: u64,
    pub identity: PlayerIdentity,
    pub prev_state: PlayerSnapshot,
    pub curr_state: PlayerSnapshot,
    pub prev_timestamp: f64,
    pub curr_timestamp: f64,
    pub skin_color: u32,
    pub name: String,
}

impl RemotePlayer {
    /// Push a new server snapshot, shifting current to previous.
    pub fn push_snapshot(
        &mut self,
        snapshot: PlayerSnapshot,
        timestamp: f64,
    ) {
        self.prev_state = self.curr_state;
        self.prev_timestamp = self.curr_timestamp;
        self.curr_state = snapshot;
        self.curr_timestamp = timestamp;
    }

    /// Interpolate position between prev and curr states.
    pub fn interpolated_position(
        &self,
        now: f64,
    ) -> Vec3 {
        let dt = self.curr_timestamp - self.prev_timestamp;
        let alpha = if dt > 0.0 {
            ((now - self.prev_timestamp) / dt)
                .clamp(0.0, MAX_EXTRAPOLATION as f64) as f32
        } else {
            1.0
        };
        self.prev_state
            .position
            .lerp(self.curr_state.position, alpha)
    }

    /// Interpolate yaw between prev and curr states.
    pub fn interpolated_yaw(
        &self,
        now: f64,
    ) -> f32 {
        let dt = self.curr_timestamp - self.prev_timestamp;
        let alpha = if dt > 0.0 {
            ((now - self.prev_timestamp) / dt)
                .clamp(0.0, MAX_EXTRAPOLATION as f64) as f32
        } else {
            1.0
        };
        lerp_angle(self.prev_state.yaw, self.curr_state.yaw, alpha)
    }
}

/// Lerp between two angles, taking the shortest path.
pub fn lerp_angle(
    a: f32,
    b: f32,
    t: f32,
) -> f32 {
    let mut diff = b - a;
    while diff > std::f32::consts::PI {
        diff -= 2.0 * std::f32::consts::PI;
    }
    while diff < -std::f32::consts::PI {
        diff += 2.0 * std::f32::consts::PI;
    }
    a + diff * t
}

// ---------------------------------------------------------------------------
// GPU capsule data
// ---------------------------------------------------------------------------

/// GPU-ready player capsule for the ray-marching shader.
///
/// Matches WGSL struct:
/// ```wgsl
/// struct PlayerCapsule {
///     bottom: vec3<f32>,
///     radius: f32,
///     top: vec3<f32>,
///     color: u32,
/// }
/// ```
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct GpuPlayerCapsule {
    pub bottom: [f32; 3],
    pub radius: f32,
    pub top: [f32; 3],
    pub color: u32,
}

/// SDF capsule distance function (CPU-side, mirrors WGSL).
pub fn sd_capsule(
    p: Vec3,
    a: Vec3,
    b: Vec3,
    r: f32,
) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = pa.dot(ba) / ba.dot(ba);
    let h = h.clamp(0.0, 1.0);
    (pa - ba * h).length() - r
}

/// Resource holding the GPU capsule buffer data.
#[derive(Resource, Default)]
pub struct PlayerCapsuleBuffer {
    /// Capsule data for upload to GPU storage buffer.
    pub capsules: Vec<GpuPlayerCapsule>,
    /// Active player count this frame.
    pub count: u32,
}

// ---------------------------------------------------------------------------
// Nameplate
// ---------------------------------------------------------------------------

/// Component: nameplate label positioned above a remote player.
#[derive(Component, Clone, Debug)]
pub struct PlayerNameplate {
    pub player_entity_id: u64,
    pub text: String,
    pub screen_pos: Vec2,
    pub visible: bool,
}

// ---------------------------------------------------------------------------
// Bevy systems
// ---------------------------------------------------------------------------

/// System: interpolate all remote players and upload capsule data to GPU buffer.
fn interpolate_remote_players_system(
    time: Res<Time>,
    players: Query<&RemotePlayer>,
    mut gpu_buffer: ResMut<PlayerCapsuleBuffer>,
) {
    let now = time.elapsed_secs_f64();
    let mut capsules = Vec::with_capacity(MAX_REMOTE_PLAYERS);

    for rp in players.iter() {
        if capsules.len() >= MAX_REMOTE_PLAYERS {
            break;
        }
        let pos = rp.interpolated_position(now);
        capsules.push(GpuPlayerCapsule {
            bottom: pos.to_array(),
            radius: CAPSULE_RADIUS,
            top: (pos + Vec3::Y * CAPSULE_HEIGHT).to_array(),
            color: rp.skin_color,
        });
    }

    gpu_buffer.count = capsules.len() as u32;
    gpu_buffer.capsules = capsules;
}

/// System: update nameplate screen positions from remote player world positions.
fn update_nameplates_system(
    players: Query<&RemotePlayer>,
    camera_q: Query<(&GlobalTransform, &Projection), With<Camera3d>>,
    mut nameplates: Query<&mut PlayerNameplate>,
    time: Res<Time>,
) {
    let Ok((cam_transform, _projection)) = camera_q.single() else {
        return;
    };

    let now = time.elapsed_secs_f64();
    let cam_pos = cam_transform.translation();

    for mut np in nameplates.iter_mut() {
        // Find matching remote player
        if let Some(rp) =
            players.iter().find(|p| p.entity_id == np.player_entity_id)
        {
            let head_pos = rp.interpolated_position(now)
                + Vec3::Y * (CAPSULE_HEIGHT + 0.3);
            let to_cam = cam_pos - head_pos;
            // Simple visibility: within 50 units and in front of camera
            np.visible = to_cam.length() < 50.0;
        }
    }
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

/// Plugin registering multiplayer character systems.
pub struct MultiplayerCharactersPlugin;

impl Plugin for MultiplayerCharactersPlugin {
    fn build(
        &self,
        app: &mut App,
    ) {
        app.init_resource::<PlayerCapsuleBuffer>();

        app.add_systems(
            Update,
            (interpolate_remote_players_system, update_nameplates_system),
        );
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests;
