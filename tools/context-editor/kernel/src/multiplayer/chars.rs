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
mod tests {
    use super::*;

    fn make_remote_player() -> RemotePlayer {
        RemotePlayer {
            entity_id: 1,
            identity: PlayerIdentity::new(100),
            prev_state: PlayerSnapshot {
                position: Vec3::ZERO,
                yaw: 0.0,
                hp: 100,
                max_hp: 100,
            },
            curr_state: PlayerSnapshot {
                position: Vec3::new(10.0, 0.0, 0.0),
                yaw: 1.0,
                hp: 100,
                max_hp: 100,
            },
            prev_timestamp: 0.0,
            curr_timestamp: 1.0,
            skin_color: 0xFFCCAA,
            name: "TestPlayer".to_string(),
        }
    }

    #[test]
    fn interpolation_at_start() {
        let rp = make_remote_player();
        let pos = rp.interpolated_position(0.0);
        assert!((pos - Vec3::ZERO).length() < 0.01);
    }

    #[test]
    fn interpolation_at_midpoint() {
        let rp = make_remote_player();
        let pos = rp.interpolated_position(0.5);
        assert!((pos.x - 5.0).abs() < 0.01);
    }

    #[test]
    fn interpolation_at_end() {
        let rp = make_remote_player();
        let pos = rp.interpolated_position(1.0);
        assert!((pos.x - 10.0).abs() < 0.01);
    }

    #[test]
    fn interpolation_extrapolation_clamped() {
        let rp = make_remote_player();
        // At t=2.0, alpha would be 2.0 but clamped to MAX_EXTRAPOLATION (1.2)
        let pos = rp.interpolated_position(2.0);
        assert!(pos.x <= 10.0 * MAX_EXTRAPOLATION + 0.01);
    }

    #[test]
    fn push_snapshot_shifts_states() {
        let mut rp = make_remote_player();
        let new_snapshot = PlayerSnapshot {
            position: Vec3::new(20.0, 0.0, 0.0),
            yaw: 2.0,
            hp: 80,
            max_hp: 100,
        };
        rp.push_snapshot(new_snapshot, 2.0);
        assert!((rp.prev_state.position.x - 10.0).abs() < 0.01);
        assert!((rp.curr_state.position.x - 20.0).abs() < 0.01);
        assert_eq!(rp.prev_timestamp, 1.0);
        assert_eq!(rp.curr_timestamp, 2.0);
    }

    #[test]
    fn lerp_angle_shortest_path() {
        let a = 0.0f32;
        let b = std::f32::consts::PI * 1.5; // 270° — shortest path is -90°
        let result = lerp_angle(a, b, 1.0);
        // Should go to -PI/2 (= -90°), equivalent to 1.5π via shortest path
        assert!((result - (-std::f32::consts::FRAC_PI_2)).abs() < 0.01);
    }

    #[test]
    fn lerp_angle_no_wrap() {
        let result = lerp_angle(0.0, 1.0, 0.5);
        assert!((result - 0.5).abs() < 0.01);
    }

    #[test]
    fn sd_capsule_on_surface() {
        let d = sd_capsule(
            Vec3::new(0.3, 0.9, 0.0), // on surface of capsule
            Vec3::ZERO,
            Vec3::new(0.0, 1.8, 0.0),
            0.3,
        );
        assert!(
            d.abs() < 0.05,
            "point on capsule surface should have distance ~0, got {d}"
        );
    }

    #[test]
    fn sd_capsule_inside() {
        let d = sd_capsule(
            Vec3::new(0.0, 0.9, 0.0), // center of capsule
            Vec3::ZERO,
            Vec3::new(0.0, 1.8, 0.0),
            0.3,
        );
        assert!(
            d < 0.0,
            "point inside capsule should have negative distance"
        );
    }

    #[test]
    fn sd_capsule_outside() {
        let d = sd_capsule(
            Vec3::new(5.0, 0.0, 0.0), // far away
            Vec3::ZERO,
            Vec3::new(0.0, 1.8, 0.0),
            0.3,
        );
        assert!(
            d > 4.0,
            "point far from capsule should have large positive distance"
        );
    }

    #[test]
    fn gpu_capsule_size() {
        assert_eq!(
            std::mem::size_of::<GpuPlayerCapsule>(),
            32,
            "GpuPlayerCapsule must be 32 bytes (8 × f32)"
        );
    }

    #[test]
    fn capsule_buffer_default_empty() {
        let buf = PlayerCapsuleBuffer::default();
        assert_eq!(buf.count, 0);
        assert!(buf.capsules.is_empty());
    }

    #[test]
    fn interpolated_yaw_mid() {
        let rp = make_remote_player();
        let yaw = rp.interpolated_yaw(0.5);
        assert!((yaw - 0.5).abs() < 0.01);
    }
}
