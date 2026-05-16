use super::*;

/// Plugin that registers documentation editor resources and systems.
pub struct DocEditorPlugin;

impl Plugin for DocEditorPlugin {
    fn build(
        &self,
        app: &mut App,
    ) {
        app.init_resource::<DocStore>();
        app.init_resource::<DocNavigation>();

        app.add_systems(Update, (scroll_system, navigation_system));
    }
}
