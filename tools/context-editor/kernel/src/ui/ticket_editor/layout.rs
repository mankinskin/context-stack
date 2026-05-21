use bevy::prelude::*;
use std::collections::HashMap;

use super::{
    TicketEdge,
    GRID_COLUMNS,
    GRID_SPACING,
    LAYOUT_DAMPING,
    MAX_VELOCITY,
    MIN_REPULSION_DIST,
    REPULSION_K,
    SPRING_K,
};

/// Compute grid positions for tickets (deterministic, no overlap).
pub fn grid_positions(ticket_ids: &[&str]) -> HashMap<String, Vec3> {
    let mut positions = HashMap::new();
    for (i, id) in ticket_ids.iter().enumerate() {
        let col = i % GRID_COLUMNS;
        let row = i / GRID_COLUMNS;
        let x = col as f32 * GRID_SPACING;
        let y = 0.0;
        let z = row as f32 * GRID_SPACING;
        positions.insert(id.to_string(), Vec3::new(x, y, z));
    }
    positions
}

/// Compute one step of force-directed layout.
pub fn force_directed_step(
    positions: &mut HashMap<String, Vec3>,
    velocities: &mut HashMap<String, Vec3>,
    edges: &[TicketEdge],
    dt: f32,
) {
    let ids: Vec<String> = positions.keys().cloned().collect();

    for i in 0..ids.len() {
        for j in (i + 1)..ids.len() {
            let pa = positions[&ids[i]];
            let pb = positions[&ids[j]];
            let delta = pa - pb;
            let dist = delta.length().max(MIN_REPULSION_DIST);
            let force_mag = REPULSION_K / (dist * dist);
            let force = delta.normalize_or_zero() * force_mag;

            if let Some(va) = velocities.get_mut(&ids[i]) {
                *va += force * dt;
            }
            if let Some(vb) = velocities.get_mut(&ids[j]) {
                *vb -= force * dt;
            }
        }
    }

    for edge in edges {
        if let (Some(&pa), Some(&pb)) =
            (positions.get(&edge.from), positions.get(&edge.to))
        {
            let delta = pb - pa;
            let dist = delta.length();
            let force = delta.normalize_or_zero() * SPRING_K * dist;

            if let Some(va) = velocities.get_mut(&edge.from) {
                *va += force * dt;
            }
            if let Some(vb) = velocities.get_mut(&edge.to) {
                *vb -= force * dt;
            }
        }
    }

    for id in &ids {
        if let (Some(vel), Some(pos)) =
            (velocities.get_mut(id), positions.get_mut(id))
        {
            *vel *= LAYOUT_DAMPING;
            let speed = vel.length();
            if speed > MAX_VELOCITY {
                *vel = vel.normalize() * MAX_VELOCITY;
            }
            *pos += *vel * dt;
        }
    }
}

/// Draw a 3D voxel line between two world positions (Bresenham-like).
pub fn voxel_line_3d(
    from: Vec3,
    to: Vec3,
) -> Vec<(i32, i32, i32)> {
    let mut result = Vec::new();
    let delta = to - from;
    let steps = delta.abs().max_element().ceil().max(1.0) as usize;

    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let p = from + delta * t;
        let ix = p.x.round() as i32;
        let iy = p.y.round() as i32;
        let iz = p.z.round() as i32;
        if result.last() != Some(&(ix, iy, iz)) {
            result.push((ix, iy, iz));
        }
    }
    result
}
