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
