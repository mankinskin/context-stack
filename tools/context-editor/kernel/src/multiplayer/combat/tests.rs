use super::*;

use crate::multiplayer_backend::ItemBlueprint;
use crate::multiplayer_backend::PlayerIdentity;

#[test]
fn weapon_sdf_fist_defaults() {
    let fist = WeaponSdf::Fist { radius: 0.15 };
    assert!(fist.reach() > FIST_REACH);
    assert_eq!(fist.shape_bonus(), 0);
}

#[test]
fn weapon_sdf_sword_from_elongated_bounds() {
    let bp = ItemBlueprint {
        blueprint_id: 1,
        name: "Long Sword".into(),
        voxel_data: make_voxel_line(40, 2, 2),
        base_damage: 10,
        weight: 3.0,
        item_type: crate::multiplayer_backend::ItemType::Weapon,
    };
    let sdf = WeaponSdf::from_blueprint(&bp);
    assert!(matches!(sdf, WeaponSdf::Sword { .. }));
    assert!(sdf.reach() > 0.0);
}

#[test]
fn weapon_sdf_hammer_from_tall_bounds() {
    let bp = ItemBlueprint {
        blueprint_id: 2,
        name: "War Hammer".into(),
        voxel_data: make_voxel_line(3, 30, 3),
        base_damage: 15,
        weight: 5.0,
        item_type: crate::multiplayer_backend::ItemType::Weapon,
    };
    let sdf = WeaponSdf::from_blueprint(&bp);
    assert!(matches!(sdf, WeaponSdf::Hammer { .. }));
    assert_eq!(sdf.shape_bonus(), 6);
}

#[test]
fn weapon_sdf_axe_from_cubic_bounds() {
    let bp = ItemBlueprint {
        blueprint_id: 3,
        name: "Battle Axe".into(),
        voxel_data: make_voxel_line(4, 5, 4),
        base_damage: 12,
        weight: 4.0,
        item_type: crate::multiplayer_backend::ItemType::Weapon,
    };
    let sdf = WeaponSdf::from_blueprint(&bp);
    assert!(matches!(sdf, WeaponSdf::Axe { .. }));
    assert_eq!(sdf.shape_bonus(), 4);
}

#[test]
fn compute_voxel_bounds_empty() {
    let bounds = compute_voxel_bounds(&[]);
    assert_eq!(bounds, Vec3::ONE);
}

#[test]
fn compute_voxel_bounds_single_voxel() {
    let data = vec![5, 10, 3, 0xFF, 0x00, 0x00, 0x00];
    let bounds = compute_voxel_bounds(&data);
    assert_eq!(bounds, Vec3::ONE);
}

#[test]
fn compute_voxel_bounds_multiple() {
    let mut data = vec![0, 0, 0, 0xFF, 0x00, 0x00, 0x00];
    data.extend_from_slice(&[7, 3, 5, 0xFF, 0x00, 0x00, 0x00]);
    let bounds = compute_voxel_bounds(&data);
    assert_eq!(bounds, Vec3::new(8.0, 4.0, 6.0));
}

#[test]
fn sweep_test_no_targets() {
    let hits = sweep_test(Vec3::ZERO, Vec3::X, 2.0, &[]);
    assert!(hits.is_empty());
}

#[test]
fn sweep_test_hit_within_range() {
    let target = (PlayerIdentity::new(42), Vec3::new(1.5, 0.0, 0.0));
    let hits = sweep_test(Vec3::ZERO, Vec3::X, 3.0, &[target]);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].target_identity.id, 42);
}

#[test]
fn sweep_test_miss_out_of_range() {
    let target = (PlayerIdentity::new(42), Vec3::new(100.0, 0.0, 0.0));
    let hits = sweep_test(Vec3::ZERO, Vec3::X, 2.0, &[target]);
    assert!(hits.is_empty());
}

#[test]
fn damage_number_creation() {
    let dn = DamageNumber::new(25, Vec3::new(1.0, 2.0, 3.0), true);
    assert_eq!(dn.value, 25);
    assert_eq!(dn.opacity, 1.0);
    assert!(dn.is_damage);
}

#[test]
fn damage_number_pool_eviction() {
    let mut pool = DamageNumberPool::default();
    for i in 0..(MAX_DAMAGE_NUMBERS + 10) {
        if pool.active.len() >= MAX_DAMAGE_NUMBERS {
            pool.active.pop_front();
        }
        pool.active.push_back(DamageNumber::new(i as i32, Vec3::ZERO, true));
    }
    assert_eq!(pool.active.len(), MAX_DAMAGE_NUMBERS);
    assert_eq!(pool.active.front().unwrap().value, 10);
}

#[test]
fn battle_glass_decays() {
    let mut effect = BattleGlassEffect {
        distortion: BATTLE_GLASS_MAX_DISTORTION,
        tint_red: 0.3,
        timer: BATTLE_GLASS_DURATION,
    };
    effect.timer = BATTLE_GLASS_DURATION * 0.5;
    let t = effect.timer / BATTLE_GLASS_DURATION;
    effect.distortion = BATTLE_GLASS_MAX_DISTORTION * t;
    effect.tint_red = 0.3 * t;
    assert!((effect.distortion - BATTLE_GLASS_MAX_DISTORTION * 0.5).abs() < 0.001);
    assert!((effect.tint_red - 0.15).abs() < 0.001);
}

#[test]
fn battle_glass_zero_when_expired() {
    let mut effect = BattleGlassEffect::default();
    effect.timer = 0.0;
    if effect.timer <= 0.0 {
        effect.distortion = 0.0;
        effect.tint_red = 0.0;
    }
    assert_eq!(effect.distortion, 0.0);
    assert_eq!(effect.tint_red, 0.0);
}

#[test]
fn player_vitals_smooth_animation() {
    let mut vitals = PlayerVitals::default();
    vitals.current_hp = 50;
    let dt = 0.016;
    let speed = 8.0;
    for _ in 0..10 {
        vitals.display_hp += (vitals.current_hp as f32 - vitals.display_hp) * speed * dt;
    }
    assert!(vitals.display_hp < 100.0);
    assert!(vitals.display_hp > 50.0);
}

#[test]
fn respawn_state_triggers_on_zero_hp() {
    let mut respawn = RespawnState::default();
    let vitals = PlayerVitals {
        current_hp: 0,
        ..Default::default()
    };
    if vitals.current_hp <= 0 && !respawn.is_dead {
        respawn.is_dead = true;
        respawn.respawn_timer = RESPAWN_DELAY;
    }
    assert!(respawn.is_dead);
    assert_eq!(respawn.respawn_timer, RESPAWN_DELAY);
}

#[test]
fn respawn_state_completes() {
    let mut respawn = RespawnState {
        is_dead: true,
        respawn_timer: 0.0,
    };
    if respawn.is_dead && respawn.respawn_timer <= 0.0 {
        respawn.is_dead = false;
    }
    assert!(!respawn.is_dead);
}

#[test]
fn attack_cooldown_decrements() {
    let mut state = AttackState {
        cooldown_remaining: 0.3,
        ..Default::default()
    };
    let dt = 0.1;
    state.cooldown_remaining = (state.cooldown_remaining - dt).max(0.0);
    assert!((state.cooldown_remaining - 0.2).abs() < 0.001);
}

#[test]
fn attack_cooldown_floors_at_zero() {
    let mut state = AttackState {
        cooldown_remaining: 0.05,
        ..Default::default()
    };
    state.cooldown_remaining = (state.cooldown_remaining - 0.1).max(0.0);
    assert_eq!(state.cooldown_remaining, 0.0);
}

fn make_voxel_line(
    w: u8,
    h: u8,
    d: u8,
) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&[0, 0, 0, 0xFF, 0x00, 0x00, 0x00]);
    data.extend_from_slice(&[
        w.saturating_sub(1),
        h.saturating_sub(1),
        d.saturating_sub(1),
        0xFF,
        0x00,
        0x00,
        0x00,
    ]);
    data
}
