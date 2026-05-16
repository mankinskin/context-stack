use super::*;

#[test]
fn chunk_sphere_center_only() {
    let set = chunk_sphere(IVec3::ZERO, 0);
    assert_eq!(set.len(), 1);
    assert!(set.contains(&IVec3::ZERO));
}

#[test]
fn chunk_sphere_radius_1() {
    let set = chunk_sphere(IVec3::ZERO, 1);
    // 3×3×3 = 27
    assert_eq!(set.len(), 27);
}

#[test]
fn chunk_sphere_radius_3() {
    let set = chunk_sphere(IVec3::ZERO, 3);
    // 7×7×7 = 343
    assert_eq!(set.len(), 343);
}

#[test]
fn chebyshev_distance_same() {
    assert_eq!(chebyshev_distance(IVec3::ZERO, IVec3::ZERO), 0);
}

#[test]
fn chebyshev_distance_axis() {
    assert_eq!(chebyshev_distance(IVec3::ZERO, IVec3::new(5, 0, 0)), 5);
}

#[test]
fn chebyshev_distance_diagonal() {
    assert_eq!(chebyshev_distance(IVec3::ZERO, IVec3::new(3, 4, 2)), 4);
}

#[test]
fn lod_for_distance_near() {
    let mgr = ChunkSubscriptionManager::default();
    assert_eq!(mgr.lod_for_distance(IVec3::ZERO, IVec3::ZERO), 0);
    assert_eq!(mgr.lod_for_distance(IVec3::ZERO, IVec3::new(1, 0, 0)), 0);
}

#[test]
fn lod_for_distance_mid() {
    let mgr = ChunkSubscriptionManager::default();
    assert_eq!(mgr.lod_for_distance(IVec3::ZERO, IVec3::new(2, 0, 0)), 2);
    assert_eq!(mgr.lod_for_distance(IVec3::ZERO, IVec3::new(3, 0, 0)), 2);
}

#[test]
fn lod_for_distance_far() {
    let mgr = ChunkSubscriptionManager::default();
    assert_eq!(mgr.lod_for_distance(IVec3::ZERO, IVec3::new(5, 0, 0)), 4);
}

#[test]
fn chunk_local_to_world_origin() {
    let w = chunk_local_to_world(IVec3::ZERO, 0, 0, 0);
    assert_eq!(w, IVec3::ZERO);
}

#[test]
fn chunk_local_to_world_offset() {
    let w = chunk_local_to_world(IVec3::new(1, 2, 3), 5, 10, 15);
    assert_eq!(w, IVec3::new(16 + 5, 32 + 10, 48 + 15));
}

#[test]
fn delta_queue_max_capacity() {
    let mut q = DeltaQueue::default();
    for i in 0..MAX_PENDING_DELTAS + 100 {
        q.push(PendingDelta {
            chunk_pos: IVec3::ZERO,
            local_x: 0,
            local_y: 0,
            local_z: 0,
            new_color: i as u32,
            tick: i as u64,
        });
    }
    assert_eq!(q.len(), MAX_PENDING_DELTAS);
}

#[test]
fn delta_queue_drain() {
    let mut q = DeltaQueue::default();
    q.push(PendingDelta {
        chunk_pos: IVec3::ZERO,
        local_x: 1,
        local_y: 2,
        local_z: 3,
        new_color: 0xFF,
        tick: 1,
    });
    let drained = q.drain_all();
    assert_eq!(drained.len(), 1);
    assert!(q.is_empty());
}

#[test]
fn offline_mutation_queue() {
    let mut q = OfflineMutationQueue::default();
    q.push(OfflineMutation {
        x: 1,
        y: 2,
        z: 3,
        new_color: 0xAA,
    });
    let drained = q.drain_all();
    assert_eq!(drained.len(), 1);
    assert!(q.mutations.is_empty());
}

#[test]
fn connection_default_disconnected() {
    let conn = NetworkConnection::default();
    assert_eq!(conn.status, ConnectionStatus::Disconnected);
}

#[test]
fn tier_counts_empty() {
    let mgr = ChunkSubscriptionManager::default();
    assert_eq!(mgr.tier_counts(), (0, 0, 0));
}

#[test]
fn tier_counts_mixed() {
    let mut mgr = ChunkSubscriptionManager::default();
    mgr.loaded_chunks.insert(
        IVec3::ZERO,
        ChunkState::FullDetail {
            node_count: 100,
            last_delta_tick: 0,
        },
    );
    mgr.loaded_chunks.insert(
        IVec3::X,
        ChunkState::LodLevel {
            depth: 2,
            node_count: 50,
        },
    );
    mgr.loaded_chunks.insert(
        IVec3::Y,
        ChunkState::LodLevel {
            depth: 4,
            node_count: 10,
        },
    );
    assert_eq!(mgr.tier_counts(), (1, 1, 1));
}
