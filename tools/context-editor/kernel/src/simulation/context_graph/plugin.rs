use super::*;

pub struct ContextGraph3DPlugin;

impl Plugin for ContextGraph3DPlugin {
    fn build(
        &self,
        app: &mut App,
    ) {
        app.init_resource::<GraphData>();
        app.init_resource::<GraphSelection>();
        app.add_systems(Update, (layout_system, zoom_camera_system));
    }
}
