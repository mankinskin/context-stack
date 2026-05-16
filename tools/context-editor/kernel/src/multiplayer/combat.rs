//! Combat system: SDF hit detection, damage model, voxel destruction VFX.
//!
//! Weapon shapes are SDF primitives derived from item blueprints. Hit detection
//! uses signed-distance overlap between weapon sweep volumes and player capsules.
//! Impacts trigger voxel debris particles and floating damage numbers.

use bevy::prelude::*;
use std::collections::VecDeque;

mod model;
mod plugin;

pub use model::{
    compute_voxel_bounds,
    sweep_test,
    SweepHit,
    WeaponSdf,
};
pub use plugin::CombatPlugin;

use crate::{
    multiplayer_backend::{
        CombatEvent,
        CombatEventQueue,
        MultiplayerConnection,
        PlayerTable,
        ReducerQueue,
        ReducerRequest,
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

#[cfg(test)]
mod tests;
