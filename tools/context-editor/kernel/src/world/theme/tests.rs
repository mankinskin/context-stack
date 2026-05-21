use super::*;
use crate::svo::VoxelMaterial;

#[test]
fn material_def_pack_roundtrip() {
    let def = MaterialDef {
        base_color: [0.5, 0.25, 0.75],
        roughness: 0.5,
        metallic: false,
    };
    let packed = def.pack();
    let unpacked = VoxelMaterial::unpack(packed);
    assert!((unpacked.r as f32 / 255.0 - 0.5).abs() < 0.01);
    assert!((unpacked.g as f32 / 255.0 - 0.25).abs() < 0.01);
    assert!((unpacked.b as f32 / 255.0 - 0.75).abs() < 0.01);
    assert!(!unpacked.metallic);
}

#[test]
fn material_def_metallic_pack() {
    let def = MaterialDef {
        base_color: [0.95, 0.85, 0.2],
        roughness: 0.2,
        metallic: true,
    };
    let packed = def.pack();
    let unpacked = VoxelMaterial::unpack(packed);
    assert!(unpacked.metallic);
    assert!((unpacked.roughness as f32 / 31.0 - 0.2).abs() < 0.05);
}

#[test]
fn dark_and_light_presets_differ() {
    let dark = ThemePalette::dark_default();
    let light = ThemePalette::light_default();
    assert_ne!(dark.voxel_primary.pack(), light.voxel_primary.pack());
    assert_ne!(dark.voxel_terrain.pack(), light.voxel_terrain.pack());
}

#[test]
fn resolve_returns_correct_slot() {
    let palette = ThemePalette::dark_default();
    assert_eq!(
        palette.resolve(MaterialRef::Primary).pack(),
        palette.voxel_primary.pack()
    );
    assert_eq!(
        palette.resolve(MaterialRef::Highlight).pack(),
        palette.voxel_highlight.pack()
    );
}

#[test]
fn theme_update_repacks_voxels() {
    let dark = ThemePalette::dark_default();
    let light = ThemePalette::light_default();

    let mut world = VoxelWorld::new(4);
    let mat = dark.voxel_primary.to_voxel_material();
    world.set_voxel(IVec3::new(2, 2, 2), mat);

    let leaf_idx = world
        .nodes
        .iter()
        .position(|n| n.is_leaf() && n.color_data == dark.voxel_primary.pack())
        .expect("painted voxel must exist");

    let mut ref_map = MaterialRefMap::default();
    ref_map.refs.insert(leaf_idx, MaterialRef::Primary);

    let new_packed = light.voxel_primary.pack();
    if world.nodes[leaf_idx].color_data != new_packed {
        world.nodes[leaf_idx].color_data = new_packed;
        world.mark_dirty(leaf_idx);
    }

    assert_eq!(world.nodes[leaf_idx].color_data, light.voxel_primary.pack());
    assert!(!world.take_dirty_ranges().is_empty());
}
