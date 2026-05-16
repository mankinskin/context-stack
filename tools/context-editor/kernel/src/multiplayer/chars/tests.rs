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
    let b = std::f32::consts::PI * 1.5;
    let result = lerp_angle(a, b, 1.0);
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
        Vec3::new(0.3, 0.9, 0.0),
        Vec3::ZERO,
        Vec3::new(0.0, 1.8, 0.0),
        0.3,
    );
    assert!(d.abs() < 0.05, "point on capsule surface should have distance ~0, got {d}");
}

#[test]
fn sd_capsule_inside() {
    let d = sd_capsule(
        Vec3::new(0.0, 0.9, 0.0),
        Vec3::ZERO,
        Vec3::new(0.0, 1.8, 0.0),
        0.3,
    );
    assert!(d < 0.0, "point inside capsule should have negative distance");
}

#[test]
fn sd_capsule_outside() {
    let d = sd_capsule(
        Vec3::new(5.0, 0.0, 0.0),
        Vec3::ZERO,
        Vec3::new(0.0, 1.8, 0.0),
        0.3,
    );
    assert!(d > 4.0, "point far from capsule should have large positive distance");
}

#[test]
fn gpu_capsule_size() {
    assert_eq!(
        std::mem::size_of::<GpuPlayerCapsule>(),
        32,
        "GpuPlayerCapsule must be 32 bytes (8 x f32)"
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
