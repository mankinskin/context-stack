//! Debug overlay — SVO wireframe visualization and debug panel.
//!
//! Provides a [`DebugOverlayPlugin`] that draws octree cell wireframes via
//! spawned line-mesh entities, and a [`DebugPanel`] Dioxus component for
//! interactive control.
//!
//! Communication between Dioxus (HTML thread) and Bevy (ECS) uses lock-free
//! atomics: the UI writes settings, and the Bevy system reads them each frame.

use bevy::prelude::*;
use std::sync::atomic::{
    AtomicBool,
    AtomicI32,
    AtomicU32,
    AtomicU64,
    Ordering,
};

mod panel;
mod plugin;
#[cfg(test)]
mod tests;
mod wireframe;

pub use panel::DebugPanel;
pub use plugin::DebugOverlayPlugin;

use crate::{
    character::CharacterController,
    svo::VoxelWorld,
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
fn init_platform_info() {
    #[cfg(target_arch = "wasm32")]
    {
        let threads = web_sys::window()
            .map(|window| window.navigator().hardware_concurrency() as u32)
            .unwrap_or(1);
        CPU_THREADS.store(threads, Ordering::Relaxed);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let threads = std::thread::available_parallelism()
            .map(|count| count.get() as u32)
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
    if let Some(controller) = ctrl {
        CAMERA_YAW_DEG10.store(
            (controller.yaw.to_degrees() * 10.0) as i32,
            Ordering::Relaxed,
        );
        CAMERA_PITCH_DEG10.store(
            (controller.pitch.to_degrees() * 10.0) as i32,
            Ordering::Relaxed,
        );
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
    if let Some(generator) = generator {
        let seed = generator.seed;
        WORLD_SEED_HI.store((seed >> 32) as u32, Ordering::Relaxed);
        WORLD_SEED_LO.store(seed as u32, Ordering::Relaxed);
    }
}

/// Detect world-preset changes and rebuild the `VoxelWorld`.
fn apply_world_preset(world: &mut World) {
    let preset = WORLD_PRESET.load(Ordering::Relaxed);
    let applied = world.resource::<AppliedWorldPreset>().0;
    if preset == applied {
        return;
    }
    world.resource_mut::<AppliedWorldPreset>().0 = preset;
    crate::apply_registered_preset(preset, world);
}
