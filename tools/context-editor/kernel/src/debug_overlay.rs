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
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicU64, Ordering};
use std::time::Duration;

use crate::character::CharacterController;
use crate::svo::VoxelWorld;
use crate::ui::{GlassPanel, TreeSection};
use crate::world_gen::WorldGenerator;

// Re-alias Bevy's KeyCode to avoid ambiguity with dioxus::prelude::KeyCode.
use bevy::input::keyboard::KeyCode as BevyKeyCode;

// ---------------------------------------------------------------------------
// Shared atomic state (Dioxus ↔ Bevy)
// ---------------------------------------------------------------------------

// --- Rendering toggles (Dioxus writes, Bevy reads) ---
static WIREFRAME_ENABLED: AtomicBool = AtomicBool::new(false);
static WIREFRAME_DEPTH: AtomicU32 = AtomicU32::new(4);
static WIREFRAME_OCCUPIED: AtomicBool = AtomicBool::new(true);
/// Z-prepass toggle — OFF by default to avoid redundant per-pixel SDF work.
static ZPREPASS_ENABLED: AtomicBool = AtomicBool::new(false);
/// Ray-march toggle — when ON, `SvoRayMarchNode` renders instead of tiled pipeline.
static RAY_MARCH_ENABLED: AtomicBool = AtomicBool::new(true);
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

/// Returns `true` if the z-prepass is enabled (read by `ZPrepassNode`).
pub fn is_zprepass_enabled() -> bool {
    ZPREPASS_ENABLED.load(Ordering::Relaxed)
}

/// Returns `true` if the SVO ray march pipeline is active.
///
/// When `true`, `SvoRayMarchNode` renders the scene; the tiled forward+ nodes
/// are no-ops for that frame.
pub fn is_ray_march_enabled() -> bool {
    RAY_MARCH_ENABLED.load(Ordering::Relaxed)
}

/// Returns the Phase 2a/2b feature flag bitmask for use in `RayMarchUniforms`.
///
/// Bit layout:
/// - Bit 0 (0x1): neighbor blend (Phase 2a smooth-min seam removal)
/// - Bit 1 (0x2): shadow rays (Phase 2b)
/// - Bit 2 (0x4): reflection rays (Phase 2b)
pub fn ray_march_feature_flags() -> u32 {
    let mut flags = 0u32;
    if NEIGHBOR_BLEND_ENABLED.load(Ordering::Relaxed) { flags |= 0x1; }
    if SHADOW_RAYS_ENABLED.load(Ordering::Relaxed)    { flags |= 0x2; }
    if REFLECTION_RAYS_ENABLED.load(Ordering::Relaxed) { flags |= 0x4; }
    flags
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
    fn build(&self, app: &mut App) {
        app.init_resource::<DebugOverlayState>();
        app.init_resource::<AppliedWorldPreset>();
        app.add_systems(Startup, init_platform_info);
        app.add_systems(Update, (
            toggle_debug_keys,
            sync_debug_from_shared,
            draw_svo_wireframe,
            track_frame_time,
        ).chain());
        app.add_systems(Update, (
            track_camera_state,
            track_svo_state,
            apply_world_preset,
        ));
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
            draw_radius: 800.0,
            wire_color: Color::srgb(0.0, 1.0, 0.0),
            occupied_only: true,
        }
    }
}

/// Tracks which world preset was last applied, to detect changes from the atomic.
#[derive(Resource, Default)]
struct AppliedWorldPreset(u32);

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
    if keys.just_pressed(BevyKeyCode::F7) {
        let v = RAY_MARCH_ENABLED.load(Ordering::Relaxed);
        RAY_MARCH_ENABLED.store(!v, Ordering::Relaxed);
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
    camera_q: Query<(&Transform, Option<&CharacterController>), With<Camera3d>>,
) {
    let Ok((tf, ctrl)) = camera_q.single() else { return };
    let pos = tf.translation;
    CAMERA_X_DM.store((pos.x * 10.0) as i32, Ordering::Relaxed);
    CAMERA_Y_DM.store((pos.y * 10.0) as i32, Ordering::Relaxed);
    CAMERA_Z_DM.store((pos.z * 10.0) as i32, Ordering::Relaxed);
    if let Some(c) = ctrl {
        CAMERA_YAW_DEG10.store((c.yaw.to_degrees() * 10.0) as i32, Ordering::Relaxed);
        CAMERA_PITCH_DEG10.store((c.pitch.to_degrees() * 10.0) as i32, Ordering::Relaxed);
    }
}

/// Write SVO structural metrics and world seed into shared atomics each frame.
fn track_svo_state(
    voxel_world: Res<VoxelWorld>,
    generator: Option<Res<WorldGenerator>>,
) {
    SVO_NODE_COUNT.store(voxel_world.nodes.len() as u64, Ordering::Relaxed);
    SVO_DIRTY_RANGES.store(voxel_world.dirty_ranges.len() as u32, Ordering::Relaxed);
    SVO_MAX_DEPTH.store(voxel_world.max_depth, Ordering::Relaxed);
    if let Some(gen) = generator {
        let seed = gen.seed;
        WORLD_SEED_HI.store((seed >> 32) as u32, Ordering::Relaxed);
        WORLD_SEED_LO.store(seed as u32, Ordering::Relaxed);
    }
}

/// Detect world-preset changes and rebuild the `VoxelWorld` accordingly.
///
/// All four presets place terrain around the bootstrap scene centre (512, 253, 512)
/// so the player sees results immediately without flying.
fn apply_world_preset(
    mut voxel_world: ResMut<VoxelWorld>,
    mut applied: ResMut<AppliedWorldPreset>,
) {
    let preset = WORLD_PRESET.load(Ordering::Relaxed);
    if preset == applied.0 {
        return;
    }
    applied.0 = preset;

    let max_depth = voxel_world.max_depth;
    *voxel_world = VoxelWorld::new(max_depth);

    let cx = 512_i32;
    let cz = 512_i32;
    let fy = 253_i32;

    match preset {
        // --- Flat: uniform grass/dirt slab, no terrain noise ---
        1 => {
// [WorldPreset] Flat
            let half = 120_i32;
            let grass = crate::svo::VoxelMaterial::new(60, 140, 40, 18);
            let dirt  = crate::svo::VoxelMaterial::new(100, 70, 40, 22);
            for x in (cx - half)..(cx + half) {
                for z in (cz - half)..(cz + half) {
                    for dy in 0..2_i32 {
                        voxel_world.set_voxel(IVec3::new(x, fy - dy, z), grass);
                    }
                    for dy in 2..6_i32 {
                        voxel_world.set_voxel(IVec3::new(x, fy - dy, z), dirt);
                    }
                }
            }
        }
        // --- Caves: sine-wave surface with cave tunnels carved through ---
        2 => {
// [WorldPreset] Caves
            let half = 96_i32;
            let stone = crate::svo::VoxelMaterial::new(128, 128, 130, 28);
            let grass = crate::svo::VoxelMaterial::new(60, 140, 40, 18);
            for x in (cx - half)..(cx + half) {
                for z in (cz - half)..(cz + half) {
                    let h_offset = ((x as f32 * 0.08).sin() * (z as f32 * 0.06).cos() * 4.0) as i32;
                    let surface_y = fy + h_offset;
                    voxel_world.set_voxel(IVec3::new(x, surface_y, z), grass);
                    for dy in 1..14_i32 {
                        let y = surface_y - dy;
                        // Tunnel void: alternating bands 4–9 voxels below surface.
                        let is_cave = ((x as f32 * 0.15).sin().abs()
                            + (z as f32 * 0.12).cos().abs()) > 1.6
                            && dy >= 4 && dy <= 9;
                        if !is_cave {
                            voxel_world.set_voxel(IVec3::new(x, y, z), stone);
                        }
                    }
                }
            }
        }
        // --- Empty: cleared world (VoxelWorld already reset above) ---
        3 => {
// [WorldPreset] Empty
        }
        // --- Terrain (0) or unknown: rolling noise hills with biome materials ---
        _ => {
// [WorldPreset] Terrain
            let half = 120_i32;
            let grass = crate::svo::VoxelMaterial::new(60, 140, 40, 18);
            let dirt  = crate::svo::VoxelMaterial::new(100, 70, 40, 22);
            let stone = crate::svo::VoxelMaterial::new(128, 128, 130, 28);
            let sand  = crate::svo::VoxelMaterial::new(210, 190, 140, 24);
            for x in (cx - half)..(cx + half) {
                for z in (cz - half)..(cz + half) {
                    let dx = (x - cx) as f32;
                    let dz = (z - cz) as f32;
                    let h = ((dx * 0.04).sin() * 3.0
                        + (dz * 0.05).cos() * 3.0
                        + ((dx * 0.015 + dz * 0.02).sin() * 5.0)) as i32;
                    let surface_y = fy + h;
                    let top_mat = if h >= 4 { stone } else if h <= -2 { sand } else { grass };
                    voxel_world.set_voxel(IVec3::new(x, surface_y, z), top_mat);
                    for dy in 1..4_i32 {
                        voxel_world.set_voxel(IVec3::new(x, surface_y - dy, z), dirt);
                    }
                    for dy in 4..12_i32 {
                        voxel_world.set_voxel(IVec3::new(x, surface_y - dy, z), stone);
                    }
                }
            }
        }
    }
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
    let fps = if frame_us == 0 { 0.0 } else { 1_000_000.0 / frame_us as f64 };
    let ms = frame_us as f64 / 1000.0;
    let cpu_threads = CPU_THREADS.load(Ordering::Relaxed);
    let vram_mb = VRAM_ESTIMATE_MB.load(Ordering::Relaxed);

    // --- Camera (display values) ---
    let cam_x = CAMERA_X_DM.load(Ordering::Relaxed) as f32 / 10.0;
    let cam_y = CAMERA_Y_DM.load(Ordering::Relaxed) as f32 / 10.0;
    let cam_z = CAMERA_Z_DM.load(Ordering::Relaxed) as f32 / 10.0;
    let yaw_deg   = CAMERA_YAW_DEG10.load(Ordering::Relaxed) as f32 / 10.0;
    let pitch_deg = CAMERA_PITCH_DEG10.load(Ordering::Relaxed) as f32 / 10.0;

    // --- SVO (display values) ---
    let node_count  = SVO_NODE_COUNT.load(Ordering::Relaxed);
    let dirty       = SVO_DIRTY_RANGES.load(Ordering::Relaxed);
    let max_depth   = SVO_MAX_DEPTH.load(Ordering::Relaxed);
    let resolution  = if max_depth > 0 { 1u64 << max_depth } else { 0 };
    let capacity    = crate::gpu::SVO_CAPACITY_NODES as u64;

    // --- World seed ---
    let seed_hi = WORLD_SEED_HI.load(Ordering::Relaxed);
    let seed_lo = WORLD_SEED_LO.load(Ordering::Relaxed);
    let seed = ((seed_hi as u64) << 32) | (seed_lo as u64);
    let active_preset = WORLD_PRESET.load(Ordering::Relaxed);
    let preset_names = ["Terrain", "Flat", "Caves", "Empty"];
    let active_preset_name = preset_names.get(active_preset as usize).copied().unwrap_or("?");

    // --- Pre-compute preset button classes (avoids if-expr in rsx attribute) ---
    let pcls = |n: u32| -> &'static str {
        if n == active_preset {
            "px-2 py-0.5 text-[10px] rounded bg-cyan-500/40 border border-cyan-400 text-white"
        } else {
            "px-2 py-0.5 text-[10px] rounded bg-white/10 hover:bg-white/20 border border-white/20 text-white/60"
        }
    };
    let cls0 = pcls(0);
    let cls1 = pcls(1);
    let cls2 = pcls(2);
    let cls3 = pcls(3);

    // --- Mutable toggle / slider state ---
    let mut free_fly       = use_signal(|| FREE_FLY_ENABLED.load(Ordering::Relaxed));
    let mut zprepass       = use_signal(|| ZPREPASS_ENABLED.load(Ordering::Relaxed));
    let mut ray_march      = use_signal(|| RAY_MARCH_ENABLED.load(Ordering::Relaxed));
    let mut neighbor_blend = use_signal(|| NEIGHBOR_BLEND_ENABLED.load(Ordering::Relaxed));
    let mut shadow_rays    = use_signal(|| SHADOW_RAYS_ENABLED.load(Ordering::Relaxed));
    let mut reflection_rays = use_signal(|| REFLECTION_RAYS_ENABLED.load(Ordering::Relaxed));
    let mut wireframe_en   = use_signal(|| WIREFRAME_ENABLED.load(Ordering::Relaxed));
    let mut depth          = use_signal(|| WIREFRAME_DEPTH.load(Ordering::Relaxed));
    let mut occupied       = use_signal(|| WIREFRAME_OCCUPIED.load(Ordering::Relaxed));

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
                    label { class: "flex items-center gap-2 cursor-pointer text-white/70 hover:text-white",
                        input {
                            r#type: "checkbox",
                            checked: *zprepass.read(),
                            onclick: move |_| {
                                let v = !*zprepass.read();
                                zprepass.set(v);
                                ZPREPASS_ENABLED.store(v, Ordering::Relaxed);
                            }
                        }
                        "Z-Prepass"
                    }

                    // Ray March sub-section
                    TreeSection { label: "Ray March".to_string(), default_open: true,
                        label { class: "flex items-center gap-2 cursor-pointer text-white font-semibold hover:text-green-300",
                            input {
                                r#type: "checkbox",
                                checked: *ray_march.read(),
                                onclick: move |_| {
                                    let v = !*ray_march.read();
                                    ray_march.set(v);
                                    RAY_MARCH_ENABLED.store(v, Ordering::Relaxed);
                                }
                            }
                            "Enable"
                        }
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
                        button {
                            class: "{cls0}",
                            onclick: move |_| { WORLD_PRESET.store(0, Ordering::Relaxed); },
                            "Terrain"
                        }
                        button {
                            class: "{cls1}",
                            onclick: move |_| { WORLD_PRESET.store(1, Ordering::Relaxed); },
                            "Flat"
                        }
                        button {
                            class: "{cls2}",
                            onclick: move |_| { WORLD_PRESET.store(2, Ordering::Relaxed); },
                            "Caves"
                        }
                        button {
                            class: "{cls3}",
                            onclick: move |_| { WORLD_PRESET.store(3, Ordering::Relaxed); },
                            "Empty"
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
        assert_eq!(s.draw_radius, 800.0);
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
