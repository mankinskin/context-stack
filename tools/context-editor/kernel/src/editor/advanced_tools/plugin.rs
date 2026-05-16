use super::*;

pub struct AdvancedToolsPlugin;

impl Plugin for AdvancedToolsPlugin {
    fn build(
        &self,
        app: &mut App,
    ) {
        app.init_resource::<AdvancedToolState>();
        app.add_systems(
            Update,
            (
                select_advanced_tool,
                apply_fill,
                apply_smooth,
                apply_extrude,
                apply_clone,
            ),
        );
    }
}
