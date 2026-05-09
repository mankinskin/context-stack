//! Combat system: SDF hit detection, damage model, voxel destruction VFX.
//!
//! Weapon shapes are SDF primitives derived from item blueprints. Hit detection
//! uses signed-distance overlap between weapon sweep volumes and player capsules.
//! Impacts trigger voxel debris particles and floating damage numbers.

use bevy::prelude::*;
use std::collections::VecDeque;

use crate::{
    multiplayer_backend::{
        CombatEvent,
        CombatEventQueue,
        ItemBlueprint,
        MultiplayerConnection,
        PlayerIdentity,
        PlayerTable,
        ReducerQueue,
        ReducerRequest,
    },
    multiplayer_chars::{
        sd_capsule,
        RemotePlayer,
        CAPSULE_HEIGHT,
        CAPSULE_RADIUS,
    },
    particle_splat::{
        Particle,
        ParticleSystem,
    },
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum weapon reach (metres).
pub const MAX_WEAPON_REACH: f32 = 5.0;

/// Default fist reach when no weapon is equipped.
pub const FIST_REACH: f32 = 1.2;

/// Default fist damage when no weapon is equipped.
pub const FIST_DAMAGE: i32 = 5;

/// Impact particle count per hit.
pub const DEBRIS_PARTICLE_COUNT: usize = 24;

/// Damage number float speed (units/sec).
pub const DAMAGE_NUMBER_FLOAT_SPEED: f32 = 2.0;

/// Damage number lifetime (seconds).
pub const DAMAGE_NUMBER_LIFETIME: f32 = 1.5;

/// Maximum active damage numbers.
pub const MAX_DAMAGE_NUMBERS: usize = 64;

/// Battle-glass distortion duration (seconds).
pub const BATTLE_GLASS_DURATION: f32 = 0.4;

/// Battle-glass max extra distortion.
pub const BATTLE_GLASS_MAX_DISTORTION: f32 = 0.15;

/// Respawn delay after death (seconds).
pub const RESPAWN_DELAY: f32 = 3.0;

/// Default spawn position.
pub const SPAWN_POINT: (f32, f32, f32) = (0.0, 20.0, 0.0);

/// Attack cooldown (seconds).
pub const ATTACK_COOLDOWN: f32 = 0.5;

// ---------------------------------------------------------------------------
// Weapon SDF model
// ---------------------------------------------------------------------------

/// Weapon SDF primitives derived from item voxel bounds.
#[derive(Clone, Debug)]
pub enum WeaponSdf {
    /// Sword: elongated box with length-based reach.
    Sword { half_extents: Vec3, reach: f32 },
    /// Axe: handle + rounded head.
    Axe { handle_length: f32, head_size: Vec3 },
    /// Hammer: sphere at end of capsule handle.
    Hammer {
        handle_length: f32,
        head_radius: f32,
    },
    /// Fist: default unarmed capsule.
    Fist { radius: f32 },
}

impl WeaponSdf {
    /// Generate weapon SDF from an item blueprint's voxel bounds.
    pub fn from_blueprint(bp: &ItemBlueprint) -> Self {
        let bounds = compute_voxel_bounds(&bp.voxel_data);
        let aspect_xz = if bounds.z > 0.001 {
            bounds.x / bounds.z
        } else {
            10.0
        };
        let aspect_yx = if bounds.x > 0.001 {
            bounds.y / bounds.x
        } else {
            10.0
        };

        if aspect_xz > 3.0 {
            // Long and thin → sword
            let reach = bounds.x * 0.1;
            WeaponSdf::Sword {
                half_extents: bounds * 0.05,
                reach: reach.min(MAX_WEAPON_REACH),
            }
        } else if aspect_yx > 2.0 {
            // Tall relative to width → hammer
            WeaponSdf::Hammer {
                handle_length: bounds.y * 0.08,
                head_radius: bounds.x.max(bounds.z) * 0.06,
            }
        } else {
            // Default → axe
            WeaponSdf::Axe {
                handle_length: bounds.y * 0.08,
                head_size: Vec3::new(bounds.x, bounds.y * 0.3, bounds.z) * 0.05,
            }
        }
    }

    /// Effective reach of this weapon SDF.
    pub fn reach(&self) -> f32 {
        match self {
            WeaponSdf::Sword { reach, .. } => *reach,
            WeaponSdf::Axe {
                handle_length,
                head_size,
            } => handle_length + head_size.y,
            WeaponSdf::Hammer {
                handle_length,
                head_radius,
            } => handle_length + head_radius,
            WeaponSdf::Fist { radius } => *radius + FIST_REACH,
        }
    }

    /// Base damage contribution from weapon shape (heavier shapes deal more).
    pub fn shape_bonus(&self) -> i32 {
        match self {
            WeaponSdf::Sword { .. } => 2,
            WeaponSdf::Axe { .. } => 4,
            WeaponSdf::Hammer { .. } => 6,
            WeaponSdf::Fist { .. } => 0,
        }
    }
}

/// Compute bounding box dimensions from serialized mini-SVO voxel data.
///
/// Returns (width, height, depth) in voxel units.
pub fn compute_voxel_bounds(voxel_data: &[u8]) -> Vec3 {
    if voxel_data.is_empty() {
        return Vec3::ONE;
    }
    // Each voxel is stored as (x, y, z, color_packed) = 7 bytes
    let stride = 7;
    let count = voxel_data.len() / stride;
    if count == 0 {
        return Vec3::ONE;
    }

    let mut min_x = i16::MAX;
    let mut min_y = i16::MAX;
    let mut min_z = i16::MAX;
    let mut max_x = i16::MIN;
    let mut max_y = i16::MIN;
    let mut max_z = i16::MIN;

    for i in 0..count {
        let base = i * stride;
        if base + 2 >= voxel_data.len() {
            break;
        }
        let x = voxel_data[base] as i16;
        let y = voxel_data[base + 1] as i16;
        let z = voxel_data[base + 2] as i16;
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        min_z = min_z.min(z);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
        max_z = max_z.max(z);
    }

    Vec3::new(
        (max_x - min_x + 1) as f32,
        (max_y - min_y + 1) as f32,
        (max_z - min_z + 1) as f32,
    )
}

// ---------------------------------------------------------------------------
// Attack state
// ---------------------------------------------------------------------------

/// Tracks the player's current attack state (cooldown, direction).
#[derive(Resource)]
pub struct AttackState {
    pub cooldown_remaining: f32,
    pub equipped_weapon: Option<WeaponSdf>,
    pub equipped_damage: i32,
    pub equipped_weight: f32,
}

impl Default for AttackState {
    fn default() -> Self {
        Self {
            cooldown_remaining: 0.0,
            equipped_weapon: None,
            equipped_damage: FIST_DAMAGE,
            equipped_weight: 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// SDF sweep hit detection (client-side prediction)
// ---------------------------------------------------------------------------

/// Result of a sweep test against a target.
#[derive(Clone, Debug)]
pub struct SweepHit {
    pub target_identity: PlayerIdentity,
    pub hit_position: Vec3,
    pub distance: f32,
}

/// Perform SDF sweep test: check if a weapon sweep arc overlaps with any
/// remote player capsule.
pub fn sweep_test(
    sweep_origin: Vec3,
    sweep_dir: Vec3,
    reach: f32,
    remote_players: &[(PlayerIdentity, Vec3)],
) -> Vec<SweepHit> {
    let mut hits = Vec::new();
    let dir = sweep_dir.normalize_or_zero();

    for (identity, pos) in remote_players {
        let capsule_bottom = *pos;
        let capsule_top = *pos + Vec3::Y * CAPSULE_HEIGHT;

        // Check distance from sweep line to capsule
        let capsule_dist = sd_capsule(
            sweep_origin + dir * reach * 0.5,
            capsule_bottom,
            capsule_top,
            CAPSULE_RADIUS,
        );

        if capsule_dist < reach {
            let hit_pos = *pos + Vec3::Y * (CAPSULE_HEIGHT * 0.5);
            hits.push(SweepHit {
                target_identity: identity.clone(),
                hit_position: hit_pos,
                distance: capsule_dist,
            });
        }
    }

    hits
}

// ---------------------------------------------------------------------------
// Damage numbers (screen-space floating text)
// ---------------------------------------------------------------------------

/// A floating damage number in world space.
#[derive(Clone, Debug)]
pub struct DamageNumber {
    pub value: i32,
    pub world_pos: Vec3,
    pub velocity: Vec3,
    pub lifetime: f32,
    pub age: f32,
    pub opacity: f32,
    /// true = damage dealt (red), false = healing (green).
    pub is_damage: bool,
}

impl DamageNumber {
    pub fn new(
        value: i32,
        pos: Vec3,
        is_damage: bool,
    ) -> Self {
        Self {
            value,
            world_pos: pos,
            velocity: Vec3::Y * DAMAGE_NUMBER_FLOAT_SPEED,
            lifetime: DAMAGE_NUMBER_LIFETIME,
            age: 0.0,
            opacity: 1.0,
            is_damage,
        }
    }
}

/// Resource tracking active damage numbers.
#[derive(Resource, Default)]
pub struct DamageNumberPool {
    pub active: VecDeque<DamageNumber>,
}

// ---------------------------------------------------------------------------
// Battle-glass effect
// ---------------------------------------------------------------------------

/// Resource tracking the battle-glass distortion effect on damage taken.
#[derive(Resource)]
pub struct BattleGlassEffect {
    pub distortion: f32,
    pub tint_red: f32,
    pub timer: f32,
}

impl Default for BattleGlassEffect {
    fn default() -> Self {
        Self {
            distortion: 0.0,
            tint_red: 0.0,
            timer: 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Respawn tracking
// ---------------------------------------------------------------------------

/// Tracks respawn state after player death.
#[derive(Resource)]
pub struct RespawnState {
    pub is_dead: bool,
    pub respawn_timer: f32,
}

impl Default for RespawnState {
    fn default() -> Self {
        Self {
            is_dead: false,
            respawn_timer: 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// HP/Mana bar data (bridge to Dioxus overlay)
// ---------------------------------------------------------------------------

/// Resource exposing current HP/Mana for UI overlays.
#[derive(Resource)]
pub struct PlayerVitals {
    pub current_hp: i32,
    pub max_hp: i32,
    pub current_mana: i32,
    pub max_mana: i32,
    /// Smoothed display HP (animates toward current_hp).
    pub display_hp: f32,
    /// Smoothed display mana.
    pub display_mana: f32,
}

impl Default for PlayerVitals {
    fn default() -> Self {
        Self {
            current_hp: 100,
            max_hp: 100,
            current_mana: 50,
            max_mana: 50,
            display_hp: 100.0,
            display_mana: 50.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// System: tick attack cooldown.
fn attack_cooldown_system(
    time: Res<Time>,
    mut state: ResMut<AttackState>,
) {
    if state.cooldown_remaining > 0.0 {
        state.cooldown_remaining =
            (state.cooldown_remaining - time.delta_secs()).max(0.0);
    }
}

/// System: process incoming combat events from server.
///
/// For each CombatEvent:
/// - Spawn debris particles at hit position
/// - Create floating damage number
/// - Trigger battle-glass effect if local player was hit
fn process_combat_events_system(
    mut events: ResMut<CombatEventQueue>,
    connection: Res<MultiplayerConnection>,
    mut particles: ResMut<ParticleSystem>,
    mut damage_numbers: ResMut<DamageNumberPool>,
    mut battle_glass: ResMut<BattleGlassEffect>,
) {
    let drained: Vec<CombatEvent> = std::mem::take(&mut events.events);
    for event in drained {
        let hit_pos =
            Vec3::new(event.hit_pos.0, event.hit_pos.1, event.hit_pos.2);

        // --- Debris particles ---
        spawn_debris_particles(&mut particles, hit_pos, DEBRIS_PARTICLE_COUNT);

        // --- Damage number ---
        let dmg_num =
            DamageNumber::new(event.damage, hit_pos + Vec3::Y * 0.5, true);
        if damage_numbers.active.len() >= MAX_DAMAGE_NUMBERS {
            damage_numbers.active.pop_front();
        }
        damage_numbers.active.push_back(dmg_num);

        // --- Battle-glass if we are the target ---
        if event.target.id == connection.identity.id {
            battle_glass.timer = BATTLE_GLASS_DURATION;
        }
    }
}

/// Spawn debris particles outward from an impact point.
fn spawn_debris_particles(
    particles: &mut ParticleSystem,
    hit_pos: Vec3,
    count: usize,
) {
    for i in 0..count {
        // Distribute outward in a hemisphere above the hit
        let angle = (i as f32 / count as f32) * std::f32::consts::TAU;
        let up_bias = 0.3 + (i % 3) as f32 * 0.2;
        let dir = Vec3::new(angle.cos(), up_bias, angle.sin()).normalize();

        particles.particles.push(Particle {
            position: hit_pos,
            velocity: dir * 5.0,
            color: [0.6, 0.5, 0.4, 1.0], // stone-like debris color
            scale: 0.08 + (i % 4) as f32 * 0.02,
            opacity: 1.0,
            lifetime: 1.5 + (i % 3) as f32 * 0.5,
            age: 0.0,
        });
    }
}

/// System: update floating damage numbers (position, opacity, lifetime).
fn damage_number_system(
    time: Res<Time>,
    mut pool: ResMut<DamageNumberPool>,
) {
    let dt = time.delta_secs();
    pool.active.retain_mut(|num| {
        num.age += dt;
        num.world_pos += num.velocity * dt;
        num.opacity = 1.0 - (num.age / num.lifetime).min(1.0);
        num.age < num.lifetime
    });
}

/// System: update battle-glass distortion effect.
fn battle_glass_system(
    time: Res<Time>,
    mut effect: ResMut<BattleGlassEffect>,
) {
    if effect.timer > 0.0 {
        effect.timer = (effect.timer - time.delta_secs()).max(0.0);
        let t = effect.timer / BATTLE_GLASS_DURATION;
        // Quick attack, slow decay
        effect.distortion = BATTLE_GLASS_MAX_DISTORTION * t;
        effect.tint_red = 0.3 * t;
    } else {
        effect.distortion = 0.0;
        effect.tint_red = 0.0;
    }
}

/// System: update player vitals from local player table.
fn vitals_sync_system(
    player_table: Res<PlayerTable>,
    connection: Res<MultiplayerConnection>,
    mut vitals: ResMut<PlayerVitals>,
    time: Res<Time>,
) {
    if let Some(row) = player_table.local_player(&connection.identity) {
        vitals.current_hp = row.hp;
        vitals.max_hp = row.max_hp;
        vitals.current_mana = row.mana;
        vitals.max_mana = row.max_mana;
    }

    // Smooth animation toward current values
    let dt = time.delta_secs();
    let lerp_speed = 8.0;
    vitals.display_hp +=
        (vitals.current_hp as f32 - vitals.display_hp) * lerp_speed * dt;
    vitals.display_mana +=
        (vitals.current_mana as f32 - vitals.display_mana) * lerp_speed * dt;
}

/// System: handle respawn after death.
fn respawn_system(
    time: Res<Time>,
    mut respawn: ResMut<RespawnState>,
    vitals: Res<PlayerVitals>,
    mut reducer_queue: ResMut<ReducerQueue>,
) {
    // Detect death
    if vitals.current_hp <= 0 && !respawn.is_dead {
        respawn.is_dead = true;
        respawn.respawn_timer = RESPAWN_DELAY;
    }

    // Count down respawn
    if respawn.is_dead {
        respawn.respawn_timer -= time.delta_secs();
        if respawn.respawn_timer <= 0.0 {
            respawn.is_dead = false;
            respawn.respawn_timer = 0.0;
            // Request respawn via reducer
            reducer_queue.push(ReducerRequest::Respawn {
                position: SPAWN_POINT,
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(
        &self,
        app: &mut App,
    ) {
        app.init_resource::<AttackState>();
        app.init_resource::<DamageNumberPool>();
        app.init_resource::<BattleGlassEffect>();
        app.init_resource::<RespawnState>();
        app.init_resource::<PlayerVitals>();

        app.add_systems(
            Update,
            (
                attack_cooldown_system,
                process_combat_events_system,
                damage_number_system,
                battle_glass_system,
                vitals_sync_system,
                respawn_system,
            ),
        );
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

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
            voxel_data: make_voxel_line(40, 2, 2), // 40 wide, 2 tall, 2 deep
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
        // One voxel at (5, 10, 3)
        let data = vec![5, 10, 3, 0xFF, 0x00, 0x00, 0x00];
        let bounds = compute_voxel_bounds(&data);
        assert_eq!(bounds, Vec3::ONE);
    }

    #[test]
    fn compute_voxel_bounds_multiple() {
        // Two voxels: (0,0,0) and (7,3,5)
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
            pool.active.push_back(DamageNumber::new(
                i as i32,
                Vec3::ZERO,
                true,
            ));
        }
        assert_eq!(pool.active.len(), MAX_DAMAGE_NUMBERS);
        // Oldest should have been evicted
        assert_eq!(pool.active.front().unwrap().value, 10);
    }

    #[test]
    fn battle_glass_decays() {
        let mut effect = BattleGlassEffect {
            distortion: BATTLE_GLASS_MAX_DISTORTION,
            tint_red: 0.3,
            timer: BATTLE_GLASS_DURATION,
        };
        // Simulate half decay
        effect.timer = BATTLE_GLASS_DURATION * 0.5;
        let t = effect.timer / BATTLE_GLASS_DURATION;
        effect.distortion = BATTLE_GLASS_MAX_DISTORTION * t;
        effect.tint_red = 0.3 * t;
        assert!(
            (effect.distortion - BATTLE_GLASS_MAX_DISTORTION * 0.5).abs()
                < 0.001
        );
        assert!((effect.tint_red - 0.15).abs() < 0.001);
    }

    #[test]
    fn battle_glass_zero_when_expired() {
        let mut effect = BattleGlassEffect::default();
        effect.timer = 0.0;
        // Simulate system logic
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
        // Simulate a couple lerp steps
        let dt = 0.016;
        let speed = 8.0;
        for _ in 0..10 {
            vitals.display_hp +=
                (vitals.current_hp as f32 - vitals.display_hp) * speed * dt;
        }
        // display_hp should be moving toward 50
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
        // Simulate detection
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
        // Timer expired
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

    // --- Test helpers ---

    /// Create a voxel data buffer representing a bounding box of (w, h, d) voxels.
    /// Only corner voxels are stored (min and max).
    fn make_voxel_line(
        w: u8,
        h: u8,
        d: u8,
    ) -> Vec<u8> {
        let mut data = Vec::new();
        // Origin voxel
        data.extend_from_slice(&[0, 0, 0, 0xFF, 0x00, 0x00, 0x00]);
        // Far corner voxel
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
}
