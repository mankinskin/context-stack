use super::*;

#[test]
fn spatial_hash_deterministic() {
    let h1 = spatial_hash(1, 2, 3);
    let h2 = spatial_hash(1, 2, 3);
    assert_eq!(h1, h2);
}

#[test]
fn spatial_hash_differs_for_different_coords() {
    let h1 = spatial_hash(0, 0, 0);
    let h2 = spatial_hash(1, 0, 0);
    let h3 = spatial_hash(0, 1, 0);
    assert_ne!(h1, h2);
    assert_ne!(h1, h3);
    assert_ne!(h2, h3);
}

#[test]
fn world_to_chunk_positive() {
    assert_eq!(world_to_chunk(0, 0, 0), (0, 0, 0));
    assert_eq!(world_to_chunk(15, 15, 15), (0, 0, 0));
    assert_eq!(world_to_chunk(16, 0, 0), (1, 0, 0));
    assert_eq!(world_to_chunk(32, 48, 64), (2, 3, 4));
}

#[test]
fn world_to_chunk_negative() {
    assert_eq!(world_to_chunk(-1, 0, 0), (-1, 0, 0));
    assert_eq!(world_to_chunk(-16, 0, 0), (-1, 0, 0));
    assert_eq!(world_to_chunk(-17, 0, 0), (-2, 0, 0));
}

#[test]
fn world_to_local_positive() {
    assert_eq!(world_to_local(0, 0, 0), (0, 0, 0));
    assert_eq!(world_to_local(5, 10, 15), (5, 10, 15));
    assert_eq!(world_to_local(16, 0, 0), (0, 0, 0));
    assert_eq!(world_to_local(17, 0, 0), (1, 0, 0));
}

#[test]
fn world_to_local_negative() {
    assert_eq!(world_to_local(-1, 0, 0), (15, 0, 0));
    assert_eq!(world_to_local(-16, 0, 0), (0, 0, 0));
}

#[test]
fn validate_interaction_range_within() {
    assert!(validate_interaction_range((0.0, 0.0, 0.0), (5.0, 0.0, 0.0)));
}

#[test]
fn validate_interaction_range_too_far() {
    assert!(!validate_interaction_range((0.0, 0.0, 0.0), (20.0, 0.0, 0.0)));
}

#[test]
fn validate_interaction_range_exact_boundary() {
    assert!(validate_interaction_range(
        (0.0, 0.0, 0.0),
        (MAX_INTERACTION_RANGE, 0.0, 0.0),
    ));
}

#[test]
fn compute_damage_minimum_one() {
    assert_eq!(compute_damage(0, 0.0), 1);
}

#[test]
fn compute_damage_with_weight_bonus() {
    assert_eq!(compute_damage(10, 5.0), 20);
}

#[test]
fn player_identity_local() {
    let id = PlayerIdentity::local();
    assert_eq!(id.id, 1);
}

#[test]
fn player_table_online_count() {
    let mut table = PlayerTable::default();
    assert_eq!(table.online_count(), 0);
    table.players.insert(PlayerIdentity::new(1), PlayerRow::default());
    table.players.insert(PlayerIdentity::new(2), PlayerRow::default());
    assert_eq!(table.online_count(), 2);
}

#[test]
fn inventory_occupied_count() {
    let mut inv = InventoryTable::default();
    inv.slots.push(InventorySlot {
        slot_id: 1,
        owner: PlayerIdentity::local(),
        blueprint_id: 100,
        quantity: 5,
        slot_index: 0,
    });
    inv.slots.push(InventorySlot {
        slot_id: 2,
        owner: PlayerIdentity::local(),
        blueprint_id: 200,
        quantity: 0,
        slot_index: 1,
    });
    assert_eq!(inv.occupied_count(), 1);
}

#[test]
fn reducer_queue_drain() {
    let mut queue = ReducerQueue::default();
    queue.push(ReducerRequest::PlayerMove {
        position: (1.0, 2.0, 3.0),
        yaw: 0.5,
    });
    queue.push(ReducerRequest::UpdateVoxel {
        x: 0,
        y: 0,
        z: 0,
        new_color: 0xFF0000,
    });
    let drained = queue.drain();
    assert_eq!(drained.len(), 2);
    assert!(queue.pending.is_empty());
}

#[test]
fn item_type_roundtrip() {
    for v in 0..4u8 {
        let item_type = ItemType::from_u8(v);
        assert_eq!(item_type as u8, v);
    }
    assert_eq!(ItemType::from_u8(99), ItemType::Block);
}

#[test]
fn local_game_tick_default() {
    let tick = LocalGameTick::default();
    assert_eq!(tick.tick, 0);
    assert_eq!(tick.timer_ms, 0.0);
}

#[test]
fn combat_event_queue_default_empty() {
    let queue = CombatEventQueue::default();
    assert!(queue.events.is_empty());
}
