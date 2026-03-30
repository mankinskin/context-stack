//! Skill system: spell SDFs, procedural shader effects & volumetric magic.
//!
//! Spells are transient SDF volumes evaluated in the ray-marching loop. Each
//! spell category manipulates rendering differently: fireballs emit light,
//! shields refract rays, frost modifies roughness, gravity warps ray direction.

use bevy::prelude::*;
use bytemuck::{Pod, Zeroable};
use std::collections::VecDeque;

use crate::multiplayer_backend::{
    MultiplayerConnection, PlayerIdentity, PlayerTable,
    ReducerQueue, ReducerRequest,
};

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
    pub fn mana_cost(self, power: f32) -> i32 {
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
// Spell SDF data
// ---------------------------------------------------------------------------

/// Spell SDF variants with typed parameters.
#[derive(Clone, Debug)]
pub enum SpellSdf {
    Fireball {
        center: Vec3,
        radius: f32,
        noise_scale: f32,
        emission_color: Vec3,
        emission_intensity: f32,
    },
    Shield {
        center: Vec3,
        outer_radius: f32,
        thickness: f32,
        ior: f32,
        tint: Vec3,
    },
    Frost {
        center: Vec3,
        radius: f32,
        roughness_boost: f32,
    },
    Gravity {
        center: Vec3,
        radius: f32,
        strength: f32,
    },
    Lightning {
        origin: Vec3,
        direction: Vec3,
        length: f32,
        radius: f32,
        branch_noise: f32,
    },
}

impl SpellSdf {
    /// Sample the signed distance at point `p`.
    pub fn evaluate(&self, p: Vec3) -> f32 {
        match self {
            SpellSdf::Fireball { center, radius, .. } => {
                (p - *center).length() - radius
            }
            SpellSdf::Shield { center, outer_radius, thickness, .. } => {
                let d_outer = (p - *center).length() - outer_radius;
                let d_inner = (p - *center).length() - (outer_radius - thickness);
                d_outer.max(-d_inner)
            }
            SpellSdf::Frost { center, radius, .. } => {
                (p - *center).length() - radius
            }
            SpellSdf::Gravity { center, radius, .. } => {
                (p - *center).length() - radius
            }
            SpellSdf::Lightning { origin, direction, length, radius, .. } => {
                // Capsule SDF along the beam direction
                let dir = direction.normalize_or_zero();
                let end = *origin + dir * *length;
                sd_capsule_line(p, *origin, end, *radius)
            }
        }
    }
}

/// Signed distance from point to a capsule (line segment + radius).
fn sd_capsule_line(p: Vec3, a: Vec3, b: Vec3, r: f32) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = (pa.dot(ba) / ba.dot(ba)).clamp(0.0, 1.0);
    (pa - ba * h).length() - r
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
    pub fn can_cast(&self, spell_type: SpellType) -> bool {
        self.global_remaining <= 0.0 && self.per_type[spell_type as usize] <= 0.0
    }

    pub fn trigger(&mut self, spell_type: SpellType) {
        self.global_remaining = GLOBAL_COOLDOWN;
        self.per_type[spell_type as usize] = spell_type.lifetime() * 0.5;
    }

    pub fn tick(&mut self, dt: f32) {
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
fn cooldown_system(time: Res<Time>, mut cooldowns: ResMut<SpellCooldowns>) {
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
// Plugin
// ---------------------------------------------------------------------------

pub struct SkillPlugin;

impl Plugin for SkillPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SpellCooldowns>();
        app.init_resource::<SpellBuffer>();
        app.add_systems(
            Update,
            (
                cooldown_system,
                advance_spells_system,
                spell_buffer_system,
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
    fn spell_type_from_u8_valid() {
        assert_eq!(SpellType::from_u8(0), Some(SpellType::Fireball));
        assert_eq!(SpellType::from_u8(4), Some(SpellType::Lightning));
    }

    #[test]
    fn spell_type_from_u8_invalid() {
        assert_eq!(SpellType::from_u8(5), None);
        assert_eq!(SpellType::from_u8(255), None);
    }

    #[test]
    fn mana_cost_scales_with_power() {
        let cost_low = SpellType::Fireball.mana_cost(0.5);
        let cost_high = SpellType::Fireball.mana_cost(2.0);
        assert!(cost_high > cost_low);
    }

    #[test]
    fn mana_cost_minimum_clamp() {
        let cost = SpellType::Fireball.mana_cost(0.1);
        // Power clamped to 0.5 minimum
        let expected = (15.0 * 0.5f32).ceil() as i32;
        assert_eq!(cost, expected);
    }

    #[test]
    fn base_radius_varies_by_type() {
        assert!(SpellType::Gravity.base_radius() > SpellType::Fireball.base_radius());
        assert!(SpellType::Shield.base_radius() > SpellType::Lightning.base_radius());
    }

    #[test]
    fn fireball_is_projectile() {
        assert!(SpellType::Fireball.is_projectile());
        assert!(!SpellType::Shield.is_projectile());
        assert!(!SpellType::Frost.is_projectile());
    }

    #[test]
    fn lightning_is_projectile() {
        assert!(SpellType::Lightning.is_projectile());
    }

    #[test]
    fn shield_not_projectile() {
        assert!(!SpellType::Shield.is_projectile());
        assert_eq!(SpellType::Shield.speed(), 0.0);
    }

    #[test]
    fn active_spell_expires() {
        let mut spell = ActiveSpell::from_server(
            1, PlayerIdentity::local(), SpellType::Fireball,
            Vec3::ZERO, Vec3::X, 1.0, 3.0,
        );
        assert!(!spell.is_expired());
        spell.elapsed = 3.0;
        assert!(spell.is_expired());
    }

    #[test]
    fn active_spell_to_sdf_fireball() {
        let spell = ActiveSpell::from_server(
            1, PlayerIdentity::local(), SpellType::Fireball,
            Vec3::new(1.0, 2.0, 3.0), Vec3::X, 1.0, 3.0,
        );
        let sdf = spell.to_sdf();
        assert!(matches!(sdf, SpellSdf::Fireball { .. }));
    }

    #[test]
    fn active_spell_to_sdf_shield() {
        let spell = ActiveSpell::from_server(
            2, PlayerIdentity::local(), SpellType::Shield,
            Vec3::ZERO, Vec3::Y, 1.5, SHIELD_DURATION,
        );
        let sdf = spell.to_sdf();
        assert!(matches!(sdf, SpellSdf::Shield { .. }));
    }

    #[test]
    fn sdf_evaluate_fireball_inside() {
        let sdf = SpellSdf::Fireball {
            center: Vec3::ZERO,
            radius: 2.0,
            noise_scale: 5.0,
            emission_color: Vec3::ONE,
            emission_intensity: 3.0,
        };
        // Point at center should be negative (inside)
        assert!(sdf.evaluate(Vec3::ZERO) < 0.0);
    }

    #[test]
    fn sdf_evaluate_fireball_outside() {
        let sdf = SpellSdf::Fireball {
            center: Vec3::ZERO,
            radius: 1.0,
            noise_scale: 5.0,
            emission_color: Vec3::ONE,
            emission_intensity: 3.0,
        };
        assert!(sdf.evaluate(Vec3::new(5.0, 0.0, 0.0)) > 0.0);
    }

    #[test]
    fn sdf_evaluate_shield_shell() {
        let sdf = SpellSdf::Shield {
            center: Vec3::ZERO,
            outer_radius: 2.0,
            thickness: 0.1,
            ior: 1.3,
            tint: Vec3::ONE,
        };
        // Center of shell should be outside (hollow)
        assert!(sdf.evaluate(Vec3::ZERO) > 0.0);
        // On the shell surface
        let on_shell = sdf.evaluate(Vec3::new(2.0, 0.0, 0.0));
        assert!(on_shell.abs() < 0.2);
    }

    #[test]
    fn sdf_evaluate_gravity() {
        let sdf = SpellSdf::Gravity {
            center: Vec3::new(5.0, 5.0, 5.0),
            radius: 3.0,
            strength: 1.0,
        };
        // Far away should be positive
        assert!(sdf.evaluate(Vec3::ZERO) > 0.0);
        // At center should be negative
        assert!(sdf.evaluate(Vec3::new(5.0, 5.0, 5.0)) < 0.0);
    }

    #[test]
    fn sdf_evaluate_lightning_on_beam() {
        let sdf = SpellSdf::Lightning {
            origin: Vec3::ZERO,
            direction: Vec3::X,
            length: 10.0,
            radius: 0.5,
            branch_noise: 0.1,
        };
        // Point on the beam axis should be inside
        assert!(sdf.evaluate(Vec3::new(5.0, 0.0, 0.0)) < 0.0);
        // Point far from beam should be outside
        assert!(sdf.evaluate(Vec3::new(5.0, 10.0, 0.0)) > 0.0);
    }

    #[test]
    fn spell_cooldowns_can_cast() {
        let cd = SpellCooldowns::default();
        assert!(cd.can_cast(SpellType::Fireball));
    }

    #[test]
    fn spell_cooldowns_trigger_blocks_cast() {
        let mut cd = SpellCooldowns::default();
        cd.trigger(SpellType::Fireball);
        assert!(!cd.can_cast(SpellType::Fireball));
        // Other types also blocked by global cooldown
        assert!(!cd.can_cast(SpellType::Shield));
    }

    #[test]
    fn spell_cooldowns_tick_recovers() {
        let mut cd = SpellCooldowns::default();
        cd.trigger(SpellType::Fireball);
        // Tick enough to clear global cooldown
        cd.tick(GLOBAL_COOLDOWN + 0.01);
        // Global is clear but per-type may not be
        assert!(cd.global_remaining <= 0.0);
    }

    #[test]
    fn spell_cooldowns_full_recovery() {
        let mut cd = SpellCooldowns::default();
        cd.trigger(SpellType::Shield);
        // Tick past all cooldowns
        cd.tick(SHIELD_DURATION);
        assert!(cd.can_cast(SpellType::Shield));
    }

    #[test]
    fn gpu_spell_data_size() {
        assert_eq!(std::mem::size_of::<GpuSpellData>(), 80);
    }

    #[test]
    fn gpu_spell_pack_roundtrip() {
        let spell = ActiveSpell::from_server(
            7, PlayerIdentity::new(99), SpellType::Fireball,
            Vec3::new(1.0, 2.0, 3.0), Vec3::Z, 1.5, 3.0,
        );
        let packed = SpellBuffer::pack_spell(&spell);
        assert_eq!(packed.spell_type, 0); // Fireball = 0
        assert_eq!(packed.pos, [1.0, 2.0, 3.0]);
        assert!(packed.power > 1.0);
    }

    #[test]
    fn emission_colors_distinct() {
        let fb = SpellType::Fireball.emission_color();
        let sh = SpellType::Shield.emission_color();
        assert!(fb != sh);
    }

    #[test]
    fn sd_capsule_line_on_axis() {
        let d = sd_capsule_line(Vec3::new(5.0, 0.0, 0.0), Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0), 0.5);
        assert!(d < 0.0); // inside
    }

    #[test]
    fn sd_capsule_line_far_away() {
        let d = sd_capsule_line(Vec3::new(5.0, 100.0, 0.0), Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0), 0.5);
        assert!(d > 0.0); // outside
    }
}
