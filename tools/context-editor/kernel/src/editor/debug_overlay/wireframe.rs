use super::*;

/// Walk the SVO and populate the wireframe overlay vertex buffer.
pub(super) fn draw_svo_wireframe(
    state: Res<DebugOverlayState>,
    voxel_world: Res<VoxelWorld>,
    mut wire_data: Option<
        ResMut<crate::render::wireframe_overlay::WireframeData>,
    >,
    camera_q: Query<&Transform, With<Camera3d>>,
) {
    if let Some(ref mut data) = wire_data {
        data.corners.clear();
        data.indices.clear();
    }

    if !state.enabled || voxel_world.nodes.is_empty() {
        return;
    }
    let target = state.display_depth.min(voxel_world.max_depth);
    let occ_only = state.occupied_only;
    let root_extent = (1u32 << voxel_world.max_depth) as f32;

    const MAX_CUBES: usize =
        crate::render::wireframe_overlay::MAX_WIREFRAME_CUBES as usize;

    let mut cubes: Vec<(Vec3, f32)> = Vec::new();

    if occ_only {
        let cam_pos = camera_q
            .single()
            .map(|transform| transform.translation)
            .unwrap_or(Vec3::ZERO);

        let mut queue: std::collections::VecDeque<(usize, u32, Vec3, f32)> =
            std::collections::VecDeque::new();
        queue.push_back((
            voxel_world.root_index as usize,
            0,
            Vec3::ZERO,
            root_extent,
        ));

        while let Some((idx, depth, origin, extent)) = queue.pop_front() {
            if idx >= voxel_world.nodes.len() {
                continue;
            }
            let node = &voxel_world.nodes[idx];
            let half = extent * 0.5;
            let center = origin + Vec3::splat(half);

            if node.color_data == 0 && node.child_mask() == 0 {
                continue;
            }

            if depth >= target || node.is_leaf() {
                cubes.push((center, extent));
                continue;
            }

            let mask = node.child_mask();
            let first = node.first_child_index();
            for slot in 0..8usize {
                if mask & (1 << slot) == 0 {
                    continue;
                }
                let child_origin = origin
                    + Vec3::new(
                        if slot & 1 != 0 { half } else { 0.0 },
                        if slot & 2 != 0 { half } else { 0.0 },
                        if slot & 4 != 0 { half } else { 0.0 },
                    );
                queue.push_back((first + slot, depth + 1, child_origin, half));
            }
        }

        if cubes.len() > MAX_CUBES {
            cubes.sort_unstable_by(|(a, _), (b, _)| {
                a.distance_squared(cam_pos)
                    .partial_cmp(&b.distance_squared(cam_pos))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            cubes.truncate(MAX_CUBES);
        }
    } else {
        let full_grid_cap: u32 = {
            let mut depth = 0u32;
            let mut cells = 1usize;
            while cells.saturating_mul(8) <= MAX_CUBES {
                depth += 1;
                cells = cells.saturating_mul(8);
            }
            depth
        };

        let mut queue: std::collections::VecDeque<(usize, u32, Vec3, f32)> =
            std::collections::VecDeque::new();
        queue.push_back((
            voxel_world.root_index as usize,
            0,
            Vec3::ZERO,
            root_extent,
        ));

        while let Some((idx, depth, origin, extent)) = queue.pop_front() {
            if cubes.len() >= MAX_CUBES {
                break;
            }
            let center = origin + Vec3::splat(extent * 0.5);
            let half = extent * 0.5;

            if idx == usize::MAX {
                let empty_target = target.min(full_grid_cap);
                if depth >= empty_target {
                    cubes.push((center, extent));
                } else {
                    for slot in 0..8usize {
                        let child_origin = origin
                            + Vec3::new(
                                if slot & 1 != 0 { half } else { 0.0 },
                                if slot & 2 != 0 { half } else { 0.0 },
                                if slot & 4 != 0 { half } else { 0.0 },
                            );
                        queue.push_back((
                            usize::MAX,
                            depth + 1,
                            child_origin,
                            half,
                        ));
                    }
                }
                continue;
            }

            if idx >= voxel_world.nodes.len() {
                continue;
            }
            let node = &voxel_world.nodes[idx];

            if depth >= target || node.is_leaf() {
                cubes.push((center, extent));
                continue;
            }

            let mask = node.child_mask();
            let first = node.first_child_index();
            for slot in 0..8usize {
                let child_origin = origin
                    + Vec3::new(
                        if slot & 1 != 0 { half } else { 0.0 },
                        if slot & 2 != 0 { half } else { 0.0 },
                        if slot & 4 != 0 { half } else { 0.0 },
                    );
                if mask & (1 << slot) == 0 {
                    if depth + 1 <= full_grid_cap {
                        queue.push_back((
                            usize::MAX,
                            depth + 1,
                            child_origin,
                            half,
                        ));
                    }
                } else {
                    queue.push_back((
                        first + slot,
                        depth + 1,
                        child_origin,
                        half,
                    ));
                }
            }
        }
    }

    if cubes.is_empty() {
        return;
    }

    let Some(ref mut wire_data) = wire_data else {
        return;
    };

    use crate::render::wireframe_overlay::{
        WIREFRAME_INDICES_PER_CUBE,
        WIREFRAME_VERTS_PER_CUBE,
    };
    const CUBE_EDGES: [(u32, u32); 12] = [
        (0, 1),
        (1, 2),
        (2, 3),
        (3, 0),
        (4, 5),
        (5, 6),
        (6, 7),
        (7, 4),
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7),
    ];

    wire_data
        .corners
        .reserve(cubes.len() * WIREFRAME_VERTS_PER_CUBE);
    wire_data
        .indices
        .reserve(cubes.len() * WIREFRAME_INDICES_PER_CUBE);
    for (i, &(center, extent)) in cubes.iter().enumerate() {
        let base = (i * WIREFRAME_VERTS_PER_CUBE) as u32;
        let half = extent * 0.5;
        wire_data
            .corners
            .push((center + Vec3::new(-half, -half, -half)).to_array());
        wire_data
            .corners
            .push((center + Vec3::new(half, -half, -half)).to_array());
        wire_data
            .corners
            .push((center + Vec3::new(half, half, -half)).to_array());
        wire_data
            .corners
            .push((center + Vec3::new(-half, half, -half)).to_array());
        wire_data
            .corners
            .push((center + Vec3::new(-half, -half, half)).to_array());
        wire_data
            .corners
            .push((center + Vec3::new(half, -half, half)).to_array());
        wire_data
            .corners
            .push((center + Vec3::new(half, half, half)).to_array());
        wire_data
            .corners
            .push((center + Vec3::new(-half, half, half)).to_array());
        for &(a, b) in &CUBE_EDGES {
            wire_data.indices.push(base + a);
            wire_data.indices.push(base + b);
        }
    }
}
