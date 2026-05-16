use super::*;

#[test]
fn undo_redo_cycle() {
    let mut history = EditHistory::default();
    let snapshot = EditSnapshot {
        changes: vec![VoxelChange {
            pos: IVec3::ZERO,
            old: None,
            new: Some(VoxelMaterial::new(255, 0, 0, 16).pack()),
        }],
    };
    history.push_edit(snapshot);
    assert_eq!(history.undos.len(), 1);
    assert!(history.redos.is_empty());

    let undone = history.undo();
    assert!(undone.is_some());
    assert!(history.undos.is_empty());
    assert_eq!(history.redos.len(), 1);

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
    assert!(history.redos.is_empty(), "redo stack should be cleared after new edit");
}

#[test]
fn history_respects_max() {
    let mut history = EditHistory::default();
    for index in 0..(MAX_HISTORY + 10) {
        history.push_edit(EditSnapshot {
            changes: vec![VoxelChange {
                pos: IVec3::new(index as i32, 0, 0),
                old: None,
                new: Some(index as u32),
            }],
        });
    }
    assert!(history.undos.len() <= MAX_HISTORY);
}

#[test]
fn mirror_x_produces_two_positions() {
    let symmetry = SymmetryState {
        mode: Symmetry::MirrorX,
        pivot: IVec3::new(128, 128, 128),
    };
    let pos = IVec3::new(130, 128, 128);
    let mirrored = mirror_positions(pos, &symmetry);
    assert_eq!(mirrored.len(), 2);
    assert_eq!(mirrored[0], IVec3::new(130, 128, 128));
    assert_eq!(mirrored[1], IVec3::new(126, 128, 128));
}

#[test]
fn mirror_x_on_axis_produces_one() {
    let symmetry = SymmetryState {
        mode: Symmetry::MirrorX,
        pivot: IVec3::new(128, 128, 128),
    };
    let pos = IVec3::new(128, 128, 128);
    let mirrored = mirror_positions(pos, &symmetry);
    assert_eq!(mirrored.len(), 1);
}

#[test]
fn mirror_xz_produces_four_positions() {
    let symmetry = SymmetryState {
        mode: Symmetry::MirrorXZ,
        pivot: IVec3::new(128, 128, 128),
    };
    let pos = IVec3::new(130, 128, 130);
    let mirrored = mirror_positions(pos, &symmetry);
    assert_eq!(mirrored.len(), 4);
}

#[test]
fn radial_symmetry_count() {
    let symmetry = SymmetryState {
        mode: Symmetry::Radial(6),
        pivot: IVec3::new(128, 128, 128),
    };
    let pos = IVec3::new(130, 128, 128);
    let mirrored = mirror_positions(pos, &symmetry);
    assert!(mirrored.len() >= 2);
    assert!(mirrored.len() <= 6);
}

#[test]
fn apply_undo_restores_voxel() {
    let mut world = VoxelWorld::new(8);
    let material = VoxelMaterial::new(100, 150, 200, 10);
    world.set_voxel(IVec3::ZERO, material);

    let snapshot = EditSnapshot {
        changes: vec![VoxelChange {
            pos: IVec3::ZERO,
            old: None,
            new: Some(material.pack()),
        }],
    };

    apply_undo(&mut world, &snapshot);
    let node = world.descend_to(IVec3::ZERO);
    if let Some(index) = node {
        assert_eq!(world.nodes[index].color_data, 0, "voxel should be cleared after undo");
    }
}

#[test]
fn palette_materials_returns_4() {
    let palette = ThemePalette::dark_default();
    let materials = palette_materials(&palette);
    assert_eq!(materials.len(), 4);
}

#[test]
fn empty_edit_not_pushed() {
    let mut history = EditHistory::default();
    history.push_edit(EditSnapshot { changes: vec![] });
    assert!(history.undos.is_empty(), "empty edits should be ignored");
}
