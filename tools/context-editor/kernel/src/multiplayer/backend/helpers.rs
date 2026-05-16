use super::{
    CHUNK_SIZE,
    MAX_INTERACTION_RANGE,
};

/// Compute a spatial hash for chunk coordinates.
///
/// Uses FNV-1a-style mixing to distribute chunks across hash space.
pub fn spatial_hash(
    cx: i32,
    cy: i32,
    cz: i32,
) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for byte in cx
        .to_le_bytes()
        .iter()
        .chain(&cy.to_le_bytes())
        .chain(&cz.to_le_bytes())
    {
        h ^= *byte as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Convert world voxel coordinates to chunk coordinates.
pub fn world_to_chunk(
    x: i32,
    y: i32,
    z: i32,
) -> (i32, i32, i32) {
    (
        x.div_euclid(CHUNK_SIZE as i32),
        y.div_euclid(CHUNK_SIZE as i32),
        z.div_euclid(CHUNK_SIZE as i32),
    )
}

/// Convert world voxel coordinates to local-within-chunk coordinates.
pub fn world_to_local(
    x: i32,
    y: i32,
    z: i32,
) -> (u8, u8, u8) {
    (
        x.rem_euclid(CHUNK_SIZE as i32) as u8,
        y.rem_euclid(CHUNK_SIZE as i32) as u8,
        z.rem_euclid(CHUNK_SIZE as i32) as u8,
    )
}

/// Validate that a player is within interaction range of a target position.
pub fn validate_interaction_range(
    player_pos: (f32, f32, f32),
    target: (f32, f32, f32),
) -> bool {
    let dx = player_pos.0 - target.0;
    let dy = player_pos.1 - target.1;
    let dz = player_pos.2 - target.2;
    (dx * dx + dy * dy + dz * dz).sqrt() <= MAX_INTERACTION_RANGE
}

/// Compute damage from weapon stats against a target.
pub fn compute_damage(
    base_damage: i32,
    weapon_weight: f32,
) -> i32 {
    let weight_bonus = (weapon_weight * 2.0) as i32;
    (base_damage + weight_bonus).max(1)
}
