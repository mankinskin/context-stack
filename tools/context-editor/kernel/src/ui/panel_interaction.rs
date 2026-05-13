//! Panel Interaction — 3D ray-cast hit testing and input handling for WorldPanels.
//!
//! Casts a ray from the camera through the mouse position, intersects with
//! panel planes, and dispatches input events (hover, click, drag).
//! Panels have priority over world geometry (SVO raycasts) for input.

use crate::world_panel::WorldPanel;
use bevy::prelude::*;

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

/// Tracks the currently hovered panel and its local UV hit position.
#[derive(Resource, Default)]
pub struct HoveredPanel {
    pub entity: Option<(Entity, Vec2)>,
}

/// Tracks the most recent panel click (consumed by downstream systems).
#[derive(Resource, Default)]
pub struct PanelClick {
    pub hit: Option<(Entity, Vec2)>,
}

/// Tracks active drag state for panel repositioning.
#[derive(Resource, Default)]
pub struct PanelDragState {
    pub dragging: Option<Entity>,
    pub drag_offset: Vec3,
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct PanelInteractionPlugin;

impl Plugin for PanelInteractionPlugin {
    fn build(
        &self,
        app: &mut App,
    ) {
        app.init_resource::<HoveredPanel>();
        app.init_resource::<PanelClick>();
        app.init_resource::<PanelDragState>();
        app.add_systems(
            Update,
            (panel_raycast_system, panel_input_system, panel_drag_system),
        );
    }
}

// ---------------------------------------------------------------------------
// Ray helpers
// ---------------------------------------------------------------------------

/// A simple ray: origin + normalised direction.
pub struct Ray3 {
    pub origin: Vec3,
    pub direction: Vec3,
}

/// Build a camera ray from a screen-space position and camera transform/projection.
pub fn camera_ray(
    screen_pos: Vec2,
    camera_tf: &GlobalTransform,
    projection: &Projection,
    window_size: Vec2,
) -> Ray3 {
    // Convert screen pos to NDC (-1..1)
    let ndc = Vec2::new(
        (screen_pos.x / window_size.x) * 2.0 - 1.0,
        1.0 - (screen_pos.y / window_size.y) * 2.0,
    );

    let near = match projection {
        Projection::Perspective(persp) => {
            let half_h = (persp.fov * 0.5).tan() * persp.near;
            let half_w = half_h * persp.aspect_ratio;
            Vec3::new(ndc.x * half_w, ndc.y * half_h, -persp.near)
        },
        Projection::Orthographic(ortho) => {
            let half_w = (ortho.area.max.x - ortho.area.min.x) * 0.5;
            let half_h = (ortho.area.max.y - ortho.area.min.y) * 0.5;
            Vec3::new(ndc.x * half_w, ndc.y * half_h, -ortho.near)
        },
        _ => {
            // Custom projections: fall back to a sensible default direction
            Vec3::new(ndc.x, ndc.y, -1.0)
        },
    };

    let ray_dir = camera_tf
        .compute_transform()
        .rotation
        .mul_vec3(near)
        .normalize();
    let ray_origin = camera_tf.translation();

    Ray3 {
        origin: ray_origin,
        direction: ray_dir,
    }
}

/// Intersect a ray with a panel plane. Returns (distance, panel_uv) or None.
///
/// The panel is centred at its transform's translation, oriented according to
/// its rotation, and sized by `half_extents`. The panel lies in the local XY
/// plane (normal = local +Z).
pub fn ray_panel_intersect(
    ray: &Ray3,
    panel_tf: &GlobalTransform,
    panel: &WorldPanel,
) -> Option<(f32, Vec2)> {
    let tf = panel_tf.compute_transform();
    let panel_normal = tf.rotation.mul_vec3(Vec3::Z);
    let panel_center = tf.translation;

    let denom = panel_normal.dot(ray.direction);
    // Back-face culling and near-parallel rejection
    if denom.abs() < 1e-6 {
        return None;
    }

    let t = (panel_center - ray.origin).dot(panel_normal) / denom;
    // Only forward hits
    if t < 0.0 {
        return None;
    }

    let hit_world = ray.origin + ray.direction * t;
    let local = tf.rotation.inverse().mul_vec3(hit_world - panel_center);

    let u = local.x / panel.half_extents.x;
    let v = local.y / panel.half_extents.y;

    // Check bounds [-1, 1]
    if u.abs() > 1.0 || v.abs() > 1.0 {
        return None;
    }

    // Map to [0, 1] UV
    let uv = Vec2::new((u + 1.0) * 0.5, (1.0 - v) * 0.5);
    Some((t, uv))
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// Ray-cast from camera through mouse position to find the closest panel hit.
fn panel_raycast_system(
    windows: Query<&Window>,
    mut hovered: ResMut<HoveredPanel>,
    camera_q: Query<(&GlobalTransform, &Projection), With<Camera3d>>,
    panels: Query<(&WorldPanel, &GlobalTransform, Entity)>,
) {
    hovered.entity = None;

    let Ok(window) = windows.single() else {
        return;
    };
    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };
    let Ok((cam_tf, proj)) = camera_q.single() else {
        return;
    };

    let window_size = Vec2::new(window.width(), window.height());
    let ray = camera_ray(cursor_pos, cam_tf, proj, window_size);

    let mut closest: Option<(Entity, f32, Vec2)> = None;

    for (panel, panel_tf, entity) in panels.iter() {
        if let Some((dist, uv)) = ray_panel_intersect(&ray, panel_tf, panel) {
            if closest.is_none() || dist < closest.unwrap().1 {
                closest = Some((entity, dist, uv));
            }
        }
    }

    hovered.entity = closest.map(|(e, _, uv)| (e, uv));
}

/// Dispatch hover and click events based on the currently hovered panel.
fn panel_input_system(
    hovered: Res<HoveredPanel>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut click: ResMut<PanelClick>,
) {
    click.hit = None;
    if let Some((entity, uv)) = hovered.entity {
        if mouse.just_pressed(MouseButton::Left) {
            click.hit = Some((entity, uv));
        }
    }
}

/// Drag-to-reposition panels on a plane perpendicular to the camera.
fn panel_drag_system(
    mouse: Res<ButtonInput<MouseButton>>,
    hovered: Res<HoveredPanel>,
    mut drag: ResMut<PanelDragState>,
    camera_q: Query<(&GlobalTransform, &Projection), With<Camera3d>>,
    windows: Query<&Window>,
    mut panel_tfs: Query<&mut Transform, With<WorldPanel>>,
) {
    // Start drag
    if mouse.just_pressed(MouseButton::Left) {
        if let Some((entity, _uv)) = hovered.entity {
            if let Ok(tf) = panel_tfs.get(entity) {
                drag.dragging = Some(entity);
                drag.drag_offset = tf.translation;
            }
        }
    }

    // End drag
    if mouse.just_released(MouseButton::Left) {
        drag.dragging = None;
        return;
    }

    // Continue drag
    let Some(entity) = drag.dragging else { return };
    let Ok(window) = windows.single() else { return };
    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };
    let Ok((cam_tf, proj)) = camera_q.single() else {
        return;
    };

    let window_size = Vec2::new(window.width(), window.height());
    let ray = camera_ray(cursor_pos, cam_tf, proj, window_size);

    // Project onto a plane at the original drag depth perpendicular to camera forward
    let cam_forward = cam_tf.compute_transform().rotation.mul_vec3(-Vec3::Z);
    let plane_d = cam_forward.dot(drag.drag_offset);
    let denom = cam_forward.dot(ray.direction);
    if denom.abs() < 1e-6 {
        return;
    }
    let t = (plane_d - cam_forward.dot(ray.origin)) / denom;
    if t < 0.0 {
        return;
    }

    let new_pos = ray.origin + ray.direction * t;
    if let Ok(mut tf) = panel_tfs.get_mut(entity) {
        tf.translation = new_pos;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ray_hits_front_panel() {
        let panel = WorldPanel {
            half_extents: Vec2::new(1.0, 1.0),
            corner_radius: 0.0,
            roughness: 0.5,
            tint: [1.0; 4],
            anchor: crate::world_panel::PanelAnchor::WorldFixed(
                Vec3::ZERO,
                Quat::IDENTITY,
            ),
        };
        let tf = GlobalTransform::from_translation(Vec3::new(0.0, 0.0, -5.0));
        let ray = Ray3 {
            origin: Vec3::ZERO,
            direction: -Vec3::Z,
        };
        let result = ray_panel_intersect(&ray, &tf, &panel);
        assert!(result.is_some());
        let (dist, uv) = result.unwrap();
        assert!((dist - 5.0).abs() < 0.01);
        assert!((uv.x - 0.5).abs() < 0.01);
        assert!((uv.y - 0.5).abs() < 0.01);
    }

    #[test]
    fn ray_misses_behind_camera() {
        let panel = WorldPanel {
            half_extents: Vec2::new(1.0, 1.0),
            corner_radius: 0.0,
            roughness: 0.5,
            tint: [1.0; 4],
            anchor: crate::world_panel::PanelAnchor::WorldFixed(
                Vec3::ZERO,
                Quat::IDENTITY,
            ),
        };
        let tf = GlobalTransform::from_translation(Vec3::new(0.0, 0.0, 5.0));
        let ray = Ray3 {
            origin: Vec3::ZERO,
            direction: -Vec3::Z,
        };
        let result = ray_panel_intersect(&ray, &tf, &panel);
        assert!(result.is_none());
    }

    #[test]
    fn ray_misses_outside_extents() {
        let panel = WorldPanel {
            half_extents: Vec2::new(0.5, 0.5),
            corner_radius: 0.0,
            roughness: 0.5,
            tint: [1.0; 4],
            anchor: crate::world_panel::PanelAnchor::WorldFixed(
                Vec3::ZERO,
                Quat::IDENTITY,
            ),
        };
        let tf = GlobalTransform::from_translation(Vec3::new(0.0, 0.0, -5.0));
        // Ray aimed at offset (2, 0), panel only +-0.5
        let ray = Ray3 {
            origin: Vec3::new(2.0, 0.0, 0.0),
            direction: -Vec3::Z,
        };
        let result = ray_panel_intersect(&ray, &tf, &panel);
        assert!(result.is_none());
    }
}
