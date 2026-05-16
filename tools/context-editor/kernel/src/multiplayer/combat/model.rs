use super::{
    FIST_REACH,
    MAX_WEAPON_REACH,
};
use bevy::prelude::*;

use crate::{
    multiplayer_backend::{
        ItemBlueprint,
        PlayerIdentity,
    },
    multiplayer_chars::{
        sd_capsule,
        CAPSULE_HEIGHT,
        CAPSULE_RADIUS,
    },
};

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
            let reach = bounds.x * 0.1;
            WeaponSdf::Sword {
                half_extents: bounds * 0.05,
                reach: reach.min(MAX_WEAPON_REACH),
            }
        } else if aspect_yx > 2.0 {
            WeaponSdf::Hammer {
                handle_length: bounds.y * 0.08,
                head_radius: bounds.x.max(bounds.z) * 0.06,
            }
        } else {
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
