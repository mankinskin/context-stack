//! Debug overlay — SVO wireframe visualization and settings panel.
//!
//! Provides a [`DebugOverlayPlugin`] that draws octree cell wireframes via
//! spawned line-mesh entities, and a [`DebugPanel`] Dioxus component for
//! interactive control.
//!
//! Communication between Dioxus (HTML thread) and Bevy (ECS) uses lock-free
//! atomics: the UI writes settings, and the Bevy system reads them each frame.

use bevy::prelude::*;
use dioxus::prelude::*;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use crate::svo::VoxelWorld;
use crate::ui::GlassPanel;

// Re-alias Bevy's KeyCode to avoid ambiguity with dioxus::prelude::KeyCode.
use bevy::input::keyboard::KeyCode as BevyKeyCode;

// ---------------------------------------------------------------------------
// Shared atomic state (Dioxus ↔ Bevy)
// ---------------------------------------------------------------------------

static WIREFRAME_ENABLED: AtomicBool = AtomicBool::new(false);
static WIREFRAME_DEPTH: AtomicU32 = AtomicU32::new(4);
static WIREFRAME_OCCUPIED: AtomicBool = AtomicBool::new(true);

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct DebugOverlayPlugin;

impl Plugin for DebugOverlayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DebugOverlayState>();
        app.add_systems(Update, (
            toggle_debug_keys,
            sync_debug_from_shared,
            draw_svo_wireframe,
        ).chain());
    }
}

// ---------------------------------------------------------------------------
// Bevy resource
// ---------------------------------------------------------------------------

#[derive(Resource)]
pub struct DebugOverlayState {
    pub enabled: bool,
    pub display_depth: u32,
    pub draw_radius: f32,
    pub wire_color: Color,
    pub occupied_only: bool,
}

impl Default for DebugOverlayState {
    fn default() -> Self {
        Self {
            enabled: false,
            display_depth: 4,
            draw_radius: 60.0,
            wire_color: Color::srgb(0.0, 1.0, 0.0),
            occupied_only: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Bevy systems
// ---------------------------------------------------------------------------

fn toggle_debug_keys(keys: Res<ButtonInput<BevyKeyCode>>) {
    if keys.just_pressed(BevyKeyCode::F3) {
        let v = WIREFRAME_ENABLED.load(Ordering::Relaxed);
        WIREFRAME_ENABLED.store(!v, Ordering::Relaxed);
    }
    if keys.just_pressed(BevyKeyCode::F4) {
        let v = WIREFRAME_DEPTH.load(Ordering::Relaxed);
        if v > 0 {
            WIREFRAME_DEPTH.store(v - 1, Ordering::Relaxed);
        }
    }
    if keys.just_pressed(BevyKeyCode::F5) {
        let v = WIREFRAME_DEPTH.load(Ordering::Relaxed);
        WIREFRAME_DEPTH.store(v.min(7) + 1, Ordering::Relaxed);
    }
    if keys.just_pressed(BevyKeyCode::F6) {
        let v = WIREFRAME_OCCUPIED.load(Ordering::Relaxed);
        WIREFRAME_OCCUPIED.store(!v, Ordering::Relaxed);
    }
}

/// Pull shared atomic state into the Bevy resource each frame.
fn sync_debug_from_shared(mut state: ResMut<DebugOverlayState>) {
    state.enabled = WIREFRAME_ENABLED.load(Ordering::Relaxed);
    state.display_depth = WIREFRAME_DEPTH.load(Ordering::Relaxed);
    state.occupied_only = WIREFRAME_OCCUPIED.load(Ordering::Relaxed);
}

/// Walk the SVO and populate the wireframe overlay vertex buffer.
///
/// Each frame we recompute the line positions and write them to the
/// [`WireframeVertices`] resource, which the GPU overlay node uploads and draws
/// on top of the voxel splats.
fn draw_svo_wireframe(
    state: Res<DebugOverlayState>,
    voxel_world: Res<VoxelWorld>,
    camera_q: Query<&Transform, With<Camera3d>>,
    mut wire_verts: Option<ResMut<crate::render::wireframe_overlay::WireframeVertices>>,
) {
    // Clear previous frame's data.
    if let Some(ref mut verts) = wire_verts {
        verts.positions.clear();
    }

    if !state.enabled || voxel_world.nodes.is_empty() {
        return;
    }
    let cam_pos = camera_q.iter().next().map_or(Vec3::ZERO, |t| t.translation);
    let target = state.display_depth.min(voxel_world.max_depth);
    let root_extent = (1u32 << voxel_world.max_depth) as f32;

    let mut cubes = Vec::new();
    collect_wireframe_cubes(
        &voxel_world,
        voxel_world.root_index as usize,
        0, Vec3::ZERO, root_extent,
        target, cam_pos, state.draw_radius, state.occupied_only,
        &mut cubes,
    );

    if cubes.is_empty() {
        return;
    }

    let Some(ref mut wire_verts) = wire_verts else { return };

    let edges: [(usize, usize); 12] = [
        (0,1),(1,2),(2,3),(3,0), // front face
        (4,5),(5,6),(6,7),(7,4), // back face
        (0,4),(1,5),(2,6),(3,7), // connectors
    ];

    wire_verts.positions.reserve(cubes.len() * 24);
    for &(center, extent) in &cubes {
        let h = extent * 0.5;
        let c = [
            center + Vec3::new(-h, -h, -h),
            center + Vec3::new( h, -h, -h),
            center + Vec3::new( h,  h, -h),
            center + Vec3::new(-h,  h, -h),
            center + Vec3::new(-h, -h,  h),
            center + Vec3::new( h, -h,  h),
            center + Vec3::new( h,  h,  h),
            center + Vec3::new(-h,  h,  h),
        ];
        for &(a, b) in &edges {
            wire_verts.positions.push(c[a].to_array());
            wire_verts.positions.push(c[b].to_array());
        }
    }
}

// ---------------------------------------------------------------------------
// SVO traversal — collects cube positions without touching Gizmos
// ---------------------------------------------------------------------------

fn collect_wireframe_cubes(
    world: &VoxelWorld,
    idx: usize,
    depth: u32,
    origin: Vec3,
    extent: f32,
    target: u32,
    cam: Vec3,
    draw_radius: f32,
    occ_only: bool,
    out: &mut Vec<(Vec3, f32)>,
) {
    if idx >= world.nodes.len() {
        return;
    }
    let center = origin + Vec3::splat(extent * 0.5);
    // Distance cull — include half-diagonal so large nodes partially in range show
    let half_diag = extent * 0.866; // sqrt(3)/2
    if (center - cam).length() > draw_radius + half_diag {
        return;
    }

    let node = &world.nodes[idx];

    // Draw at target depth or at leaf nodes (whichever comes first)
    if depth >= target || node.is_leaf() {
        if occ_only && node.color_data == 0 && node.child_mask() == 0 {
            return;
        }
        out.push((center, extent));
        return;
    }

    // Recurse into occupied children
    let half = extent * 0.5;
    let mask = node.child_mask();
    let first = node.first_child_index();
    for slot in 0..8usize {
        if mask & (1 << slot) == 0 {
            continue;
        }
        let child_origin = origin + Vec3::new(
            if slot & 1 != 0 { half } else { 0.0 },
            if slot & 2 != 0 { half } else { 0.0 },
            if slot & 4 != 0 { half } else { 0.0 },
        );
        collect_wireframe_cubes(
            world, first + slot, depth + 1,
            child_origin, half,
            target, cam, draw_radius, occ_only, out,
        );
    }
}

// ---------------------------------------------------------------------------
// Dioxus UI component
// ---------------------------------------------------------------------------

#[component]
pub fn DebugPanel() -> Element {
    let mut enabled = use_signal(|| WIREFRAME_ENABLED.load(Ordering::Relaxed));
    let mut depth = use_signal(|| WIREFRAME_DEPTH.load(Ordering::Relaxed));
    let mut occupied = use_signal(|| WIREFRAME_OCCUPIED.load(Ordering::Relaxed));

    rsx! {
        GlassPanel { title: "Debug Settings".to_string(),
            div { class: "space-y-3 text-xs",
                // Wireframe toggle
                label { class: "flex items-center gap-2 cursor-pointer text-white/70 hover:text-white",
                    input {
                        r#type: "checkbox",
                        checked: *enabled.read(),
                        onclick: move |_| {
                            let v = !*enabled.read();
                            enabled.set(v);
                            WIREFRAME_ENABLED.store(v, Ordering::Relaxed);
                        }
                    },
                    "SVO Wireframe"
                }

                // Depth slider
                div { class: "text-white/70",
                    div { class: "flex justify-between",
                        span { "Octree Depth" }
                        span { class: "font-mono", "{depth}" }
                    }
                    input {
                        r#type: "range",
                        min: "0",
                        max: "8",
                        value: "{depth}",
                        class: "w-full accent-green-400",
                        oninput: move |evt| {
                            if let Ok(v) = evt.value().parse::<u32>() {
                                depth.set(v);
                                WIREFRAME_DEPTH.store(v, Ordering::Relaxed);
                            }
                        }
                    }
                }

                // Occupied-only filter
                label { class: "flex items-center gap-2 cursor-pointer text-white/70 hover:text-white",
                    input {
                        r#type: "checkbox",
                        checked: *occupied.read(),
                        onclick: move |_| {
                            let v = !*occupied.read();
                            occupied.set(v);
                            WIREFRAME_OCCUPIED.store(v, Ordering::Relaxed);
                        }
                    },
                    "Occupied Only"
                }

                // Keybinding hints
                div { class: "mt-2 pt-2 border-t border-white/10 text-white/30 text-[10px]",
                    "F3 toggle · F4/F5 depth · F6 filter"
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::svo::VoxelMaterial;

    #[test]
    fn default_state_values() {
        let s = DebugOverlayState::default();
        assert!(!s.enabled);
        assert_eq!(s.display_depth, 4);
        assert!(s.occupied_only);
        assert_eq!(s.draw_radius, 60.0);
    }

    #[test]
    fn shared_enabled_roundtrip() {
        WIREFRAME_ENABLED.store(true, Ordering::Relaxed);
        assert!(WIREFRAME_ENABLED.load(Ordering::Relaxed));
        WIREFRAME_ENABLED.store(false, Ordering::Relaxed);
        assert!(!WIREFRAME_ENABLED.load(Ordering::Relaxed));
    }

    #[test]
    fn shared_depth_roundtrip() {
        WIREFRAME_DEPTH.store(7, Ordering::Relaxed);
        assert_eq!(WIREFRAME_DEPTH.load(Ordering::Relaxed), 7);
        WIREFRAME_DEPTH.store(4, Ordering::Relaxed);
    }

    #[test]
    fn wire_cube_corner_geometry() {
        let center = Vec3::new(10.0, 20.0, 30.0);
        let size = 4.0;
        let h = size * 0.5;
        assert_eq!(center - Vec3::splat(h), Vec3::new(8.0, 18.0, 28.0));
        assert_eq!(center + Vec3::splat(h), Vec3::new(12.0, 22.0, 32.0));
    }

    #[test]
    fn empty_world_root_is_leaf() {
        let world = VoxelWorld::new(4);
        assert_eq!(world.nodes.len(), 1);
        assert!(world.nodes[0].is_leaf());
        assert_eq!(world.nodes[0].color_data, 0);
    }

    #[test]
    fn occupied_world_has_children() {
        let mut world = VoxelWorld::new(4);
        world.set_voxel(IVec3::new(0, 0, 0), VoxelMaterial::new(255, 0, 0, 10));
        assert!(!world.nodes[0].is_leaf());
        assert!(world.nodes[0].child_mask() != 0);
    }

    #[test]
    fn collect_finds_occupied_voxel() {
        let mut world = VoxelWorld::new(4);
        world.set_voxel(IVec3::new(0, 0, 0), VoxelMaterial::new(255, 0, 0, 10));

        let mut cubes = Vec::new();
        collect_wireframe_cubes(
            &world, 0, 0, Vec3::ZERO, 16.0,
            4, Vec3::new(8.0, 8.0, 8.0), 100.0, true, &mut cubes,
        );
        assert_eq!(cubes.len(), 1, "expected 1 occupied leaf cube, got {}", cubes.len());
    }

    #[test]
    fn collect_respects_depth_limit() {
        let mut world = VoxelWorld::new(4);
        world.set_voxel(IVec3::new(0, 0, 0), VoxelMaterial::new(255, 0, 0, 10));

        // Depth 0 = just the root (which has children, so non-empty → shown)
        let mut cubes = Vec::new();
        collect_wireframe_cubes(
            &world, 0, 0, Vec3::ZERO, 16.0,
            0, Vec3::new(8.0, 8.0, 8.0), 100.0, false, &mut cubes,
        );
        assert_eq!(cubes.len(), 1, "depth 0 should yield 1 cube (root)");
    }

    #[test]
    fn collect_culls_distant_nodes() {
        let mut world = VoxelWorld::new(4);
        world.set_voxel(IVec3::new(0, 0, 0), VoxelMaterial::new(255, 0, 0, 10));

        let mut cubes = Vec::new();
        collect_wireframe_cubes(
            &world, 0, 0, Vec3::ZERO, 16.0,
            4, Vec3::new(1000.0, 1000.0, 1000.0), 1.0, true, &mut cubes,
        );
        assert_eq!(cubes.len(), 0, "distant camera should cull all nodes");
    }

    #[test]
    fn collect_occ_only_skips_empty() {
        let world = VoxelWorld::new(4);
        // Empty world, occupied_only = true → nothing collected
        let mut cubes = Vec::new();
        collect_wireframe_cubes(
            &world, 0, 0, Vec3::ZERO, 16.0,
            4, Vec3::new(8.0, 8.0, 8.0), 100.0, true, &mut cubes,
        );
        assert_eq!(cubes.len(), 0, "empty world with occ_only should yield 0");
    }

    #[test]
    fn collect_all_shows_empty_root() {
        let world = VoxelWorld::new(4);
        // Empty world, occupied_only = false → root shown
        let mut cubes = Vec::new();
        collect_wireframe_cubes(
            &world, 0, 0, Vec3::ZERO, 16.0,
            0, Vec3::new(8.0, 8.0, 8.0), 100.0, false, &mut cubes,
        );
        assert_eq!(cubes.len(), 1, "empty root with occ_only=false at depth 0 should show");
    }
}
