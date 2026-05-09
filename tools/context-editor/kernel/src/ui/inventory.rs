//! Voxel inventory: mini-SVO items in glass UI containers with drag-to-world.
//!
//! Items are small octree volumes that can be inspected in 3D inside inventory
//! slots, rotated, and dragged into the world where they materialise as voxels.

use bevy::prelude::*;
use std::collections::HashMap;

use crate::{
    multiplayer_backend::{
        validate_interaction_range,
        BlueprintTable,
        InventorySlot,
        ItemBlueprint,
        MultiplayerConnection,
        PlayerIdentity,
        PlayerTable,
        ReducerQueue,
        ReducerRequest,
    },
    svo::OctreeNode,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum inventory slots.
pub const MAX_SLOTS: usize = 36;

/// Maximum stack size for identical blueprints.
pub const MAX_STACK: u32 = 99;

/// Mini-SVO volume dimension (cube).
pub const MINI_SVO_DIM: u32 = 8;

/// Ghost preview opacity.
pub const GHOST_OPACITY: f32 = 0.4;

/// Maximum interaction range for drop placement.
pub const DROP_RANGE: f32 = 6.0;

/// Hotbar slot count (bottom row of inventory).
pub const HOTBAR_SIZE: usize = 9;

// ---------------------------------------------------------------------------
// Inventory cache (Bevy resource)
// ---------------------------------------------------------------------------

/// Cached inventory slot with deserialized mini-SVO for rendering.
#[derive(Clone, Debug)]
pub struct CachedSlot {
    pub slot_id: u64,
    pub blueprint_id: u32,
    pub quantity: u32,
    pub slot_index: u8,
    /// Deserialized octree nodes for this item's mini-SVO.
    pub mini_svo: Vec<OctreeNode>,
    /// Player-controlled rotation for item inspection.
    pub rotation: Quat,
}

/// Drag state for inventory interaction.
#[derive(Clone, Debug)]
pub enum DragState {
    None,
    Dragging {
        slot_index: usize,
        screen_pos: Vec2,
    },
    Previewing {
        slot_index: usize,
        world_pos: Vec3,
        valid: bool,
    },
}

impl Default for DragState {
    fn default() -> Self {
        DragState::None
    }
}

/// Client-side inventory cache synced from server subscriptions.
#[derive(Resource)]
pub struct InventoryCache {
    pub slots: Vec<CachedSlot>,
    pub active_slot: Option<usize>,
    pub drag_state: DragState,
}

impl Default for InventoryCache {
    fn default() -> Self {
        Self {
            slots: Vec::new(),
            active_slot: None,
            drag_state: DragState::None,
        }
    }
}

// ---------------------------------------------------------------------------
// Mini-SVO deserialization
// ---------------------------------------------------------------------------

/// Deserialize a mini-SVO from an item blueprint's packed voxel_data.
///
/// Format: repeated (x, y, z, r, g, b, roughness) = 7 bytes per voxel.
/// Builds a flat list of OctreeNode entries for GPU traversal.
pub fn deserialize_mini_svo(voxel_data: &[u8]) -> Vec<OctreeNode> {
    let stride = 7;
    let count = voxel_data.len() / stride;
    let mut nodes = Vec::with_capacity(count);

    for i in 0..count {
        let base = i * stride;
        if base + 6 >= voxel_data.len() {
            break;
        }
        let r = voxel_data[base + 3];
        let g = voxel_data[base + 4];
        let b = voxel_data[base + 5];
        let roughness = voxel_data[base + 6];

        let color = crate::svo::VoxelMaterial::new(r, g, b, roughness).pack();
        nodes.push(OctreeNode {
            child_pointer: 0, // leaf
            color_data: color,
        });
    }

    nodes
}

/// Count non-empty voxels in serialized voxel data.
pub fn voxel_count(voxel_data: &[u8]) -> usize {
    voxel_data.len() / 7
}

// ---------------------------------------------------------------------------
// Inventory operations
// ---------------------------------------------------------------------------

/// Find a slot by slot_index in the cache.
pub fn find_slot(
    cache: &InventoryCache,
    slot_index: u8,
) -> Option<&CachedSlot> {
    cache.slots.iter().find(|s| s.slot_index == slot_index)
}

/// Find a slot with available stack space for a blueprint.
pub fn find_stackable_slot(
    cache: &InventoryCache,
    blueprint_id: u32,
) -> Option<usize> {
    cache
        .slots
        .iter()
        .position(|s| s.blueprint_id == blueprint_id && s.quantity < MAX_STACK)
}

/// Find the first empty slot index not used by any cached slot.
pub fn find_empty_slot_index(cache: &InventoryCache) -> Option<u8> {
    let used: std::collections::HashSet<u8> =
        cache.slots.iter().map(|s| s.slot_index).collect();
    (0..MAX_SLOTS as u8).find(|i| !used.contains(i))
}

/// Validate that a world position is valid for item placement.
///
/// Checks: within drop range of player, coordinates in world bounds.
pub fn validate_drop_position(
    player_pos: (f32, f32, f32),
    target: Vec3,
) -> bool {
    let target_tuple = (target.x, target.y, target.z);
    validate_interaction_range(player_pos, target_tuple)
        && target.y >= 0.0
        && target.y < 256.0
}

// ---------------------------------------------------------------------------
// Ghost preview (transparent overlay during drag)
// ---------------------------------------------------------------------------

/// Data for rendering a ghost preview of an item being placed.
#[derive(Clone, Debug)]
pub struct GhostPreview {
    pub voxel_offsets: Vec<IVec3>,
    pub world_origin: IVec3,
    pub color: u32,
    pub opacity: f32,
    pub valid: bool,
}

/// Build ghost preview data from a cached slot and target position.
pub fn build_ghost_preview(
    slot: &CachedSlot,
    world_pos: Vec3,
    valid: bool,
    voxel_data: &[u8],
) -> GhostPreview {
    let stride = 7;
    let count = voxel_data.len() / stride;
    let origin = IVec3::new(
        world_pos.x.floor() as i32,
        world_pos.y.floor() as i32,
        world_pos.z.floor() as i32,
    );

    let mut offsets = Vec::with_capacity(count);
    for i in 0..count {
        let base = i * stride;
        if base + 2 >= voxel_data.len() {
            break;
        }
        offsets.push(IVec3::new(
            voxel_data[base] as i32,
            voxel_data[base + 1] as i32,
            voxel_data[base + 2] as i32,
        ));
    }

    // Use first node's color or fallback
    let color = slot.mini_svo.first().map_or(0xFFFFFF, |n| n.color_data);

    GhostPreview {
        voxel_offsets: offsets,
        world_origin: origin,
        color,
        opacity: if valid {
            GHOST_OPACITY
        } else {
            GHOST_OPACITY * 0.5
        },
        valid,
    }
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// System: sync inventory cache from server subscription data.
///
/// In a real implementation this would read from a SpacetimeDB subscription.
/// Here we maintain the cache resource for other systems to read.
fn inventory_sync_system(
    mut _cache: ResMut<InventoryCache>,
    _connection: Res<MultiplayerConnection>,
) {
    // Placeholder: in production, this drains subscription updates from
    // the SpacetimeDB InventorySlot table into the local cache.
    // The cache is already populated by the initial subscription.
}

/// System: handle hotbar slot selection (keys 1-9).
fn hotbar_select_system(
    input: Res<ButtonInput<KeyCode>>,
    mut cache: ResMut<InventoryCache>,
) {
    let keys = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
        KeyCode::Digit8,
        KeyCode::Digit9,
    ];
    for (i, key) in keys.iter().enumerate() {
        if input.just_pressed(*key) {
            cache.active_slot = Some(i);
        }
    }
}

/// System: request item drop when in Previewing state and mouse released.
fn drop_item_system(
    mouse: Res<ButtonInput<MouseButton>>,
    mut cache: ResMut<InventoryCache>,
    mut reducer_queue: ResMut<ReducerQueue>,
) {
    if mouse.just_released(MouseButton::Left) {
        if let DragState::Previewing {
            slot_index,
            world_pos,
            valid,
        } = &cache.drag_state
        {
            if *valid {
                reducer_queue.push(ReducerRequest::DropItem {
                    slot_index: *slot_index as u8,
                });
            }
            cache.drag_state = DragState::None;
        }
    }
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct InventoryPlugin;

impl Plugin for InventoryPlugin {
    fn build(
        &self,
        app: &mut App,
    ) {
        app.init_resource::<InventoryCache>();
        app.add_systems(
            Update,
            (
                inventory_sync_system,
                hotbar_select_system,
                drop_item_system,
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
    use crate::svo::VoxelMaterial;

    fn sample_voxel_data() -> Vec<u8> {
        let mut data = Vec::new();
        // 3 voxels at different positions
        data.extend_from_slice(&[0, 0, 0, 255, 0, 0, 20]); // red at origin
        data.extend_from_slice(&[1, 0, 0, 0, 255, 0, 15]); // green at (1,0,0)
        data.extend_from_slice(&[0, 1, 0, 0, 0, 255, 10]); // blue at (0,1,0)
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
        let mat0 = VoxelMaterial::unpack(nodes[0].color_data);
        assert_eq!(mat0.r, 255);
        assert_eq!(mat0.g, 0);
        assert_eq!(mat0.b, 0);
    }

    #[test]
    fn deserialize_empty_data() {
        let nodes = deserialize_mini_svo(&[]);
        assert!(nodes.is_empty());
    }

    #[test]
    fn deserialize_partial_stride() {
        // Less than 7 bytes — no complete voxel
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
        for i in 0..MAX_SLOTS as u8 {
            let mut slot = sample_cached_slot();
            slot.slot_index = i;
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
            Vec3::new(2.0, 10.0, 0.0),
        ));
    }

    #[test]
    fn validate_drop_position_too_far() {
        assert!(!validate_drop_position(
            (0.0, 10.0, 0.0),
            Vec3::new(100.0, 10.0, 0.0),
        ));
    }

    #[test]
    fn validate_drop_position_below_ground() {
        assert!(!validate_drop_position(
            (0.0, 10.0, 0.0),
            Vec3::new(0.0, -5.0, 0.0),
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
        let rot = Quat::from_rotation_y(std::f32::consts::FRAC_PI_4);
        slot.rotation = rot;
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
}
