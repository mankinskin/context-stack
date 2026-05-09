//! Debug overlay — SVO wireframe visualization and debug panel.
//!
//! Provides a [`DebugOverlayPlugin`] that draws octree cell wireframes via
//! spawned line-mesh entities, and a [`DebugPanel`] Dioxus component for
//! interactive control.
//!
//! Communication between Dioxus (HTML thread) and Bevy (ECS) uses lock-free
//! atomics: the UI writes settings, and the Bevy system reads them each frame.

use bevy::prelude::*;
use dioxus::prelude::*;
use std::{
    sync::atomic::{
        AtomicBool,
        AtomicI32,
        AtomicU32,
        AtomicU64,
        Ordering,
    },
    time::Duration,
};

use crate::{
    character::CharacterController,
    svo::VoxelWorld,
    ui::{
        GlassPanel,
        TreeSection,
    },
    world_gen::WorldGenerator,
};

// Re-alias Bevy's KeyCode to avoid ambiguity with dioxus::prelude::KeyCode.
use bevy::input::keyboard::KeyCode as BevyKeyCode;

// ---------------------------------------------------------------------------
// Shared atomic state (Dioxus ↔ Bevy)
// ---------------------------------------------------------------------------

// --- Rendering toggles (Dioxus writes, Bevy reads) ---
static WIREFRAME_ENABLED: AtomicBool = AtomicBool::new(false);
static WIREFRAME_DEPTH: AtomicU32 = AtomicU32::new(4);
static WIREFRAME_OCCUPIED: AtomicBool = AtomicBool::new(true);
/// Smooth-min neighbor blending for SDF seams (Phase 2a).
static NEIGHBOR_BLEND_ENABLED: AtomicBool = AtomicBool::new(false);
/// Shadow rays — primary hits cast a shadow ray toward the light (Phase 2b).
static SHADOW_RAYS_ENABLED: AtomicBool = AtomicBool::new(true);
/// Reflection rays — metallic surfaces cast a secondary traversal ray (Phase 2b).
static REFLECTION_RAYS_ENABLED: AtomicBool = AtomicBool::new(true);

// --- Performance counters (Bevy writes, Dioxus reads) ---
/// Frame time in microseconds (written by Bevy system, read by Dioxus UI).
static FRAME_TIME_US: AtomicU64 = AtomicU64::new(0);
/// CPU logical thread count (set once at startup via `init_platform_info`).
static CPU_THREADS: AtomicU32 = AtomicU32::new(0);
/// Estimated GPU VRAM usage in MiB (set once at startup).
static VRAM_ESTIMATE_MB: AtomicU32 = AtomicU32::new(0);

// --- Camera state (Bevy writes, Dioxus reads) ---
/// World position stored as signed integer tenths (value × 10) for i32 precision.
static CAMERA_X_DM: AtomicI32 = AtomicI32::new(0);
static CAMERA_Y_DM: AtomicI32 = AtomicI32::new(0);
static CAMERA_Z_DM: AtomicI32 = AtomicI32::new(0);
/// Euler orientation in tenths of a degree.
static CAMERA_YAW_DEG10: AtomicI32 = AtomicI32::new(0);
static CAMERA_PITCH_DEG10: AtomicI32 = AtomicI32::new(0);

// --- SVO introspection (Bevy writes, Dioxus reads) ---
static SVO_NODE_COUNT: AtomicU64 = AtomicU64::new(0);
static SVO_DIRTY_RANGES: AtomicU32 = AtomicU32::new(0);
static SVO_MAX_DEPTH: AtomicU32 = AtomicU32::new(0);

// --- Camera mode (Dioxus writes, Bevy reads) ---
/// When `true`, physics-based character movement is replaced by unconstrained flight.
static FREE_FLY_ENABLED: AtomicBool = AtomicBool::new(false);

// --- World selection (Dioxus writes, Bevy reads / tracks) ---
/// Active world preset: 0 = Terrain, 1 = Flat, 2 = Caves, 3 = Empty.
static WORLD_PRESET: AtomicU32 = AtomicU32::new(0);
/// Active world seed — high and low 32-bit halves (Bevy writes, Dioxus reads).
static WORLD_SEED_HI: AtomicU32 = AtomicU32::new(0xDEAD_BEEF);
static WORLD_SEED_LO: AtomicU32 = AtomicU32::new(0xCAFE_BABE);

// --- LOD Cutoff (Phase 4b) — Dioxus writes, Bevy reads ---
/// LOD enabled toggle (bit 3 in feature_flags).
static LOD_ENABLED: AtomicBool = AtomicBool::new(true);
/// LOD threshold in pixels ×1000 (default 16 000 = 16.0 px).
static LOD_THRESHOLD_X1000: AtomicU32 = AtomicU32::new(16_000);
/// LOD soft-band half-width in pixels ×1000 (default 1 000 = 1.0 px).
static LOD_SOFTNESS_X1000: AtomicU32 = AtomicU32::new(1_000);

/// Returns the Phase 2a–4b feature flag bitmask for use in `RayMarchUniforms`.
///
/// Bit layout:
/// - Bit 0 (0x1): neighbor blend (Phase 2a smooth-min seam removal)
/// - Bit 1 (0x2): shadow rays (Phase 2b)
/// - Bit 2 (0x4): reflection rays (Phase 2b)
/// - Bit 3 (0x8): LOD cutoff (Phase 4b)
pub fn ray_march_feature_flags() -> u32 {
    let mut flags = 0u32;
    if NEIGHBOR_BLEND_ENABLED.load(Ordering::Relaxed) {
        flags |= 0x1;
    }
    if SHADOW_RAYS_ENABLED.load(Ordering::Relaxed) {
        flags |= 0x2;
    }
    if REFLECTION_RAYS_ENABLED.load(Ordering::Relaxed) {
        flags |= 0x4;
    }
    if LOD_ENABLED.load(Ordering::Relaxed) {
        flags |= 0x8;
    }
    flags
}

/// Returns the LOD screen-space size threshold in pixels (Phase 4b).
pub fn lod_threshold() -> f32 {
    LOD_THRESHOLD_X1000.load(Ordering::Relaxed) as f32 / 1000.0
}

/// Returns the LOD soft-band half-width in pixels (Phase 4b).
pub fn lod_softness() -> f32 {
    LOD_SOFTNESS_X1000.load(Ordering::Relaxed) as f32 / 1000.0
}

/// Returns `true` if free-fly camera mode is active.
///
/// Read by [`crate::character`] each frame to bypass physics-based movement.
pub fn is_free_fly_enabled() -> bool {
    FREE_FLY_ENABLED.load(Ordering::Relaxed)
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct DebugOverlayPlugin;

impl Plugin for DebugOverlayPlugin {
    fn build(
        &self,
        app: &mut App,
    ) {
        app.init_resource::<DebugOverlayState>();
        app.init_resource::<AppliedWorldPreset>();
        app.add_systems(Startup, init_platform_info);
        app.add_systems(
            Update,
            (
                toggle_debug_keys,
                sync_debug_from_shared,
                draw_svo_wireframe,
                track_frame_time,
            )
                .chain(),
        );
        app.add_systems(Update, (track_camera_state, track_svo_state));
        app.add_systems(Update, apply_world_preset);
    }
}

// ---------------------------------------------------------------------------
// Bevy resource
// ---------------------------------------------------------------------------

#[derive(Resource)]
pub struct DebugOverlayState {
    pub enabled: bool,
    pub display_depth: u32,
    pub wire_color: Color,
    pub occupied_only: bool,
}

impl Default for DebugOverlayState {
    fn default() -> Self {
        Self {
            enabled: false,
            display_depth: 4,
            wire_color: Color::srgb(0.0, 1.0, 0.0),
            occupied_only: true,
        }
    }
}

/// Tracks which world preset was last applied, to detect changes from the atomic.
/// Initialised to `u32::MAX` so the first Update frame always applies preset 0.
#[derive(Resource)]
struct AppliedWorldPreset(u32);

impl Default for AppliedWorldPreset {
    fn default() -> Self {
        Self(u32::MAX)
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
        WIREFRAME_DEPTH.store(v.min(11) + 1, Ordering::Relaxed);
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

/// Write frame dt (microseconds) into the shared atomic for UI display.
fn track_frame_time(time: Res<Time>) {
    let dt_us = (time.delta_secs_f64() * 1_000_000.0) as u64;
    FRAME_TIME_US.store(dt_us, Ordering::Relaxed);
}

/// Startup system: detect CPU parallelism and compute a static VRAM estimate.
///
/// VRAM breakdown (all figures are designed max sizes, not live allocations):
/// - SVO double-buffer:  4 M nodes × 8 B × 2 = 64 MiB
/// - Gaussian buffer:    1 M splats × 232 B  = 221 MiB
/// - Projected buffer:   1 M splats × 40 B   =  38 MiB
/// - Tile data + misc:                        ~  16 MiB
fn init_platform_info() {
    #[cfg(target_arch = "wasm32")]
    {
        let threads = web_sys::window()
            .map(|w| w.navigator().hardware_concurrency() as u32)
            .unwrap_or(1);
        CPU_THREADS.store(threads, Ordering::Relaxed);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let threads = std::thread::available_parallelism()
            .map(|n| n.get() as u32)
            .unwrap_or(1);
        CPU_THREADS.store(threads, Ordering::Relaxed);
    }
    VRAM_ESTIMATE_MB.store(64 + 221 + 38 + 16, Ordering::Relaxed);
}

/// Write camera world position and orientation into shared atomics each frame.
fn track_camera_state(
    camera_q: Query<(&Transform, Option<&CharacterController>), With<Camera3d>>
) {
    let Ok((tf, ctrl)) = camera_q.single() else {
        return;
    };
    let pos = tf.translation;
    CAMERA_X_DM.store((pos.x * 10.0) as i32, Ordering::Relaxed);
    CAMERA_Y_DM.store((pos.y * 10.0) as i32, Ordering::Relaxed);
    CAMERA_Z_DM.store((pos.z * 10.0) as i32, Ordering::Relaxed);
    if let Some(c) = ctrl {
        CAMERA_YAW_DEG10
            .store((c.yaw.to_degrees() * 10.0) as i32, Ordering::Relaxed);
        CAMERA_PITCH_DEG10
            .store((c.pitch.to_degrees() * 10.0) as i32, Ordering::Relaxed);
    }
}

/// Write SVO structural metrics and world seed into shared atomics each frame.
fn track_svo_state(
    voxel_world: Res<VoxelWorld>,
    generator: Option<Res<WorldGenerator>>,
) {
    SVO_NODE_COUNT.store(voxel_world.nodes.len() as u64, Ordering::Relaxed);
    SVO_DIRTY_RANGES
        .store(voxel_world.dirty_ranges.len() as u32, Ordering::Relaxed);
    SVO_MAX_DEPTH.store(voxel_world.max_depth, Ordering::Relaxed);
    if let Some(gen) = generator {
        let seed = gen.seed;
        WORLD_SEED_HI.store((seed >> 32) as u32, Ordering::Relaxed);
        WORLD_SEED_LO.store(seed as u32, Ordering::Relaxed);
    }
}

/// Detect world-preset changes and rebuild the `VoxelWorld` via the
/// sandbox-registered preset functions.
fn apply_world_preset(world: &mut World) {
    let preset = WORLD_PRESET.load(Ordering::Relaxed);
    let applied = world.resource::<AppliedWorldPreset>().0;
    if preset == applied {
        return;
    }
    world.resource_mut::<AppliedWorldPreset>().0 = preset;
    crate::apply_registered_preset(preset, world);
}

/// Walk the SVO and populate the wireframe overlay vertex buffer.
///
/// Each frame we recompute the line positions and write them to the
/// [`WireframeData`] resource, which the GPU overlay node uploads and draws
/// on top of the voxel splats.
fn draw_svo_wireframe(
    state: Res<DebugOverlayState>,
    voxel_world: Res<VoxelWorld>,
    mut wire_data: Option<
        ResMut<crate::render::wireframe_overlay::WireframeData>,
    >,
    camera_q: Query<&Transform, With<Camera3d>>,
) {
    // Clear previous frame's data.
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

    // -------------------------------------------------------------------------
    // Occupied-only mode: nearest-first priority queue.
    //
    // At fine depths (depth 10 → 1-unit cells) the world surface contains far
    // more occupied cells than the vertex budget.  A plain BFS distributes the
    // budget evenly over the whole world, producing cells too sparse/small to
    // see from a normal camera distance.  Using a min-heap ordered by
    // center-to-camera distance ensures the budget is consumed by the nearest
    // occupied terrain first, giving a dense, visible grid around the camera.
    // Cell positions are world-fixed (SVO nodes never move), so proximity-based
    // *selection* does not produce the camera-following artefact that plagued
    // the old draw-radius culling approach.
    // -------------------------------------------------------------------------
    let mut cubes: Vec<(Vec3, f32)> = Vec::new();

    if occ_only {
        // Occupied-only: BFS that follows only non-empty SVO branches.
        // We collect ALL occupied nodes up to `target` depth with no in-loop
        // budget cutoff (correct traversal), then sort by camera proximity and
        // truncate to MAX_CUBES so that the nearest terrain is always visible.
        // At typical UI depths (≤8) the terrain surface has far fewer than
        // MAX_CUBES cells, so the sort/truncate is effectively a no-op.
        let cam_pos = camera_q
            .single()
            .map(|t| t.translation)
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

            // Skip truly empty leaves (no color, no children).
            if node.color_data == 0 && node.child_mask() == 0 {
                continue;
            }

            if depth >= target || node.is_leaf() {
                cubes.push((center, extent));
                continue;
            }

            // Recurse into occupied children only.
            let mask = node.child_mask();
            let first = node.first_child_index();
            for slot in 0..8usize {
                if mask & (1 << slot) == 0 {
                    continue;
                }
                let co = origin
                    + Vec3::new(
                        if slot & 1 != 0 { half } else { 0.0 },
                        if slot & 2 != 0 { half } else { 0.0 },
                        if slot & 4 != 0 { half } else { 0.0 },
                    );
                // SVO stores children at first_child + slot (fixed 8-slot block).
                queue.push_back((first + slot, depth + 1, co, half));
            }
        }

        // When over budget, keep only the nearest cells to the camera.
        if cubes.len() > MAX_CUBES {
            cubes.sort_unstable_by(|(a, _), (b, _)| {
                a.distance_squared(cam_pos)
                    .partial_cmp(&b.distance_squared(cam_pos))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            cubes.truncate(MAX_CUBES);
        }
    } else {
        // -----------------------------------------------------------------------
        // Full-grid mode: level-order BFS (VecDeque).
        //
        // Processing all nodes at depth D before any at depth D+1 ensures the
        // budget cutoff is spread evenly across all world regions.
        // -----------------------------------------------------------------------
        // In full-grid mode, empty octants are emitted as virtual cells.
        // To prevent the BFS queue from growing exponentially (8^depth entries)
        // cap empty-region recursion at the deepest D where 8^D ≤ MAX_CUBES.
        let full_grid_cap: u32 = {
            let mut d = 0u32;
            let mut cells = 1usize;
            while cells.saturating_mul(8) <= MAX_CUBES {
                d += 1;
                cells = cells.saturating_mul(8);
            }
            d // 8^5=32768 ≤ 87381 < 8^6=262144 → cap=5
        };

        // Queue entry: (node_idx, depth, origin, extent).
        // node_idx == usize::MAX is a sentinel for an "empty region".
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
                // Empty-region sentinel.
                let empty_target = target.min(full_grid_cap);
                if depth >= empty_target {
                    cubes.push((center, extent));
                } else {
                    for slot in 0..8usize {
                        let co = origin
                            + Vec3::new(
                                if slot & 1 != 0 { half } else { 0.0 },
                                if slot & 2 != 0 { half } else { 0.0 },
                                if slot & 4 != 0 { half } else { 0.0 },
                            );
                        queue.push_back((usize::MAX, depth + 1, co, half));
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
                let co = origin
                    + Vec3::new(
                        if slot & 1 != 0 { half } else { 0.0 },
                        if slot & 2 != 0 { half } else { 0.0 },
                        if slot & 4 != 0 { half } else { 0.0 },
                    );
                if mask & (1 << slot) == 0 {
                    // Empty octant: enqueue as virtual cell within depth cap.
                    if depth + 1 <= full_grid_cap {
                        queue.push_back((usize::MAX, depth + 1, co, half));
                    }
                } else {
                    // SVO stores children at first_child + slot (fixed 8-slot block).
                    queue.push_back((first + slot, depth + 1, co, half));
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
        (3, 0), // front face
        (4, 5),
        (5, 6),
        (6, 7),
        (7, 4), // back face
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7), // connectors
    ];

    wire_data
        .corners
        .reserve(cubes.len() * WIREFRAME_VERTS_PER_CUBE);
    wire_data
        .indices
        .reserve(cubes.len() * WIREFRAME_INDICES_PER_CUBE);
    for (i, &(center, extent)) in cubes.iter().enumerate() {
        let base = (i * WIREFRAME_VERTS_PER_CUBE) as u32;
        let h = extent * 0.5;
        wire_data
            .corners
            .push((center + Vec3::new(-h, -h, -h)).to_array());
        wire_data
            .corners
            .push((center + Vec3::new(h, -h, -h)).to_array());
        wire_data
            .corners
            .push((center + Vec3::new(h, h, -h)).to_array());
        wire_data
            .corners
            .push((center + Vec3::new(-h, h, -h)).to_array());
        wire_data
            .corners
            .push((center + Vec3::new(-h, -h, h)).to_array());
        wire_data
            .corners
            .push((center + Vec3::new(h, -h, h)).to_array());
        wire_data
            .corners
            .push((center + Vec3::new(h, h, h)).to_array());
        wire_data
            .corners
            .push((center + Vec3::new(-h, h, h)).to_array());
        for &(a, b) in &CUBE_EDGES {
            wire_data.indices.push(base + a);
            wire_data.indices.push(base + b);
        }
    }
}

// ---------------------------------------------------------------------------
// Dioxus UI component
// ---------------------------------------------------------------------------

#[component]
pub fn DebugPanel() -> Element {
    // Auto-incremented every 500 ms to re-read all display atomics.
    let mut tick = use_signal(|| 0u32);
    let _ = *tick.read();

    use_future(move || async move {
        loop {
            futures_timer::Delay::new(Duration::from_millis(500)).await;
            tick.set(tick() + 1);
        }
    });

    // --- Performance ---
    let frame_us = FRAME_TIME_US.load(Ordering::Relaxed);
    let fps = if frame_us == 0 {
        0.0
    } else {
        1_000_000.0 / frame_us as f64
    };
    let ms = frame_us as f64 / 1000.0;
    let cpu_threads = CPU_THREADS.load(Ordering::Relaxed);
    let vram_mb = VRAM_ESTIMATE_MB.load(Ordering::Relaxed);

    // --- Camera (display values) ---
    let cam_x = CAMERA_X_DM.load(Ordering::Relaxed) as f32 / 10.0;
    let cam_y = CAMERA_Y_DM.load(Ordering::Relaxed) as f32 / 10.0;
    let cam_z = CAMERA_Z_DM.load(Ordering::Relaxed) as f32 / 10.0;
    let yaw_deg = CAMERA_YAW_DEG10.load(Ordering::Relaxed) as f32 / 10.0;
    let pitch_deg = CAMERA_PITCH_DEG10.load(Ordering::Relaxed) as f32 / 10.0;

    // --- SVO (display values) ---
    let node_count = SVO_NODE_COUNT.load(Ordering::Relaxed);
    let dirty = SVO_DIRTY_RANGES.load(Ordering::Relaxed);
    let max_depth = SVO_MAX_DEPTH.load(Ordering::Relaxed);
    let resolution = if max_depth > 0 { 1u64 << max_depth } else { 0 };
    let capacity = crate::gpu::SVO_CAPACITY_NODES as u64;

    // --- World seed ---
    let seed_hi = WORLD_SEED_HI.load(Ordering::Relaxed);
    let seed_lo = WORLD_SEED_LO.load(Ordering::Relaxed);
    let seed = ((seed_hi as u64) << 32) | (seed_lo as u64);
    let active_preset = WORLD_PRESET.load(Ordering::Relaxed);
    let preset_names = crate::world_preset_names();
    let active_preset_name = preset_names
        .get(active_preset as usize)
        .cloned()
        .unwrap_or_else(|| "?".to_string());

    // --- Mutable toggle / slider state ---
    let mut free_fly = use_signal(|| FREE_FLY_ENABLED.load(Ordering::Relaxed));
    let mut neighbor_blend =
        use_signal(|| NEIGHBOR_BLEND_ENABLED.load(Ordering::Relaxed));
    let mut shadow_rays =
        use_signal(|| SHADOW_RAYS_ENABLED.load(Ordering::Relaxed));
    let mut reflection_rays =
        use_signal(|| REFLECTION_RAYS_ENABLED.load(Ordering::Relaxed));
    let mut wireframe_en =
        use_signal(|| WIREFRAME_ENABLED.load(Ordering::Relaxed));
    let mut depth = use_signal(|| WIREFRAME_DEPTH.load(Ordering::Relaxed));
    let mut occupied =
        use_signal(|| WIREFRAME_OCCUPIED.load(Ordering::Relaxed));
    let mut lod_en = use_signal(|| LOD_ENABLED.load(Ordering::Relaxed));
    let mut lod_thresh =
        use_signal(|| LOD_THRESHOLD_X1000.load(Ordering::Relaxed));
    let mut lod_soft =
        use_signal(|| LOD_SOFTNESS_X1000.load(Ordering::Relaxed));

    rsx! {
        GlassPanel { title: "Debug".to_string(),
            div { class: "space-y-0.5 text-xs",

                // ── Performance ──────────────────────────────────────────────
                TreeSection { label: "Performance".to_string(), default_open: true,
                    div { class: "flex justify-between text-white/70",
                        span { "FPS" }
                        span { class: "font-mono text-green-400", "{fps:.0}" }
                    }
                    div { class: "flex justify-between text-white/70",
                        span { "Frame" }
                        span { class: "font-mono", "{ms:.2} ms" }
                    }
                    div { class: "flex justify-between text-white/70",
                        span { "CPU threads" }
                        span { class: "font-mono", "{cpu_threads}" }
                    }
                    div { class: "flex justify-between text-white/70",
                        span { "VRAM est." }
                        span { class: "font-mono", "{vram_mb} MiB" }
                    }
                }

                // ── Camera ───────────────────────────────────────────────────
                TreeSection { label: "Camera".to_string(), default_open: true,
                    div { class: "grid grid-cols-2 gap-x-2 text-white/70",
                        span { "X" }
                        span { class: "font-mono text-right", "{cam_x:.1}" }
                        span { "Y" }
                        span { class: "font-mono text-right", "{cam_y:.1}" }
                        span { "Z" }
                        span { class: "font-mono text-right", "{cam_z:.1}" }
                        span { "Yaw" }
                        span { class: "font-mono text-right", "{yaw_deg:.1}°" }
                        span { "Pitch" }
                        span { class: "font-mono text-right", "{pitch_deg:.1}°" }
                    }
                    label { class: "flex items-center gap-2 cursor-pointer text-white/70 hover:text-white mt-1.5",
                        input {
                            r#type: "checkbox",
                            checked: *free_fly.read(),
                            onclick: move |_| {
                                let v = !*free_fly.read();
                                free_fly.set(v);
                                FREE_FLY_ENABLED.store(v, Ordering::Relaxed);
                            }
                        }
                        "Free Fly  (Q/E = up/down)"
                    }
                }

                // ── SVO / Geometry ───────────────────────────────────────────
                TreeSection { label: "SVO / Geometry".to_string(), default_open: false,
                    div { class: "grid grid-cols-2 gap-x-2 text-white/70",
                        span { "Max Depth" }
                        span { class: "font-mono text-right", "{max_depth}" }
                        span { "Resolution" }
                        span { class: "font-mono text-right", "{resolution}³" }
                        span { "Nodes" }
                        span { class: "font-mono text-right", "{node_count} / {capacity}" }
                        span { "Dirty ranges" }
                        span { class: "font-mono text-right text-yellow-400", "{dirty}" }
                    }
                    div { class: "flex justify-between text-white/70 mt-1.5",
                        span { "Seed" }
                        span { class: "font-mono text-[10px] text-white/50", "{seed:#018X}" }
                    }
                }

                // ── Rendering ────────────────────────────────────────────────
                TreeSection { label: "Rendering".to_string(), default_open: true,
                    // Ray March sub-section
                    TreeSection { label: "Ray March".to_string(), default_open: true,
                        label { class: "flex items-center gap-2 cursor-pointer text-white/70 hover:text-white",
                            input {
                                r#type: "checkbox",
                                checked: *neighbor_blend.read(),
                                onclick: move |_| {
                                    let v = !*neighbor_blend.read();
                                    neighbor_blend.set(v);
                                    NEIGHBOR_BLEND_ENABLED.store(v, Ordering::Relaxed);
                                }
                            }
                            "Neighbor Blend"
                        }
                        label { class: "flex items-center gap-2 cursor-pointer text-white/70 hover:text-white",
                            input {
                                r#type: "checkbox",
                                checked: *shadow_rays.read(),
                                onclick: move |_| {
                                    let v = !*shadow_rays.read();
                                    shadow_rays.set(v);
                                    SHADOW_RAYS_ENABLED.store(v, Ordering::Relaxed);
                                }
                            }
                            "Shadow Rays"
                        }
                        label { class: "flex items-center gap-2 cursor-pointer text-white/70 hover:text-white",
                            input {
                                r#type: "checkbox",
                                checked: *reflection_rays.read(),
                                onclick: move |_| {
                                    let v = !*reflection_rays.read();
                                    reflection_rays.set(v);
                                    REFLECTION_RAYS_ENABLED.store(v, Ordering::Relaxed);
                                }
                            }
                            "Reflections"
                        }
                        // ── LOD Cutoff (Phase 4b) ─────────────────────────
                        TreeSection { label: "LOD Cutoff".to_string(), default_open: true,
                            label { class: "flex items-center gap-2 cursor-pointer text-white/70 hover:text-white",
                                input {
                                    r#type: "checkbox",
                                    checked: *lod_en.read(),
                                    onclick: move |_| {
                                        let v = !*lod_en.read();
                                        lod_en.set(v);
                                        LOD_ENABLED.store(v, Ordering::Relaxed);
                                    }
                                }
                                "Enable LOD"
                            }
                            div { class: "text-white/70 mt-1",
                                div { class: "flex justify-between",
                                    span { "Threshold (px)" }
                                    span { class: "font-mono", "{*lod_thresh.read() as f32 / 1000.0:.1}" }
                                }
                                input {
                                    r#type: "range",
                                    min: "500",
                                    max: "64000",
                                    step: "500",
                                    value: "{lod_thresh}",
                                    class: "w-full accent-yellow-400",
                                    oninput: move |evt| {
                                        if let Ok(v) = evt.value().parse::<u32>() {
                                            lod_thresh.set(v);
                                            LOD_THRESHOLD_X1000.store(v, Ordering::Relaxed);
                                        }
                                    }
                                }
                            }
                            div { class: "text-white/70 mt-1",
                                div { class: "flex justify-between",
                                    span { "Softness (px)" }
                                    span { class: "font-mono", "{*lod_soft.read() as f32 / 1000.0:.1}" }
                                }
                                input {
                                    r#type: "range",
                                    min: "100",
                                    max: "8000",
                                    step: "100",
                                    value: "{lod_soft}",
                                    class: "w-full accent-yellow-400",
                                    oninput: move |evt| {
                                        if let Ok(v) = evt.value().parse::<u32>() {
                                            lod_soft.set(v);
                                            LOD_SOFTNESS_X1000.store(v, Ordering::Relaxed);
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Wireframe sub-section
                    TreeSection { label: "Wireframe".to_string(), default_open: false,
                        label { class: "flex items-center gap-2 cursor-pointer text-white/70 hover:text-white",
                            input {
                                r#type: "checkbox",
                                checked: *wireframe_en.read(),
                                onclick: move |_| {
                                    let v = !*wireframe_en.read();
                                    wireframe_en.set(v);
                                    WIREFRAME_ENABLED.store(v, Ordering::Relaxed);
                                }
                            }
                            "Enable  (F3)"
                        }
                        div { class: "text-white/70",
                            div { class: "flex justify-between",
                                span { "Depth  (F4 / F5)" }
                                span { class: "font-mono", "{depth}" }
                            }
                            input {
                                r#type: "range",
                                min: "0",
                                max: "11",
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
                        label { class: "flex items-center gap-2 cursor-pointer text-white/70 hover:text-white",
                            input {
                                r#type: "checkbox",
                                checked: *occupied.read(),
                                onclick: move |_| {
                                    let v = !*occupied.read();
                                    occupied.set(v);
                                    WIREFRAME_OCCUPIED.store(v, Ordering::Relaxed);
                                }
                            }
                            "Occupied Only  (F6)"
                        }
                    }
                }

                // ── World ────────────────────────────────────────────────────
                TreeSection { label: "World".to_string(), default_open: false,
                    div { class: "flex justify-between text-white/70 mb-1.5",
                        span { "Active" }
                        span { class: "font-mono text-cyan-400", "{active_preset_name}" }
                    }
                    div { class: "grid grid-cols-2 gap-1",
                        for (i, name) in preset_names.iter().enumerate() {
                            {
                                let idx = i as u32;
                                let name = name.clone();
                                let cls = if idx == active_preset {
                                    "px-2 py-0.5 text-[10px] rounded bg-cyan-500/40 border border-cyan-400 text-white"
                                } else {
                                    "px-2 py-0.5 text-[10px] rounded bg-white/10 hover:bg-white/20 border border-white/20 text-white/60"
                                };
                                rsx! {
                                    button {
                                        class: "{cls}",
                                        onclick: move |_| { WORLD_PRESET.store(idx, Ordering::Relaxed); },
                                        "{name}"
                                    }
                                }
                            }
                        }
                    }
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

        // BFS occupied-only at max depth should find the single voxel.
        let mut cubes: Vec<(Vec3, f32)> = Vec::new();
        let root_extent = (1u32 << world.max_depth) as f32;
        let mut queue: std::collections::VecDeque<(usize, u32, Vec3, f32)> =
            std::collections::VecDeque::new();
        queue.push_back((
            world.root_index as usize,
            0,
            Vec3::ZERO,
            root_extent,
        ));
        while let Some((idx, depth, origin, extent)) = queue.pop_front() {
            if idx >= world.nodes.len() {
                continue;
            }
            let node = &world.nodes[idx];
            let half = extent * 0.5;
            let center = origin + Vec3::splat(half);
            if node.color_data == 0 && node.child_mask() == 0 {
                continue;
            }
            if depth >= world.max_depth || node.is_leaf() {
                cubes.push((center, extent));
                continue;
            }
            let mask = node.child_mask();
            let first = node.first_child_index();
            for slot in 0..8usize {
                if mask & (1 << slot) == 0 {
                    continue;
                }
                let co = origin
                    + Vec3::new(
                        if slot & 1 != 0 { half } else { 0.0 },
                        if slot & 2 != 0 { half } else { 0.0 },
                        if slot & 4 != 0 { half } else { 0.0 },
                    );
                let off = (mask & ((1 << slot) - 1)).count_ones() as usize;
                queue.push_back((first + off, depth + 1, co, half));
            }
        }
        assert_eq!(
            cubes.len(),
            1,
            "expected 1 occupied leaf cube, got {}",
            cubes.len()
        );
    }

    #[test]
    fn collect_respects_depth_limit() {
        let mut world = VoxelWorld::new(4);
        world.set_voxel(IVec3::new(0, 0, 0), VoxelMaterial::new(255, 0, 0, 10));

        // Depth 0: root has children → non-empty → shown immediately.
        let mut cubes: Vec<(Vec3, f32)> = Vec::new();
        let root_extent = (1u32 << world.max_depth) as f32;
        let mut queue: std::collections::VecDeque<(usize, u32, Vec3, f32)> =
            std::collections::VecDeque::new();
        queue.push_back((
            world.root_index as usize,
            0,
            Vec3::ZERO,
            root_extent,
        ));
        while let Some((idx, depth, origin, extent)) = queue.pop_front() {
            if idx >= world.nodes.len() {
                continue;
            }
            let node = &world.nodes[idx];
            let half = extent * 0.5;
            let center = origin + Vec3::splat(half);
            if node.color_data == 0 && node.child_mask() == 0 {
                continue;
            }
            if depth >= 0 || node.is_leaf() {
                // target = 0
                cubes.push((center, extent));
                continue;
            }
            let mask = node.child_mask();
            let first = node.first_child_index();
            for slot in 0..8usize {
                if mask & (1 << slot) == 0 {
                    continue;
                }
                let co = origin
                    + Vec3::new(
                        if slot & 1 != 0 { half } else { 0.0 },
                        if slot & 2 != 0 { half } else { 0.0 },
                        if slot & 4 != 0 { half } else { 0.0 },
                    );
                let off = (mask & ((1 << slot) - 1)).count_ones() as usize;
                queue.push_back((first + off, depth + 1, co, half));
            }
        }
        assert_eq!(cubes.len(), 1, "depth 0 should yield 1 cube (root)");
    }

    #[test]
    fn collect_occ_only_skips_empty() {
        let world = VoxelWorld::new(4);
        // Empty world: root has color_data==0 and child_mask==0 → skipped.
        let mut cubes: Vec<(Vec3, f32)> = Vec::new();
        let root_extent = (1u32 << world.max_depth) as f32;
        let mut queue: std::collections::VecDeque<(usize, u32, Vec3, f32)> =
            std::collections::VecDeque::new();
        queue.push_back((
            world.root_index as usize,
            0,
            Vec3::ZERO,
            root_extent,
        ));
        while let Some((idx, depth, origin, extent)) = queue.pop_front() {
            if idx >= world.nodes.len() {
                continue;
            }
            let node = &world.nodes[idx];
            let half = extent * 0.5;
            let center = origin + Vec3::splat(half);
            if node.color_data == 0 && node.child_mask() == 0 {
                continue;
            }
            if depth >= world.max_depth || node.is_leaf() {
                cubes.push((center, extent));
                continue;
            }
            let mask = node.child_mask();
            let first = node.first_child_index();
            for slot in 0..8usize {
                if mask & (1 << slot) == 0 {
                    continue;
                }
                let co = origin
                    + Vec3::new(
                        if slot & 1 != 0 { half } else { 0.0 },
                        if slot & 2 != 0 { half } else { 0.0 },
                        if slot & 4 != 0 { half } else { 0.0 },
                    );
                let off = (mask & ((1 << slot) - 1)).count_ones() as usize;
                queue.push_back((first + off, depth + 1, co, half));
            }
        }
        assert_eq!(cubes.len(), 0, "empty world with occ_only should yield 0");
    }
}
