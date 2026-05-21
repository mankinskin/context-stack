use super::*;
use crate::svo::VoxelMaterial;

fn sample_voxel_data() -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&[0, 0, 0, 255, 0, 0, 20]);
    data.extend_from_slice(&[1, 0, 0, 0, 255, 0, 15]);
    data.extend_from_slice(&[0, 1, 0, 0, 0, 255, 10]);
    data
}

fn sample_cached_slot() -> CachedSlot {
    let data = sample_voxel_data();
    CachedSlot {
        slot_id: 1,
        blueprint_id: 100,
        quantity: 1,
        slot_index: 0,
        mini_svo: deserialize_mini_svo(&data),
        rotation: Quat::IDENTITY,
    }
}

#[test]
fn deserialize_mini_svo_correct_count() {
    let data = sample_voxel_data();
    let nodes = deserialize_mini_svo(&data);
    assert_eq!(nodes.len(), 3);
}

#[test]
fn deserialize_mini_svo_colors() {
    let data = sample_voxel_data();
    let nodes = deserialize_mini_svo(&data);
    let material = VoxelMaterial::unpack(nodes[0].color_data);
    assert_eq!(material.r, 255);
    assert_eq!(material.g, 0);
    assert_eq!(material.b, 0);
}

#[test]
fn deserialize_empty_data() {
    let nodes = deserialize_mini_svo(&[]);
    assert!(nodes.is_empty());
}

#[test]
fn deserialize_partial_stride() {
    let nodes = deserialize_mini_svo(&[1, 2, 3]);
    assert!(nodes.is_empty());
}

#[test]
fn voxel_count_correct() {
    let data = sample_voxel_data();
    assert_eq!(voxel_count(&data), 3);
}

#[test]
fn voxel_count_empty() {
    assert_eq!(voxel_count(&[]), 0);
}

#[test]
fn find_slot_by_index() {
    let mut cache = InventoryCache::default();
    cache.slots.push(sample_cached_slot());
    assert!(find_slot(&cache, 0).is_some());
    assert!(find_slot(&cache, 1).is_none());
}

#[test]
fn find_stackable_slot_existing() {
    let mut cache = InventoryCache::default();
    let mut slot = sample_cached_slot();
    slot.quantity = 5;
    cache.slots.push(slot);
    assert_eq!(find_stackable_slot(&cache, 100), Some(0));
}

#[test]
fn find_stackable_slot_full_stack() {
    let mut cache = InventoryCache::default();
    let mut slot = sample_cached_slot();
    slot.quantity = MAX_STACK;
    cache.slots.push(slot);
    assert!(find_stackable_slot(&cache, 100).is_none());
}

#[test]
fn find_stackable_slot_different_blueprint() {
    let mut cache = InventoryCache::default();
    cache.slots.push(sample_cached_slot());
    assert!(find_stackable_slot(&cache, 999).is_none());
}

#[test]
fn find_empty_slot_index_full() {
    let mut cache = InventoryCache::default();
    for index in 0..MAX_SLOTS as u8 {
        let mut slot = sample_cached_slot();
        slot.slot_index = index;
        cache.slots.push(slot);
    }
    assert!(find_empty_slot_index(&cache).is_none());
}

#[test]
fn find_empty_slot_index_gap() {
    let mut cache = InventoryCache::default();
    let mut slot = sample_cached_slot();
    slot.slot_index = 0;
    cache.slots.push(slot.clone());
    slot.slot_index = 2;
    cache.slots.push(slot);
    assert_eq!(find_empty_slot_index(&cache), Some(1));
}

#[test]
fn validate_drop_position_in_range() {
    assert!(validate_drop_position(
        (0.0, 10.0, 0.0),
        Vec3::new(2.0, 10.0, 0.0)
    ));
}

#[test]
fn validate_drop_position_too_far() {
    assert!(!validate_drop_position(
        (0.0, 10.0, 0.0),
        Vec3::new(100.0, 10.0, 0.0)
    ));
}

#[test]
fn validate_drop_position_below_ground() {
    assert!(!validate_drop_position(
        (0.0, 10.0, 0.0),
        Vec3::new(0.0, -5.0, 0.0)
    ));
}

#[test]
fn ghost_preview_valid_placement() {
    let slot = sample_cached_slot();
    let data = sample_voxel_data();
    let preview =
        build_ghost_preview(&slot, Vec3::new(5.0, 10.0, 3.0), true, &data);
    assert!(preview.valid);
    assert_eq!(preview.world_origin, IVec3::new(5, 10, 3));
    assert_eq!(preview.voxel_offsets.len(), 3);
    assert!((preview.opacity - GHOST_OPACITY).abs() < 0.001);
}

#[test]
fn ghost_preview_invalid_placement() {
    let slot = sample_cached_slot();
    let data = sample_voxel_data();
    let preview =
        build_ghost_preview(&slot, Vec3::new(0.0, 0.0, 0.0), false, &data);
    assert!(!preview.valid);
    assert!(preview.opacity < GHOST_OPACITY);
}

#[test]
fn drag_state_default_is_none() {
    let state = DragState::default();
    assert!(matches!(state, DragState::None));
}

#[test]
fn cached_slot_rotation() {
    let mut slot = sample_cached_slot();
    let rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_4);
    slot.rotation = rotation;
    let angle = slot.rotation.to_euler(EulerRot::YXZ).0;
    assert!((angle - std::f32::consts::FRAC_PI_4).abs() < 0.01);
}

#[test]
fn inventory_cache_default_empty() {
    let cache = InventoryCache::default();
    assert!(cache.slots.is_empty());
    assert!(cache.active_slot.is_none());
    assert!(matches!(cache.drag_state, DragState::None));
}
