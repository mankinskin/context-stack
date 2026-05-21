//! Skill system: spell SDFs, procedural shader effects & volumetric magic.
//!
//! Spells are transient SDF volumes evaluated in the ray-marching loop. Each
//! spell category manipulates rendering differently: fireballs emit light,
//! shields refract rays, frost modifies roughness, gravity warps ray direction.

use bevy::prelude::*;
use bytemuck::{
    Pod,
    Zeroable,
};

mod plugin;
mod sdf;

pub use plugin::SkillPlugin;
pub use sdf::SpellSdf;

use crate::multiplayer_backend::PlayerIdentity;

#[cfg(test)]
pub(crate) use sdf::sd_capsule_line;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum simultaneous active spells on the GPU.
pub const MAX_ACTIVE_SPELLS: usize = 64;

/// Mana regeneration rate (points per second).
pub const MANA_REGEN_RATE: f32 = 2.0;

/// Default spell lifetime (seconds) if not overridden.
pub const DEFAULT_SPELL_LIFETIME: f32 = 5.0;

/// Fireball travel speed (units/sec).
pub const FIREBALL_SPEED: f32 = 15.0;

/// Lightning bolt length (units).
pub const LIGHTNING_LENGTH: f32 = 20.0;

/// Shield duration (seconds).
pub const SHIELD_DURATION: f32 = 8.0;

/// Frost radius expansion rate (units/sec).
pub const FROST_EXPAND_RATE: f32 = 3.0;

/// Gravity well duration (seconds).
pub const GRAVITY_DURATION: f32 = 4.0;

/// Minimum cast interval per spell type (seconds).
pub const GLOBAL_COOLDOWN: f32 = 0.3;

// ---------------------------------------------------------------------------
// Spell type enum
// ---------------------------------------------------------------------------

/// Spell categories, each with unique SDF and shader behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SpellType {
    Fireball = 0,
    Shield = 1,
    Frost = 2,
    Gravity = 3,
    Lightning = 4,
}

impl SpellType {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Fireball),
            1 => Some(Self::Shield),
            2 => Some(Self::Frost),
            3 => Some(Self::Gravity),
            4 => Some(Self::Lightning),
            _ => None,
        }
    }

    /// Mana cost for this spell at the given power level.
    pub fn mana_cost(
        self,
        power: f32,
    ) -> i32 {
        let base = match self {
            SpellType::Fireball => 15,
            SpellType::Shield => 25,
            SpellType::Frost => 12,
            SpellType::Gravity => 30,
            SpellType::Lightning => 20,
        };
        (base as f32 * power.max(0.5)).ceil() as i32
    }

    /// Base radius for the spell SDF.
    pub fn base_radius(self) -> f32 {
        match self {
            SpellType::Fireball => 0.5,
            SpellType::Shield => 2.0,
            SpellType::Frost => 3.0,
            SpellType::Gravity => 4.0,
            SpellType::Lightning => 0.05,
        }
    }

    /// Default lifetime in seconds.
    pub fn lifetime(self) -> f32 {
        match self {
            SpellType::Fireball => 3.0,
            SpellType::Shield => SHIELD_DURATION,
            SpellType::Frost => 2.5,
            SpellType::Gravity => GRAVITY_DURATION,
            SpellType::Lightning => 0.3,
        }
    }

    /// Whether this spell type is a projectile (moves through space).
    pub fn is_projectile(self) -> bool {
        matches!(self, SpellType::Fireball | SpellType::Lightning)
    }

    /// Projectile speed for moving spells.
    pub fn speed(self) -> f32 {
        match self {
            SpellType::Fireball => FIREBALL_SPEED,
            SpellType::Lightning => LIGHTNING_LENGTH * 3.0, // instant-ish
            _ => 0.0,
        }
    }

    /// Emission color for GPU rendering.
    pub fn emission_color(self) -> [f32; 3] {
        match self {
            SpellType::Fireball => [1.0, 0.5, 0.1],
            SpellType::Shield => [0.3, 0.6, 1.0],
            SpellType::Frost => [0.7, 0.9, 1.0],
            SpellType::Gravity => [0.5, 0.0, 0.8],
            SpellType::Lightning => [0.6, 0.8, 1.0],
        }
    }
}

// ---------------------------------------------------------------------------
// Active spell entity
// ---------------------------------------------------------------------------

/// Active spell component on a Bevy entity.
#[derive(Component, Clone, Debug)]
pub struct ActiveSpell {
    pub spell_id: u64,
    pub caster: PlayerIdentity,
    pub spell_type: SpellType,
    pub position: Vec3,
    pub direction: Vec3,
    pub power: f32,
    pub radius: f32,
    pub elapsed: f32,
    pub lifetime: f32,
}

impl ActiveSpell {
    /// Create from server subscription data.
    pub fn from_server(
        spell_id: u64,
        caster: PlayerIdentity,
        spell_type: SpellType,
        position: Vec3,
        direction: Vec3,
        power: f32,
        lifetime: f32,
    ) -> Self {
        Self {
            spell_id,
            caster,
            spell_type,
            position,
            direction: direction.normalize_or_zero(),
            power,
            radius: spell_type.base_radius() * power,
            elapsed: 0.0,
            lifetime,
        }
    }

    /// Build the SDF variant for this spell.
    pub fn to_sdf(&self) -> SpellSdf {
        let color = self.spell_type.emission_color();
        match self.spell_type {
            SpellType::Fireball => SpellSdf::Fireball {
                center: self.position,
                radius: self.radius,
                noise_scale: 5.0,
                emission_color: Vec3::new(color[0], color[1], color[2]),
                emission_intensity: self.power * 3.0,
            },
            SpellType::Shield => SpellSdf::Shield {
                center: self.position,
                outer_radius: self.radius,
                thickness: 0.1,
                ior: 1.3,
                tint: Vec3::new(color[0], color[1], color[2]),
            },
            SpellType::Frost => SpellSdf::Frost {
                center: self.position,
                radius: self.radius,
                roughness_boost: self.power * 0.5,
            },
            SpellType::Gravity => SpellSdf::Gravity {
                center: self.position,
                radius: self.radius,
                strength: self.power * 2.0,
            },
            SpellType::Lightning => SpellSdf::Lightning {
                origin: self.position,
                direction: self.direction,
                length: LIGHTNING_LENGTH * self.power,
                radius: 0.05,
                branch_noise: 0.1,
            },
        }
    }

    /// Whether this spell has expired.
    pub fn is_expired(&self) -> bool {
        self.elapsed >= self.lifetime
    }
}

// ---------------------------------------------------------------------------
// GPU spell data (uploaded each frame)
// ---------------------------------------------------------------------------

/// Packed GPU representation of one spell for the compute/ray-march shader.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GpuSpellData {
    pub spell_type: u32,
    pub _pad0: u32,
    pub _pad1: u32,
    pub _pad2: u32,
    pub pos: [f32; 3],
    pub radius: f32,
    pub dir: [f32; 3],
    pub power: f32,
    pub color: [f32; 3],
    pub intensity: f32,
    pub time: f32,
    pub _pad3: [f32; 3],
}

/// Resource holding the GPU-side spell buffer data.
#[derive(Resource, Default)]
pub struct SpellBuffer {
    pub spells: Vec<GpuSpellData>,
}

impl SpellBuffer {
    /// Pack an active spell into GPU format.
    pub fn pack_spell(spell: &ActiveSpell) -> GpuSpellData {
        let color = spell.spell_type.emission_color();
        GpuSpellData {
            spell_type: spell.spell_type as u32,
            _pad0: 0,
            _pad1: 0,
            _pad2: 0,
            pos: spell.position.to_array(),
            radius: spell.radius,
            dir: spell.direction.to_array(),
            power: spell.power,
            color,
            intensity: spell.power * 3.0,
            time: spell.elapsed,
            _pad3: [0.0; 3],
        }
    }
}

// ---------------------------------------------------------------------------
// Cast cooldown tracking
// ---------------------------------------------------------------------------

/// Tracks per-spell-type cooldowns.
#[derive(Resource, Default)]
pub struct SpellCooldowns {
    pub global_remaining: f32,
    pub per_type: [f32; 5], // indexed by SpellType as u8
}

impl SpellCooldowns {
    pub fn can_cast(
        &self,
        spell_type: SpellType,
    ) -> bool {
        self.global_remaining <= 0.0
            && self.per_type[spell_type as usize] <= 0.0
    }

    pub fn trigger(
        &mut self,
        spell_type: SpellType,
    ) {
        self.global_remaining = GLOBAL_COOLDOWN;
        self.per_type[spell_type as usize] = spell_type.lifetime() * 0.5;
    }

    pub fn tick(
        &mut self,
        dt: f32,
    ) {
        self.global_remaining = (self.global_remaining - dt).max(0.0);
        for cd in &mut self.per_type {
            *cd = (*cd - dt).max(0.0);
        }
    }
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// System: advance active spells (position, lifetime, expiry).
fn advance_spells_system(
    time: Res<Time>,
    mut commands: Commands,
    mut query: Query<(Entity, &mut ActiveSpell)>,
) {
    let dt = time.delta_secs();
    for (entity, mut spell) in query.iter_mut() {
        spell.elapsed += dt;

        let stype = spell.spell_type;
        let dir = spell.direction;

        // Move projectiles
        if stype.is_projectile() {
            spell.position += dir * stype.speed() * dt;
        }

        // Expand frost radius
        if stype == SpellType::Frost {
            spell.radius += FROST_EXPAND_RATE * dt;
        }

        // Despawn expired
        if spell.is_expired() {
            commands.entity(entity).despawn();
        }
    }
}

/// System: update spell cooldowns.
fn cooldown_system(
    time: Res<Time>,
    mut cooldowns: ResMut<SpellCooldowns>,
) {
    cooldowns.tick(time.delta_secs());
}

/// System: pack active spells into GPU buffer.
fn spell_buffer_system(
    query: Query<&ActiveSpell>,
    mut buffer: ResMut<SpellBuffer>,
) {
    buffer.spells.clear();
    for spell in query.iter() {
        if buffer.spells.len() >= MAX_ACTIVE_SPELLS {
            break;
        }
        buffer.spells.push(SpellBuffer::pack_spell(spell));
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests;
