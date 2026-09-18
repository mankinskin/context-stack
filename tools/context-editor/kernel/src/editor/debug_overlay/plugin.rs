use super::{
    wireframe::draw_svo_wireframe,
    *,
};

pub struct DebugOverlayPlugin;

impl Plugin for DebugOverlayPlugin {
    fn build(
        &self,
        app: &mut App,
    ) {
        app.init_resource::<DebugOverlayState>();
        app.init_resource::<AppliedWorldPreset>();
        // Written by draw_svo_wireframe below and read by
        // render::wireframe_overlay::upload_wireframe_data; without this,
        // the resource is permanently absent, upload_wireframe_data always
        // takes its `None` branch, and index_count stays 0 forever — the
        // Wireframe Grid overlay never renders regardless of the Enable
        // toggle (W6 defect-evidence, ARTIFACTS.md §5 Priority 2).
        app.init_resource::<crate::render::wireframe_overlay::WireframeData>();
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
