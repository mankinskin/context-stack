//! WorldPanel — in-world glass SDF panels with content textures.
//!
//! Panels are glass shapes registered in the glass panel buffer and evaluated
//! in the tiled rasteriser's glass pre-loop. Each panel can billboard towards
//! the camera, attach to an entity, or remain fixed in world space.

use crate::render::glass::GlassPanel;
use bevy::prelude::*;

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

/// How a world panel is anchored in 3D space.
#[derive(Clone, Debug)]
pub enum PanelAnchor {
    /// Fixed world-space position and orientation.
    WorldFixed(Vec3, Quat),
    /// Follows an entity's transform with a local-space offset.
    EntityAttached(Entity, Vec3),
    /// Always faces the camera (y-up billboard).
    Billboard { target: Entity, offset: Vec3 },
}

/// A floating UI panel rendered as a glass SDF in the 3D scene.
#[derive(Component)]
pub struct WorldPanel {
    pub half_extents: Vec2,
    pub corner_radius: f32,
    pub roughness: f32,
    pub tint: [f32; 4],
    pub anchor: PanelAnchor,
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct WorldPanelPlugin;

impl Plugin for WorldPanelPlugin {
    fn build(
        &self,
        app: &mut App,
    ) {
        app.add_systems(Update, (billboard_system, entity_attach_system));
    }
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// Billboard panels rotate to face the camera.
fn billboard_system(
    camera_q: Query<&Transform, With<Camera3d>>,
    mut panels: Query<(&WorldPanel, &mut Transform), Without<Camera3d>>,
) {
    let Ok(cam_tf) = camera_q.single() else {
        return;
    };
    let cam_pos = cam_tf.translation;

    for (panel, mut tf) in &mut panels {
        if let PanelAnchor::Billboard { .. } = &panel.anchor {
            // Face towards camera (y-up)
            let target = cam_pos;
            let forward = (target - tf.translation).normalize_or_zero();
            if forward.length_squared() > 0.01 {
                tf.look_to(forward, Vec3::Y);
            }
        }
    }
}

/// Entity-attached panels follow their host entity's transform.
fn entity_attach_system(
    transforms: Query<&Transform, Without<WorldPanel>>,
    mut panels: Query<(&WorldPanel, &mut Transform)>,
) {
    for (panel, mut tf) in &mut panels {
        if let PanelAnchor::EntityAttached(entity, offset) = &panel.anchor {
            if let Ok(host_tf) = transforms.get(*entity) {
                tf.translation =
                    host_tf.translation + host_tf.rotation * *offset;
                tf.rotation = host_tf.rotation;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create a GlassPanel from a WorldPanel's parameters.
/// This lets in-world panels reuse the same glass SDF rendering path.
pub fn world_panel_to_glass(panel: &WorldPanel) -> GlassPanel {
    GlassPanel {
        ior: 1.45,
        tint: panel.tint,
        blur_roughness: panel.roughness,
        corner_radius: panel.corner_radius,
        half_size: Vec3::new(panel.half_extents.x, panel.half_extents.y, 0.02),
        caustic_strength: 0.0,
        chromatic_spread: 0.0,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_panel_to_glass_conversion() {
        let panel = WorldPanel {
            half_extents: Vec2::new(1.0, 0.5),
            corner_radius: 0.1,
            roughness: 0.0,
            tint: [1.0, 1.0, 1.0, 0.3],
            anchor: PanelAnchor::WorldFixed(Vec3::ZERO, Quat::IDENTITY),
        };
        let glass = world_panel_to_glass(&panel);
        assert_eq!(glass.half_size.x, 1.0);
        assert_eq!(glass.half_size.y, 0.5);
        assert_eq!(glass.ior, 1.45);
    }

    #[test]
    fn panel_anchor_variants() {
        let _fixed = PanelAnchor::WorldFixed(Vec3::ZERO, Quat::IDENTITY);
        let _attached =
            PanelAnchor::EntityAttached(Entity::PLACEHOLDER, Vec3::Y);
        let _billboard = PanelAnchor::Billboard {
            target: Entity::PLACEHOLDER,
            offset: Vec3::Y * 2.0,
        };
    }
}
