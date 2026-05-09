//! Core voxel editor — paint/carve tools driven by mouse input and SVO raycasting.
//!
//! ## Pipeline
//!
//! ```text
//! Mouse input → ray from camera → SVO raycast → paint/carve
//!   → dirty ranges → upload → splat regen → visual update
//! ```

use bevy::prelude::*;

use crate::svo::{
    VoxelMaterial,
    VoxelWorld,
};

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

/// Which editing tool is active.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorTool {
    Paint,
    Carve,
}

/// Per-frame editor state.
#[derive(Resource)]
pub struct EditorState {
    pub active_tool: EditorTool,
    pub brush_size: u32,
    pub current_material: VoxelMaterial,
    /// Whether the editor is enabled (false while navigating)
    pub enabled: bool,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            active_tool: EditorTool::Paint,
            brush_size: 2,
            current_material: VoxelMaterial::new(128, 200, 220, 16),
            enabled: false,
        }
    }
}

/// Result of a ray-octree intersection.
#[derive(Resource, Default)]
pub struct VoxelHit {
    pub hit: Option<HitInfo>,
}

#[derive(Clone, Debug)]
pub struct HitInfo {
    pub position: Vec3,
    pub normal: Vec3,
    pub cell: IVec3,
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct EditorPlugin;

impl Plugin for EditorPlugin {
    fn build(
        &self,
        app: &mut App,
    ) {
        app.init_resource::<EditorState>();
        app.init_resource::<VoxelHit>();
        app.add_systems(
            Update,
            (
                toggle_editor,
                toggle_tool,
                adjust_brush_size,
                editor_raycast,
                apply_tool,
            )
                .chain(),
        );
    }
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// Press E to toggle the editor on/off.
fn toggle_editor(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<EditorState>,
) {
    if keys.just_pressed(KeyCode::KeyE) {
        state.enabled = !state.enabled;
    }
}

/// Press 1 for paint, 2 for carve.
fn toggle_tool(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<EditorState>,
) {
    if !state.enabled {
        return;
    }
    if keys.just_pressed(KeyCode::Digit1) {
        state.active_tool = EditorTool::Paint;
    }
    if keys.just_pressed(KeyCode::Digit2) {
        state.active_tool = EditorTool::Carve;
    }
}

/// Scroll wheel (or +/−) to adjust brush size.
fn adjust_brush_size(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<EditorState>,
) {
    if !state.enabled {
        return;
    }
    if keys.just_pressed(KeyCode::BracketRight) {
        state.brush_size = (state.brush_size + 1).min(8);
    }
    if keys.just_pressed(KeyCode::BracketLeft) {
        state.brush_size = state.brush_size.saturating_sub(1).max(1);
    }
}

/// Cast a ray from the camera centre into the SVO and store the hit.
fn editor_raycast(
    state: Res<EditorState>,
    svo: Res<VoxelWorld>,
    camera_q: Query<&Transform, With<Camera3d>>,
    mut hit: ResMut<VoxelHit>,
) {
    hit.hit = None;
    if !state.enabled {
        return;
    }

    let Ok(cam_tf) = camera_q.single() else {
        return;
    };

    let origin = cam_tf.translation;
    let direction = *cam_tf.forward();

    if let Some((pos, normal)) = svo.raycast(origin, direction, 200.0) {
        let cell = pos.floor().as_ivec3();
        hit.hit = Some(HitInfo {
            position: pos,
            normal,
            cell,
        });
    }
}

/// Apply paint/carve when left-click is held (and editor is active).
fn apply_tool(
    mouse: Res<ButtonInput<MouseButton>>,
    state: Res<EditorState>,
    hit: Res<VoxelHit>,
    mut svo: ResMut<VoxelWorld>,
) {
    if !state.enabled || !mouse.pressed(MouseButton::Right) {
        return;
    }
    let Some(ref info) = hit.hit else { return };

    let brush_r = state.brush_size as f32;
    let brush_ri = state.brush_size as i32;

    match state.active_tool {
        EditorTool::Paint => {
            // Place voxels offset by the surface normal
            let center = info.cell + info.normal.round().as_ivec3();
            for dx in -brush_ri..=brush_ri {
                for dy in -brush_ri..=brush_ri {
                    for dz in -brush_ri..=brush_ri {
                        let off = IVec3::new(dx, dy, dz);
                        if off.as_vec3().length() <= brush_r {
                            svo.set_voxel(center + off, state.current_material);
                        }
                    }
                }
            }
        },
        EditorTool::Carve => {
            let center = info.cell;
            for dx in -brush_ri..=brush_ri {
                for dy in -brush_ri..=brush_ri {
                    for dz in -brush_ri..=brush_ri {
                        let off = IVec3::new(dx, dy, dz);
                        if off.as_vec3().length() <= brush_r {
                            svo.remove_voxel(center + off);
                        }
                    }
                }
            }
        },
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_state_defaults() {
        let state = EditorState::default();
        assert_eq!(state.active_tool, EditorTool::Paint);
        assert_eq!(state.brush_size, 2);
        assert!(!state.enabled);
    }

    #[test]
    fn brush_sphere_coverage() {
        // Brush size 1 should cover a 3x3x3 area, filtering by distance
        let brush_r = 1.0f32;
        let brush_ri = 1i32;
        let mut count = 0u32;
        for dx in -brush_ri..=brush_ri {
            for dy in -brush_ri..=brush_ri {
                for dz in -brush_ri..=brush_ri {
                    let off = IVec3::new(dx, dy, dz);
                    if off.as_vec3().length() <= brush_r {
                        count += 1;
                    }
                }
            }
        }
        // Centre + 6 face neighbours = 7
        assert_eq!(count, 7);
    }

    #[test]
    fn hit_test_raycast() {
        let mut world = VoxelWorld::new(8);
        let mat = VoxelMaterial::new(255, 0, 0, 16);
        // Place a block at (128,128,128)
        world.set_voxel(IVec3::new(128, 128, 128), mat);

        // Cast towards it
        let origin = Vec3::new(128.5, 128.5, 140.0);
        let dir = Vec3::new(0.0, 0.0, -1.0);
        let result = world.raycast(origin, dir, 50.0);
        assert!(result.is_some());
    }
}
