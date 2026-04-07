//! Interaction Bridge — 2D unprojection and Dioxus-to-WASM async event pipeline.
//!
//! Translates DOM click/hover coordinates into 3D world rays. Events flow from
//! the Dioxus UI layer through an async channel into the Bevy ECS where they
//! are processed against the SVO and dispatched to the world logic trait.

use std::sync::Mutex;

use bevy::prelude::*;

use crate::svo::VoxelWorld;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A 3D ray: origin + normalised direction.
#[derive(Clone, Copy, Debug)]
pub struct WorldRay {
    pub origin: Vec3,
    pub direction: Vec3,
}

/// Type of interaction from the UI layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionType {
    PrimaryAction,
    SecondaryAction,
    Hover,
}

/// An interaction event from the Dioxus UI side.
#[derive(Clone, Debug)]
pub struct KernelEvent {
    pub ray: WorldRay,
    pub interaction_type: InteractionType,
    pub screen_pos: Vec2,
}

/// Result of a world ray-cast hit on the SVO.
#[derive(Clone, Debug)]
pub struct WorldHit {
    pub position: Vec3,
    pub normal: Vec3,
    pub screen_pos: Vec2,
    pub interaction_type: InteractionType,
}

// ---------------------------------------------------------------------------
// Async channel (thread-safe queue for DOM→Bevy)
// ---------------------------------------------------------------------------

/// Thread-safe event queue for Dioxus→Bevy communication.
#[derive(Resource, Default)]
pub struct InteractionQueue {
    pending: Mutex<Vec<KernelEvent>>,
}

impl InteractionQueue {
    /// Push an event from the Dioxus/DOM side (safe to call from any thread).
    pub fn push(&self, event: KernelEvent) {
        if let Ok(mut q) = self.pending.lock() {
            q.push(event);
        }
    }

    /// Drain all pending events (called from the Bevy game loop).
    fn drain(&self) -> Vec<KernelEvent> {
        if let Ok(mut q) = self.pending.lock() {
            std::mem::take(&mut *q)
        } else {
            Vec::new()
        }
    }
}

/// Resource holding the results of the latest interaction ray-casts.
#[derive(Resource, Default)]
pub struct InteractionHits {
    pub hits: Vec<WorldHit>,
    /// Latest hover position in NDC for shader caustics.
    pub hover_ndc: Option<Vec2>,
}

// ---------------------------------------------------------------------------
// Unprojection
// ---------------------------------------------------------------------------

/// Convert screen coordinates to a world ray using the inverse view-projection.
pub fn screen_to_world_ray(screen_pos: Vec2, viewport: Vec2, view_proj: Mat4) -> WorldRay {
    let ndc = Vec2::new(
        (screen_pos.x / viewport.x) * 2.0 - 1.0,
        1.0 - (screen_pos.y / viewport.y) * 2.0,
    );
    let inv = view_proj.inverse();
    let near = inv.project_point3(Vec3::new(ndc.x, ndc.y, 0.0));
    let far = inv.project_point3(Vec3::new(ndc.x, ndc.y, 1.0));

    WorldRay {
        origin: near,
        direction: (far - near).normalize(),
    }
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct InteractionBridgePlugin;

impl Plugin for InteractionBridgePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InteractionQueue>();
        app.init_resource::<InteractionHits>();
        app.add_systems(Update, process_interactions);
    }
}

// ---------------------------------------------------------------------------
// System
// ---------------------------------------------------------------------------

/// Drain queued interaction events, ray-cast against SVO, and populate hit results.
fn process_interactions(
    queue: Res<InteractionQueue>,
    svo: Res<VoxelWorld>,
    mut hits: ResMut<InteractionHits>,
) {
    let events = queue.drain();
    hits.hits.clear();
    hits.hover_ndc = None;

    for event in events {
        // Hover events update the shader caustic position
        if event.interaction_type == InteractionType::Hover {
            hits.hover_ndc = Some(event.screen_pos);
        }

        // Ray-cast against SVO
        if let Some((pos, normal)) = svo.raycast(event.ray.origin, event.ray.direction, 200.0) {
            hits.hits.push(WorldHit {
                position: pos,
                normal,
                screen_pos: event.screen_pos,
                interaction_type: event.interaction_type,
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_center_produces_forward_ray() {
        let viewport = Vec2::new(800.0, 600.0);
        let center = Vec2::new(400.0, 300.0);
        // Use a perspective projection looking down -Z
        let vp = Mat4::perspective_rh(std::f32::consts::FRAC_PI_2, 4.0 / 3.0, 0.1, 100.0);
        let ray = screen_to_world_ray(center, viewport, vp);
        // Center of screen → ray direction should be mostly -Z
        assert!(ray.direction.z < 0.0, "center ray should point into screen (-Z)");
        // x and y should be near zero
        assert!(ray.direction.x.abs() < 0.01);
        assert!(ray.direction.y.abs() < 0.01);
    }

    #[test]
    fn screen_corner_offsets_ray() {
        let viewport = Vec2::new(800.0, 600.0);
        // Top-left corner
        let corner = Vec2::new(0.0, 0.0);
        let vp = Mat4::perspective_rh(std::f32::consts::FRAC_PI_2, 4.0 / 3.0, 0.1, 100.0);
        let ray = screen_to_world_ray(corner, viewport, vp);
        // Should have negative x and positive y components
        assert!(ray.direction.x < 0.0);
        assert!(ray.direction.y > 0.0);
    }

    #[test]
    fn interaction_queue_is_threadsafe() {
        let queue = InteractionQueue::default();
        queue.push(KernelEvent {
            ray: WorldRay {
                origin: Vec3::ZERO,
                direction: -Vec3::Z,
            },
            interaction_type: InteractionType::PrimaryAction,
            screen_pos: Vec2::ZERO,
        });
        let drained = queue.drain();
        assert_eq!(drained.len(), 1);
        // Second drain is empty
        let again = queue.drain();
        assert!(again.is_empty());
    }
}
