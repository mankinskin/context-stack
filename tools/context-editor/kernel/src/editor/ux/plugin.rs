use super::*;

/// Registers editor UX systems and resources.
pub struct EditorUxPlugin;

impl Plugin for EditorUxPlugin {
    fn build(
        &self,
        app: &mut App,
    ) {
        app.init_resource::<EditHistory>();
        app.init_resource::<SymmetryState>();
        app.init_resource::<BrushPreview>();
        app.init_resource::<MaterialPicker>();
        app.add_systems(
            Update,
            (
                undo_redo_input,
                cycle_symmetry,
                update_brush_preview,
                material_picker_input,
            ),
        );
    }
}
