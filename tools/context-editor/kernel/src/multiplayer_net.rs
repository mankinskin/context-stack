//! Multiplayer Networking: spatial subscriptions, chunk sync, and connection management.
//!
//! Manages the client-side network layer for SpacetimeDB multiplayer:
//! - Chunk subscription manager (near/mid/far LOD tiers)
//! - Delta application from server to local VoxelWorld
//! - Connection state machine with reconnection
//! - Bandwidth-aware chunk loading/unloading

use bevy::prelude::*;
use std::collections::{HashMap, HashSet, VecDeque};

use crate::multiplayer_backend::{
    PlayerIdentity, VoxelDelta, spatial_hash, world_to_chunk, CHUNK_SIZE,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Near subscription radius in chunks (1 → 3×3×3 = 27 chunks).
pub const NEAR_RADIUS: i32 = 1;
/// Mid subscription radius in chunks (3 → 7×7×7 = 343 chunks).
pub const MID_RADIUS: i32 = 3;
/// Far subscription radius in chunks (7 → 15×15×15 = 3375 chunks).
pub const FAR_RADIUS: i32 = 7;

/// Maximum pending deltas before dropping oldest.
pub const MAX_PENDING_DELTAS: usize = 4096;

/// Maximum queued mutations during disconnect.
pub const MAX_OFFLINE_QUEUE: usize = 1024;

// ---------------------------------------------------------------------------
// Connection state
// ---------------------------------------------------------------------------

/// Status of the connection to the SpacetimeDB server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting { attempts: u32 },
}

/// Transport protocol for the connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportKind {
    /// UDP-based WebTransport (preferred, lower latency).
    WebTransport,
    /// TCP-based WebSocket (reliable fallback).
    WebSocket,
}

/// Client connection state to SpacetimeDB.
#[derive(Resource)]
pub struct NetworkConnection {
    pub status: ConnectionStatus,
    pub transport: TransportKind,
    pub server_url: String,
    pub identity: Option<PlayerIdentity>,
    /// Round-trip time estimate in milliseconds.
    pub rtt_ms: f32,
    /// Bytes received this second (for bandwidth monitoring).
    pub bytes_received_sec: u64,
    /// Timer for bandwidth tracking.
    pub bandwidth_timer: f32,
}

impl Default for NetworkConnection {
    fn default() -> Self {
        Self {
            status: ConnectionStatus::Disconnected,
            transport: TransportKind::WebSocket,
            server_url: String::new(),
            identity: None,
            rtt_ms: 0.0,
            bytes_received_sec: 0,
            bandwidth_timer: 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Chunk subscription
// ---------------------------------------------------------------------------

/// State of a loaded chunk on the client.
#[derive(Clone, Debug)]
pub enum ChunkState {
    /// Chunk is being loaded from server.
    Loading,
    /// Full-detail chunk (all leaf nodes).
    FullDetail {
        node_count: usize,
        last_delta_tick: u64,
    },
    /// LOD-reduced chunk (aggregated nodes only).
    LodLevel {
        depth: u8,
        node_count: usize,
    },
    /// Chunk is being unloaded.
    Unloading,
}

/// Manages spatial chunk subscriptions around the player.
#[derive(Resource)]
pub struct ChunkSubscriptionManager {
    /// Current chunk the player is in.
    pub current_chunk: IVec3,
    /// Near subscription radius.
    pub near_radius: i32,
    /// Mid subscription radius.
    pub mid_radius: i32,
    /// Far subscription radius.
    pub far_radius: i32,
    /// Currently loaded chunks and their state.
    pub loaded_chunks: HashMap<IVec3, ChunkState>,
    /// Chunks pending unsubscription.
    pub pending_unsubscribe: Vec<IVec3>,
    /// Chunks pending subscription.
    pub pending_subscribe: Vec<(IVec3, u8)>, // (chunk_pos, lod_level)
}

impl Default for ChunkSubscriptionManager {
    fn default() -> Self {
        Self {
            current_chunk: IVec3::ZERO,
            near_radius: NEAR_RADIUS,
            mid_radius: MID_RADIUS,
            far_radius: FAR_RADIUS,
            loaded_chunks: HashMap::new(),
            pending_unsubscribe: Vec::new(),
            pending_subscribe: Vec::new(),
        }
    }
}

impl ChunkSubscriptionManager {
    /// Determine the LOD level for a chunk based on distance from center.
    pub fn lod_for_distance(&self, center: IVec3, chunk: IVec3) -> u8 {
        let dist = chebyshev_distance(center, chunk);
        if dist <= self.near_radius {
            0 // Full detail
        } else if dist <= self.mid_radius {
            2 // Mid LOD
        } else {
            4 // Far LOD
        }
    }

    /// Count chunks at each LOD tier.
    pub fn tier_counts(&self) -> (usize, usize, usize) {
        let (mut near, mut mid, mut far) = (0, 0, 0);
        for state in self.loaded_chunks.values() {
            match state {
                ChunkState::FullDetail { .. } => near += 1,
                ChunkState::LodLevel { depth, .. } if *depth <= 2 => mid += 1,
                ChunkState::LodLevel { .. } => far += 1,
                _ => {}
            }
        }
        (near, mid, far)
    }
}

/// Generate a set of chunk positions within a radius (Chebyshev / cube region).
pub fn chunk_sphere(center: IVec3, radius: i32) -> HashSet<IVec3> {
    let mut set = HashSet::new();
    for z in -radius..=radius {
        for y in -radius..=radius {
            for x in -radius..=radius {
                set.insert(center + IVec3::new(x, y, z));
            }
        }
    }
    set
}

/// Chebyshev distance between two chunk positions.
pub fn chebyshev_distance(a: IVec3, b: IVec3) -> i32 {
    let d = (a - b).abs();
    d.x.max(d.y).max(d.z)
}

// ---------------------------------------------------------------------------
// Delta queue
// ---------------------------------------------------------------------------

/// A voxel delta received from the server, ready for local application.
#[derive(Clone, Debug)]
pub struct PendingDelta {
    pub chunk_pos: IVec3,
    pub local_x: u8,
    pub local_y: u8,
    pub local_z: u8,
    pub new_color: u32,
    pub tick: u64,
}

/// Queue of voxel deltas from server subscriptions.
#[derive(Resource, Default)]
pub struct DeltaQueue {
    pub deltas: VecDeque<PendingDelta>,
}

impl DeltaQueue {
    pub fn push(&mut self, delta: PendingDelta) {
        if self.deltas.len() >= MAX_PENDING_DELTAS {
            self.deltas.pop_front();
        }
        self.deltas.push_back(delta);
    }

    pub fn drain_all(&mut self) -> Vec<PendingDelta> {
        self.deltas.drain(..).collect()
    }

    pub fn len(&self) -> usize {
        self.deltas.len()
    }

    pub fn is_empty(&self) -> bool {
        self.deltas.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Offline mutation queue
// ---------------------------------------------------------------------------

/// Mutations queued during disconnect, to replay on reconnect.
#[derive(Resource, Default)]
pub struct OfflineMutationQueue {
    pub mutations: Vec<OfflineMutation>,
}

/// A voxel mutation made while offline.
#[derive(Clone, Debug)]
pub struct OfflineMutation {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub new_color: u32,
}

impl OfflineMutationQueue {
    pub fn push(&mut self, mutation: OfflineMutation) {
        if self.mutations.len() < MAX_OFFLINE_QUEUE {
            self.mutations.push(mutation);
        }
    }

    pub fn drain_all(&mut self) -> Vec<OfflineMutation> {
        std::mem::take(&mut self.mutations)
    }
}

// ---------------------------------------------------------------------------
// Convert server delta to world coordinates
// ---------------------------------------------------------------------------

/// Convert chunk position + local coordinates to world coordinates.
pub fn chunk_local_to_world(chunk: IVec3, lx: u8, ly: u8, lz: u8) -> IVec3 {
    IVec3::new(
        chunk.x * CHUNK_SIZE as i32 + lx as i32,
        chunk.y * CHUNK_SIZE as i32 + ly as i32,
        chunk.z * CHUNK_SIZE as i32 + lz as i32,
    )
}

// ---------------------------------------------------------------------------
// Bevy systems
// ---------------------------------------------------------------------------

/// System: update chunk subscriptions when the player moves to a new chunk.
fn update_subscriptions_system(
    camera_q: Query<&GlobalTransform, With<Camera3d>>,
    mut sub_mgr: ResMut<ChunkSubscriptionManager>,
    connection: Res<NetworkConnection>,
) {
    let Ok(cam_transform) = camera_q.single() else {
        return;
    };

    let pos = cam_transform.translation();
    let (cx, cy, cz) = world_to_chunk(
        pos.x.floor() as i32,
        pos.y.floor() as i32,
        pos.z.floor() as i32,
    );
    let new_chunk = IVec3::new(cx, cy, cz);

    if new_chunk == sub_mgr.current_chunk {
        return;
    }
    sub_mgr.current_chunk = new_chunk;

    // Compute desired chunk sets for each tier
    let far_set = chunk_sphere(new_chunk, sub_mgr.far_radius);
    let currently_loaded: HashSet<IVec3> = sub_mgr.loaded_chunks.keys().copied().collect();

    // Unsubscribe chunks outside far range
    sub_mgr.pending_unsubscribe.clear();
    for &chunk in &currently_loaded {
        if !far_set.contains(&chunk) {
            sub_mgr.pending_unsubscribe.push(chunk);
        }
    }

    // Subscribe new chunks
    sub_mgr.pending_subscribe.clear();
    for &chunk in &far_set {
        if !currently_loaded.contains(&chunk) {
            let lod = sub_mgr.lod_for_distance(new_chunk, chunk);
            sub_mgr.pending_subscribe.push((chunk, lod));
        }
    }
}

/// System: apply pending deltas from server to VoxelWorld.
fn apply_server_deltas_system(
    mut delta_queue: ResMut<DeltaQueue>,
    mut world: ResMut<crate::svo::VoxelWorld>,
) {
    let deltas = delta_queue.drain_all();
    for delta in deltas {
        let world_pos = chunk_local_to_world(
            delta.chunk_pos,
            delta.local_x,
            delta.local_y,
            delta.local_z,
        );
        if delta.new_color == 0 {
            world.remove_voxel(world_pos);
        } else {
            let mat = crate::svo::VoxelMaterial::unpack(delta.new_color);
            world.set_voxel(world_pos, mat);
        }
    }
}

/// System: track bandwidth usage.
fn bandwidth_tracking_system(
    time: Res<Time>,
    mut connection: ResMut<NetworkConnection>,
) {
    connection.bandwidth_timer += time.delta_secs();
    if connection.bandwidth_timer >= 1.0 {
        connection.bandwidth_timer -= 1.0;
        connection.bytes_received_sec = 0;
    }
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

/// Plugin registering multiplayer networking systems.
pub struct MultiplayerNetPlugin;

impl Plugin for MultiplayerNetPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NetworkConnection>();
        app.init_resource::<ChunkSubscriptionManager>();
        app.init_resource::<DeltaQueue>();
        app.init_resource::<OfflineMutationQueue>();

        app.add_systems(
            Update,
            (
                update_subscriptions_system,
                apply_server_deltas_system,
                bandwidth_tracking_system,
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
                local_x: 0, local_y: 0, local_z: 0,
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
            local_x: 1, local_y: 2, local_z: 3,
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
        q.push(OfflineMutation { x: 1, y: 2, z: 3, new_color: 0xAA });
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
        mgr.loaded_chunks.insert(IVec3::ZERO, ChunkState::FullDetail {
            node_count: 100,
            last_delta_tick: 0,
        });
        mgr.loaded_chunks.insert(IVec3::X, ChunkState::LodLevel {
            depth: 2,
            node_count: 50,
        });
        mgr.loaded_chunks.insert(IVec3::Y, ChunkState::LodLevel {
            depth: 4,
            node_count: 10,
        });
        assert_eq!(mgr.tier_counts(), (1, 1, 1));
    }
}
