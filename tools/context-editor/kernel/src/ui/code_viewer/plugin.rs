use super::*;

/// Plugin registering code viewer resources and systems.
pub struct CodeViewerPlugin;

impl Plugin for CodeViewerPlugin {
    fn build(
        &self,
        app: &mut App,
    ) {
        app.init_resource::<SourceFileCache>();
        app.init_resource::<CodeSelection>();

        app.add_systems(Update, scroll_code_system);
    }
}
