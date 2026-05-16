use super::*;

/// Plugin that registers ticket-editor resources and systems.
pub struct TicketEditorPlugin;

impl Plugin for TicketEditorPlugin {
    fn build(
        &self,
        app: &mut App,
    ) {
        app.init_resource::<TicketStore>();
        app.init_resource::<TicketSelection>();
        app.init_resource::<LayoutMode>();

        app.add_systems(Update, (layout_system, selection_system));
    }
}
