# Phase 1b: Core SVO Ray March Compute Shader

## Problem

This is the centrepiece of the rendering rewrite. We need a compute shader that, for each pixel, casts a ray through the world-space SVO and finds the nearest voxel intersection by hierarchical traversal — replacing the entire splat-sort-tile-raster pipeline with a single shader.

**Important**: This phase uses the **existing full-SVO upload** (`SvoDoubleBuffer` with dirty-range tracking). The entire octree is resident on the GPU, so node indices in `child_pointer` are direct array lookups — no paging or address translation is needed yet. Paging is deferred to Phase 4a after the core shader is proven correct.

## Design

### Ray Generation
Per pixel (workgroup_size 8×8):
1. Compute NDC from pixel coordinates
2. Unproject two clip-space points via `inv_view_proj` to get world-space ray direction
3. Use `camera_pos` as ray origin
4. Transform ray into SVO space: `ray_origin_svo = world_to_svo(ray_origin)`, `ray_dir_svo = normalize(world_to_svo(ray_origin + ray_dir) - ray_origin_svo)`

### SVO Traversal: Small-Stack DDA

The traversal uses a small fixed-size stack (max_depth entries, typically 8) to walk the octree:

```
Algorithm:
1. Test ray against SVO root AABB. If miss → background color.
2. Push root onto stack with entry t_min.
3. While stack is not empty:
   a. Pop current node + t_enter
   b. If leaf with color_data != 0:
      - Evaluate SDF for sub-voxel precision
      - If hit (d <= 0 at surface): compute alpha, accumulate color
      - If opacity >= 0.99: early exit
   c. If internal node:
      - Read child_mask from child_pointer
      - For each set bit in child_mask:
        - Compute child AABB from parent AABB + octant index
        - Ray-AABB slab test → get (t_enter, t_exit)
        - If t_enter < t_exit and t_enter < current_best_t:
          Push (child_index, t_enter) onto stack
      - Sort children on stack by t_enter (front-to-back)  ← naive baseline; the bit-trick below replaces this
4. Output accumulated color + depth of first opaque hit
```

### Key optimizations in the traversal:
- **Bit-trick child ordering**: Use ray direction signs to determine which octant the ray enters first — **replaces the explicit sort** in the pseudocode above; children are pushed in front-to-back order directly
- **Rope/skip pointers** (future): For stackless variant
- **Early ray termination**: Stop when accumulated opacity >= 0.99
- **Interior flag skip**: When a ray reaches an interior-flagged leaf, skip SDF evaluation (its surface is fully occluded by surrounding voxels). Traversal still descends into interior *internal* nodes’ children — only the leaf-level SDF eval is skipped. In practice, front-to-back early ray termination achieves the same effect for opaque scenes

### SDF Evaluation at Leaves
When a leaf node is reached:
1. Compute the ray's closest approach point within the voxel AABB
2. Transform to voxel-local coordinates: `p_local = hit_point - voxel_center`
3. Evaluate SDF: `d = sd_box(p_local, vec3f(half_extent))` (for legacy box type)
4. Anti-aliasing: `alpha = 1.0 - smoothstep(0.0, fw, d)` where `fw = 2.0 * hit_dist / (cot_half_fov * resolution.y)` (FOV-correct pixel footprint at hit distance)
5. Compute surface normal from SDF gradient

### Output
- `output_texture: texture_storage_2d<rgba8unorm, write>` — RGBA color
- `depth_buffer: array<f32>` — view-space depth of first opaque hit (for depth compositing)

## Implementation Plan

1. **`kernel/src/render/svo_ray_march.wgsl`** (new file):
   - Uniforms: `RayMarchUniforms { inv_view_proj, camera_pos, resolution, max_depth, light_dir, light_color, cot_half_fov }` (`cot_half_fov = 1.0 / tan(fov_y * 0.5)`, used for AA pixel footprint)
   - Bindings: octree buffer, svo_transform uniform, output texture (child AABBs are computed analytically from parent AABB + octant index — no position buffer needed)
   - Entry point: `@compute @workgroup_size(8, 8) fn ray_march_main`
   - Helper functions: `ray_aabb_slab()`, `traverse_svo()`, `eval_leaf_sdf()`, `compute_normal()`

2. **`kernel/src/render/svo_ray_march.rs`** (new file):
   - `SvoRayMarchNode` implementing Bevy `render_graph::Node`
   - Pipeline creation with compute shader
   - Bind group layout: octree (storage), uniforms (uniform), output (storage texture) — no node_positions buffer (matches WGSL bindings in step 1)
   - Dispatch: `ceil(width/8) × ceil(height/8)` workgroups

3. **`kernel/src/render/mod.rs`**:
   - Add `SvoRayMarch` to `ContextEditorLabel` enum
   - Wire new node into render graph: `BufferSwap → SvoRayMarch → UiComposite`
   - Keep old pipeline in parallel initially for A/B comparison (behind a toggle)

## Basic PBR Shading (inline, not full Cook-Torrance yet)
For this phase (Phase 1b), use simplified Lambertian + ambient:
```wgsl
let diffuse = max(dot(normal, light_dir), 0.0) * light_color * base_color;
let ambient = base_color * 0.15;
let color = diffuse + ambient;
```
Full Cook-Torrance PBR is deferred to Phase 3a.

## Acceptance Criteria

1. A compute shader renders the SVO with correct depth ordering — no z-fighting.
2. Voxel faces that should be behind other voxels are NOT visible (the core bug this epic fixes).
3. Camera rotation/translation produces correct perspective rendering.
4. Empty space is traversed efficiently (ray skips empty subtrees in O(log n)).
5. SDF anti-aliasing produces smooth voxel edges (smoothstep fringe).
6. Basic lighting (ambient + diffuse) makes terrain readable.
7. Early ray termination works — opaque pixels exit the loop early.
8. Interior-flagged leaves skip SDF evaluation (their surface is occluded by surrounding voxels).
9. FPS >= 15 at 1080p for a 128³ world in release profile (basic correctness, not optimized yet).
10. Toggle in debug overlay to switch between old (tiled) and new (ray march) pipeline.

## Files Changed

| File | Change |
|------|--------|
| `kernel/src/render/svo_ray_march.wgsl` | New: core ray march compute shader |
| `kernel/src/render/svo_ray_march.rs` | New: Bevy render node, pipeline, bind groups |
| `kernel/src/render/mod.rs` | Add SvoRayMarch node to render graph, toggle system |
| `kernel/src/debug_overlay.rs` | Add ray-march vs tiled toggle |

## Dependencies

- Phase 1a (world-to-SVO transform + WGSL helpers)

Note: This phase intentionally does NOT depend on paging (Phase 4a). The existing `SvoDoubleBuffer` uploads the full octree, which is sufficient for correctness validation on small-to-medium worlds (up to ~256³). Paging optimization comes later.
