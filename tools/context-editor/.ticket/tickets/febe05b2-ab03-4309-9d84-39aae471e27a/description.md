# Phase 1a: World-to-SVO Transform and Layout Validation

## Problem

The ray march shader needs to transform world-space rays into the SVO's normalized $[0,1]^3$ coordinate space. Currently, `compute_node_positions()` outputs world-space centers and half-extents, but there is no explicit world-to-SVO transform uniform. The shader needs to know the SVO root origin, world_size, and how to map a world-space point to an octree cell address.

Additionally, the existing 2×u32 `OctreeNode` layout (child_pointer + color_data) must be validated for efficient GPU bit-manipulation during traversal. The child_pointer encodes both an 8-bit child bitmask (bits 0-7) and a 23-bit first-child index (bits 8-30), with bit 31 reserved for INTERIOR_FLAG. The ray march shader will decode this hundreds of times per pixel, so we must ensure the bit layout is optimal.

## Scope

1. **SVO Transform Uniform**: Add a `SvoTransform` struct containing:
   - `origin: vec3f` — world-space position of the SVO root's min corner (currently always (0,0,0) but should be configurable)
   - `world_size: f32` — total side length of the SVO root cube
   - `inv_world_size: f32` — precomputed `1.0 / world_size` for fast normalization
   - `max_depth: u32` — maximum octree depth

2. **Layout validation**: Confirm the `OctreeNode` bit layout works efficiently in WGSL:
   - `child_mask = node.child_pointer & 0xFFu`
   - `first_child = (node.child_pointer >> 8u) & 0x7FFFFFu` — mask off INTERIOR_FLAG (bit 31 becomes bit 23 after shift; without masking, interior nodes produce wrong child addresses)
   - `INTERIOR_FLAG = 0x80000000u` on `child_pointer`
   - `sdf_type = (node.color_data >> 30u) & 0x3u`

3. **Rust-side uniform system**: Create Bevy resource + system to compute and upload the SVO transform each frame (stable across frames unless SVO root moves).

## Implementation Plan

1. Add `SvoTransformUniform` struct to `kernel/src/gpu/mod.rs` (or a new `svo_transform.rs`)
2. Add Bevy resource `SvoTransformBuffer` with GPU uniform buffer
3. Add `update_svo_transform` system in PostUpdate that writes origin/size/depth from `VoxelWorld`
4. Write WGSL helper functions:
   - `world_to_svo(p: vec3f) -> vec3f` — normalize to $[0,1]^3$
   - `svo_to_world(p: vec3f) -> vec3f` — denormalize
   - `svo_cell_bounds(depth: u32, cell_idx: vec3u) -> (min: vec3f, max: vec3f)` — AABB for a cell at given depth

## Acceptance Criteria

1. `SvoTransformUniform` is uploaded as a GPU uniform buffer each frame.
2. `world_to_svo(origin) == vec3f(0,0,0)` and `world_to_svo(origin + world_size) == vec3f(1,1,1)`.
3. The WGSL bit-decode helpers produce correct child_mask and first_child_index for a test node.
4. No regression in current pipeline (this is additive — the old pipeline continues to work).
5. `cargo check --target wasm32-unknown-unknown -p kernel` passes.

## Files Changed

| File | Change |
|------|--------|
| `kernel/src/gpu/mod.rs` | New `SvoTransformUniform` struct, buffer resource |
| `kernel/src/gpu/svo_transform.rs` | New: Bevy systems for computing/uploading transform |
| `kernel/src/render/svo_common.wgsl` | New: shared WGSL helpers (world_to_svo, bit decode) |
| `kernel/src/svo/mod.rs` | Expose origin/world_size getters on VoxelWorld |
