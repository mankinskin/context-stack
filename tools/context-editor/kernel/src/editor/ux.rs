//! Editor UX — undo/redo, symmetry, live brush preview, and material picker.
//!
//! Extends the core [`EditorState`] with a productive editing workflow.

use crate::{
    editor::{
        EditorState,
        VoxelHit,
    },
    svo::{
        VoxelMaterial,
        VoxelWorld,
    },
    theme::ThemePalette,
};
use bevy::prelude::*;

// ---------------------------------------------------------------------------
// Undo / Redo
// ---------------------------------------------------------------------------

/// A before/after snapshot for a single voxel.
#[derive(Clone, Debug)]
pub struct VoxelChange {
    pub pos: IVec3,
    pub old: Option<u32>,
    pub new: Option<u32>,
}

/// One undoable edit (groups all voxel changes from a single tool stroke).
#[derive(Clone, Debug, Default)]
pub struct EditSnapshot {
    pub changes: Vec<VoxelChange>,
}

/// Maximum depth of the undo stack.
pub const MAX_HISTORY: usize = 256;

/// Bevy resource holding the edit undo/redo stacks.
#[derive(Resource)]
pub struct EditHistory {
    pub undos: Vec<EditSnapshot>,
    pub redos: Vec<EditSnapshot>,
}

impl Default for EditHistory {
    fn default() -> Self {
        Self {
            undos: Vec::with_capacity(MAX_HISTORY),
            redos: Vec::with_capacity(MAX_HISTORY),
        }
    }
}

impl EditHistory {
    /// Record a new edit (clears redo stack).
    pub fn push_edit(
        &mut self,
        snapshot: EditSnapshot,
    ) {
        if snapshot.changes.is_empty() {
            return;
        }
        if self.undos.len() >= MAX_HISTORY {
            self.undos.remove(0);
        }
        self.undos.push(snapshot);
        self.redos.clear();
    }

    /// Undo the last edit, returning the snapshot to apply.
    pub fn undo(&mut self) -> Option<EditSnapshot> {
        let snap = self.undos.pop()?;
        self.redos.push(snap.clone());
        Some(snap)
    }

    /// Redo the last undone edit, returning the snapshot to apply.
    pub fn redo(&mut self) -> Option<EditSnapshot> {
        let snap = self.redos.pop()?;
        self.undos.push(snap.clone());
        Some(snap)
    }
}

/// Apply an undo by restoring old voxel values.
pub fn apply_undo(
    world: &mut VoxelWorld,
    snapshot: &EditSnapshot,
) {
    for change in &snapshot.changes {
        match change.old {
            Some(packed) => {
                let mat = VoxelMaterial::unpack(packed);
                world.set_voxel(change.pos, mat);
            },
            None => {
                world.remove_voxel(change.pos);
            },
        }
    }
}

/// Apply a redo by restoring new voxel values.
pub fn apply_redo(
    world: &mut VoxelWorld,
    snapshot: &EditSnapshot,
) {
    for change in &snapshot.changes {
        match change.new {
            Some(packed) => {
                let mat = VoxelMaterial::unpack(packed);
                world.set_voxel(change.pos, mat);
            },
            None => {
                world.remove_voxel(change.pos);
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Symmetry
// ---------------------------------------------------------------------------

/// Symmetry mode for mirrored editing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Symmetry {
    None,
    MirrorX,
    MirrorXZ,
    Radial(u32),
}

/// Bevy resource holding the active symmetry mode.
#[derive(Resource)]
pub struct SymmetryState {
    pub mode: Symmetry,
    /// Pivot point for symmetry (world units).
    pub pivot: IVec3,
}

impl Default for SymmetryState {
    fn default() -> Self {
        Self {
            mode: Symmetry::None,
            pivot: IVec3::new(128, 128, 128),
        }
    }
}

/// Given a voxel position and symmetry mode, return all mirrored positions
/// (including the original).
pub fn mirror_positions(
    pos: IVec3,
    symmetry: &SymmetryState,
) -> Vec<IVec3> {
    let p = pos;
    let piv = symmetry.pivot;
    match symmetry.mode {
        Symmetry::None => vec![p],
        Symmetry::MirrorX => {
            let mirrored_x = piv.x * 2 - p.x;
            if mirrored_x == p.x {
                vec![p]
            } else {
                vec![p, IVec3::new(mirrored_x, p.y, p.z)]
            }
        },
        Symmetry::MirrorXZ => {
            let mx = piv.x * 2 - p.x;
            let mz = piv.z * 2 - p.z;
            let mut result = vec![p];
            if mx != p.x {
                result.push(IVec3::new(mx, p.y, p.z));
            }
            if mz != p.z {
                result.push(IVec3::new(p.x, p.y, mz));
            }
            if mx != p.x && mz != p.z {
                result.push(IVec3::new(mx, p.y, mz));
            }
            result
        },
        Symmetry::Radial(n) => {
            if n <= 1 {
                return vec![p];
            }
            let rel = p - piv;
            let fx = rel.x as f32;
            let fz = rel.z as f32;
            let mut result = Vec::with_capacity(n as usize);
            for i in 0..n {
                let angle = (i as f32) * std::f32::consts::TAU / (n as f32);
                let rx = fx * angle.cos() - fz * angle.sin();
                let rz = fx * angle.sin() + fz * angle.cos();
                result.push(
                    piv + IVec3::new(
                        rx.round() as i32,
                        rel.y,
                        rz.round() as i32,
                    ),
                );
            }
            result.dedup();
            result
        },
    }
}

// ---------------------------------------------------------------------------
// Brush preview
// ---------------------------------------------------------------------------

/// Per-frame preview data for the brush hover overlay.
#[derive(Resource, Default)]
pub struct BrushPreview {
    /// Positions to show as semi-transparent preview splats.
    pub positions: Vec<IVec3>,
    /// Material to preview.
    pub material: Option<VoxelMaterial>,
    /// Whether the preview is active.
    pub active: bool,
}

/// System: compute brush preview positions from the current hit and editor state.
fn update_brush_preview(
    editor: Res<EditorState>,
    hit: Res<VoxelHit>,
    symmetry: Res<SymmetryState>,
    mut preview: ResMut<BrushPreview>,
) {
    preview.positions.clear();
    preview.active = false;

    if !editor.enabled {
        return;
    }
    let Some(ref info) = hit.hit else {
        return;
    };

    preview.active = true;
    preview.material = Some(editor.current_material);

    let center = info.cell + info.normal.round().as_ivec3();
    let brush_r = editor.brush_size as f32;
    let brush_ri = editor.brush_size as i32;

    for dx in -brush_ri..=brush_ri {
        for dy in -brush_ri..=brush_ri {
            for dz in -brush_ri..=brush_ri {
                let off = IVec3::new(dx, dy, dz);
                if off.as_vec3().length() <= brush_r {
                    let base = center + off;
                    for pos in mirror_positions(base, &symmetry) {
                        preview.positions.push(pos);
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Material picker
// ---------------------------------------------------------------------------

/// Bevy resource for the material picker selection state.
#[derive(Resource)]
pub struct MaterialPicker {
    /// Index into the theme palette slots (0–7).
    pub selected_slot: usize,
}

impl Default for MaterialPicker {
    fn default() -> Self {
        Self { selected_slot: 0 }
    }
}

/// System: pressing F1–F8 selects a material slot from the palette and
/// updates the editor's `current_material`.
fn material_picker_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut picker: ResMut<MaterialPicker>,
    palette: Option<Res<ThemePalette>>,
    mut editor: ResMut<EditorState>,
) {
    let slot_keys = [
        KeyCode::F1,
        KeyCode::F2,
        KeyCode::F3,
        KeyCode::F4,
        KeyCode::F5,
        KeyCode::F6,
        KeyCode::F7,
        KeyCode::F8,
    ];

    for (i, &key) in slot_keys.iter().enumerate() {
        if keys.just_pressed(key) {
            picker.selected_slot = i;
            if let Some(ref pal) = palette {
                let materials = palette_materials(pal);
                if let Some(&mat) = materials.get(i) {
                    editor.current_material = mat;
                }
            }
        }
    }
}

/// Extract materials from a [`ThemePalette`] for the picker.
pub fn palette_materials(palette: &ThemePalette) -> Vec<VoxelMaterial> {
    use crate::theme::MaterialRef;
    let refs = [
        MaterialRef::Primary,
        MaterialRef::Secondary,
        MaterialRef::Terrain,
        MaterialRef::Highlight,
    ];
    refs.iter()
        .map(|r| palette.resolve(*r).to_voxel_material())
        .collect()
}

// ---------------------------------------------------------------------------
// Undo/Redo systems
// ---------------------------------------------------------------------------

/// System: Ctrl+Z for undo, Ctrl+Y for redo.
fn undo_redo_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut history: ResMut<EditHistory>,
    mut world: ResMut<VoxelWorld>,
) {
    let ctrl = keys.pressed(KeyCode::ControlLeft)
        || keys.pressed(KeyCode::ControlRight);

    if ctrl && keys.just_pressed(KeyCode::KeyZ) {
        if let Some(snap) = history.undo() {
            apply_undo(&mut world, &snap);
        }
    }

    if ctrl && keys.just_pressed(KeyCode::KeyY) {
        if let Some(snap) = history.redo() {
            apply_redo(&mut world, &snap);
        }
    }
}

/// System: cycle symmetry mode with S key (while editor is active).
fn cycle_symmetry(
    keys: Res<ButtonInput<KeyCode>>,
    editor: Res<EditorState>,
    mut symmetry: ResMut<SymmetryState>,
) {
    if !editor.enabled || !keys.just_pressed(KeyCode::KeyS) {
        return;
    }
    symmetry.mode = match symmetry.mode {
        Symmetry::None => Symmetry::MirrorX,
        Symmetry::MirrorX => Symmetry::MirrorXZ,
        Symmetry::MirrorXZ => Symmetry::Radial(4),
        Symmetry::Radial(_) => Symmetry::None,
    };
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undo_redo_cycle() {
        let mut history = EditHistory::default();
        let snap = EditSnapshot {
            changes: vec![VoxelChange {
                pos: IVec3::ZERO,
                old: None,
                new: Some(VoxelMaterial::new(255, 0, 0, 16).pack()),
            }],
        };
        history.push_edit(snap);
        assert_eq!(history.undos.len(), 1);
        assert!(history.redos.is_empty());

        // Undo
        let undone = history.undo();
        assert!(undone.is_some());
        assert!(history.undos.is_empty());
        assert_eq!(history.redos.len(), 1);

        // Redo
        let redone = history.redo();
        assert!(redone.is_some());
        assert_eq!(history.undos.len(), 1);
        assert!(history.redos.is_empty());
    }

    #[test]
    fn new_edit_clears_redos() {
        let mut history = EditHistory::default();
        let snap1 = EditSnapshot {
            changes: vec![VoxelChange {
                pos: IVec3::ZERO,
                old: None,
                new: Some(1),
            }],
        };
        let snap2 = EditSnapshot {
            changes: vec![VoxelChange {
                pos: IVec3::ONE,
                old: None,
                new: Some(2),
            }],
        };
        history.push_edit(snap1);
        history.undo();
        assert_eq!(history.redos.len(), 1);

        history.push_edit(snap2);
        assert!(
            history.redos.is_empty(),
            "redo stack should be cleared after new edit"
        );
    }

    #[test]
    fn history_respects_max() {
        let mut history = EditHistory::default();
        for i in 0..(MAX_HISTORY + 10) {
            history.push_edit(EditSnapshot {
                changes: vec![VoxelChange {
                    pos: IVec3::new(i as i32, 0, 0),
                    old: None,
                    new: Some(i as u32),
                }],
            });
        }
        assert!(history.undos.len() <= MAX_HISTORY);
    }

    #[test]
    fn mirror_x_produces_two_positions() {
        let sym = SymmetryState {
            mode: Symmetry::MirrorX,
            pivot: IVec3::new(128, 128, 128),
        };
        let pos = IVec3::new(130, 128, 128);
        let mirrored = mirror_positions(pos, &sym);
        assert_eq!(mirrored.len(), 2);
        assert_eq!(mirrored[0], IVec3::new(130, 128, 128));
        assert_eq!(mirrored[1], IVec3::new(126, 128, 128));
    }

    #[test]
    fn mirror_x_on_axis_produces_one() {
        let sym = SymmetryState {
            mode: Symmetry::MirrorX,
            pivot: IVec3::new(128, 128, 128),
        };
        let pos = IVec3::new(128, 128, 128); // on the axis
        let mirrored = mirror_positions(pos, &sym);
        assert_eq!(mirrored.len(), 1);
    }

    #[test]
    fn mirror_xz_produces_four_positions() {
        let sym = SymmetryState {
            mode: Symmetry::MirrorXZ,
            pivot: IVec3::new(128, 128, 128),
        };
        let pos = IVec3::new(130, 128, 130);
        let mirrored = mirror_positions(pos, &sym);
        assert_eq!(mirrored.len(), 4);
    }

    #[test]
    fn radial_symmetry_count() {
        let sym = SymmetryState {
            mode: Symmetry::Radial(6),
            pivot: IVec3::new(128, 128, 128),
        };
        let pos = IVec3::new(130, 128, 128);
        let mirrored = mirror_positions(pos, &sym);
        // Might have some duplicates due to rounding, but should be ≥ 2
        assert!(mirrored.len() >= 2);
        assert!(mirrored.len() <= 6);
    }

    #[test]
    fn apply_undo_restores_voxel() {
        let mut world = VoxelWorld::new(8);
        let mat = VoxelMaterial::new(100, 150, 200, 10);
        world.set_voxel(IVec3::ZERO, mat);

        let snap = EditSnapshot {
            changes: vec![VoxelChange {
                pos: IVec3::ZERO,
                old: None, // was empty
                new: Some(mat.pack()),
            }],
        };

        // Undo should remove the voxel
        apply_undo(&mut world, &snap);
        // After undo, descend_to may return None or a cleared node
        // The voxel should be removed
        let node = world.descend_to(IVec3::ZERO);
        if let Some(idx) = node {
            assert_eq!(
                world.nodes[idx].color_data, 0,
                "voxel should be cleared after undo"
            );
        }
    }

    #[test]
    fn palette_materials_returns_4() {
        let palette = ThemePalette::dark_default();
        let mats = palette_materials(&palette);
        assert_eq!(mats.len(), 4);
    }

    #[test]
    fn empty_edit_not_pushed() {
        let mut history = EditHistory::default();
        history.push_edit(EditSnapshot { changes: vec![] });
        assert!(history.undos.is_empty(), "empty edits should be ignored");
    }
}
