use super::*;

/// Registers all runtime parameter resources and the per-frame uniform upload.
pub struct RuntimeParamsPlugin;

impl Plugin for RuntimeParamsPlugin {
    fn build(
        &self,
        app: &mut App,
    ) {
        app.init_resource::<RenderParams>();
        app.add_systems(
            PostUpdate,
            (init_uniform_buffer, upload_render_uniforms).chain(),
        );
    }
}
