//! Voxel inventory: mini-SVO items in glass UI containers with drag-to-world.
//!
//! Items are small octree volumes that can be inspected in 3D inside inventory
//! slots, rotated, and dragged into the world where they materialise as voxels.

use bevy::prelude::*;

use crate::{
    multiplayer_backend::{
        validate_interaction_range,
        MultiplayerConnection,
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
            world_pos: _,
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
mod tests;
