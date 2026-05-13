# Multiplayer Backend: SpacetimeDB Server Module — Tables, Reducers, Auth & Tick Loop

## Problem

The context-editor is currently a single-player application. To enable multiplayer open-world gameplay, we need an authoritative server that stores world state, validates player actions, manages authentication, and broadcasts changes. SpacetimeDB combines database + server logic in a WASM runtime, letting us write server code in Rust and share types with the client.

## Architecture: SpacetimeDB as Authoritative World Server

### Why SpacetimeDB

- **Unified stack**: Server reducers are Rust compiled to WASM — same language as the client
- **Relational voxels**: SVO octree nodes stored as table rows with spatial indices
- **Real-time subscriptions**: Clients subscribe to SQL-like queries; changes push via WebSocket/WebTransport
- **Transactional reducers**: Every mutation is atomic, preventing race conditions
- **Identity**: Built-in cryptographic identity system for player authentication

### Module Structure

```
tools/context-editor/
├── server/
│   ├── Cargo.toml              # spacetimedb SDK dependency
│   ├── src/
│   │   ├── lib.rs              # Module entrypoint, init reducer
│   │   ├── tables.rs           # All table definitions
│   │   ├── voxel_reducers.rs   # Voxel CRUD + validation
│   │   ├── player_reducers.rs  # Player join/leave/move
│   │   ├── combat_reducers.rs  # Attack, damage, death
│   │   ├── inventory_reducers.rs # Item pickup/drop/craft
│   │   ├── tick.rs             # Scheduled reducer (game tick)
│   │   └── auth.rs             # Permission checks
│   └── spacetimedb.toml        # Module config
```

### Core Tables

```rust
#[spacetimedb::table(name = voxel_chunk, public)]
pub struct VoxelChunk {
    #[primary_key]
    pub chunk_hash: u64,          // spatial hash from (cx, cy, cz)
    pub cx: i32,
    pub cy: i32,
    pub cz: i32,
    pub octree_data: Vec<u8>,     // serialized OctreeNode array for this 16³ chunk
    pub last_modified: u64,       // tick number
}

#[spacetimedb::table(name = voxel_delta, public)]
pub struct VoxelDelta {
    #[primary_key]
    #[auto_inc]
    pub delta_id: u64,
    pub chunk_hash: u64,
    pub local_x: u8,
    pub local_y: u8,
    pub local_z: u8,
    pub new_color: u32,
    pub tick: u64,
}

#[spacetimedb::table(name = player, public)]
pub struct Player {
    #[primary_key]
    pub identity: Identity,
    pub entity_id: u64,
    pub position: (f32, f32, f32),
    pub velocity: (f32, f32, f32),
    pub rotation_yaw: f32,
    pub hp: i32,
    pub max_hp: i32,
    pub mana: i32,
    pub max_mana: i32,
    pub skin_color: u32,
    pub last_tick: u64,
}

#[spacetimedb::table(name = item_blueprint, public)]
pub struct ItemBlueprint {
    #[primary_key]
    pub blueprint_id: u32,
    pub name: String,
    pub voxel_data: Vec<u8>,      // serialized 8³ mini-SVO
    pub base_damage: i32,
    pub weight: f32,
    pub item_type: u8,            // 0=weapon, 1=tool, 2=block, 3=consumable
}

#[spacetimedb::table(name = inventory_slot, public)]
pub struct InventorySlot {
    #[primary_key]
    #[auto_inc]
    pub slot_id: u64,
    pub owner: Identity,
    pub blueprint_id: u32,
    pub quantity: u32,
    pub slot_index: u8,
}
```

### Reducers

```rust
#[spacetimedb::reducer]
pub fn update_voxel(ctx: &ReducerContext, x: i32, y: i32, z: i32, new_color: u32) -> Result<(), String> {
    let player = ctx.db.player().identity().find(ctx.sender)
        .ok_or("Not authenticated")?;

    // Validate: player must be within interaction range of target voxel
    let dist = distance(player.position, (x as f32, y as f32, z as f32));
    if dist > MAX_INTERACTION_RANGE {
        return Err("Too far away".into());
    }

    let chunk_hash = spatial_hash(x >> 4, y >> 4, z >> 4);
    // Insert delta (clients subscribed to this chunk see it immediately)
    ctx.db.voxel_delta().insert(VoxelDelta {
        delta_id: 0, chunk_hash,
        local_x: (x & 0xF) as u8, local_y: (y & 0xF) as u8, local_z: (z & 0xF) as u8,
        new_color, tick: ctx.timestamp,
    });
    Ok(())
}

#[spacetimedb::reducer]
pub fn player_move(ctx: &ReducerContext, new_pos: (f32, f32, f32), new_yaw: f32) -> Result<(), String> {
    let mut player = ctx.db.player().identity().find(ctx.sender)
        .ok_or("Not authenticated")?;
    // Server validates against SVO collision (coarse check)
    player.position = validated_position(player.position, new_pos, &ctx.db);
    player.rotation_yaw = new_yaw;
    player.last_tick = ctx.timestamp;
    ctx.db.player().identity().update(player);
    Ok(())
}
```

### Scheduled Tick Reducer

```rust
#[spacetimedb::reducer(repeat = 50ms)]
pub fn game_tick(ctx: &ReducerContext) {
    // 1. Apply pending voxel deltas into chunk octrees
    // 2. Run NPC AI step
    // 3. Check player timeout / disconnect
    // 4. Compact old deltas (keep recent N ticks)
    // 5. Resource regrowth (procedural re-seeding)
}
```

### Auth & Identity

SpacetimeDB provides cryptographic `Identity` per connection. The `on_connect` / `on_disconnect` lifecycle hooks manage player spawning:

```rust
#[spacetimedb::reducer(init)]
pub fn init(ctx: &ReducerContext) {
    // Generate initial world seed, create default blueprints
}

#[spacetimedb::reducer(client_connected)]
pub fn on_connect(ctx: &ReducerContext) {
    // Create or restore Player row for ctx.sender
}

#[spacetimedb::reducer(client_disconnected)]
pub fn on_disconnect(ctx: &ReducerContext) {
    // Mark player offline, preserve inventory
}
```

## Dependencies
- T1 (scaffold): Server module directory added to workspace, shared types between client and server

## Acceptance Criteria
1. `spacetimedb publish` deploys the WASM module successfully
2. Player table: join creates row, disconnect preserves it, re-join restores
3. `update_voxel` reducer validates range and inserts delta
4. `player_move` reducer validates position against SVO bounds
5. `game_tick` scheduled reducer fires at ~50ms interval
6. All reducers are transactional — concurrent mutations don't corrupt state
7. Identity-based auth prevents impersonation (cannot modify other player's data)
8. Item blueprints and inventory slots correctly linked via foreign key pattern
