# Phase 2a: SDF Blending and Front-to-Back Alpha Compositing

## Problem

Phase 1b establishes basic ray-AABB leaf hits with box SDF. This ticket refines the SDF evaluation to support:

1. **Smooth-min blending** between adjacent voxel SDFs (avoiding hard seams at voxel boundaries)
2. **Per-SDF-type evaluation** (box, sphere, torus — dispatched by the 2-bit sdf_type field)
3. **Proper front-to-back alpha compositing** along the ray for semi-transparent surfaces
4. **Anti-aliased silhouettes** using the smoothstep fringe from Phase 1b, extended to all SDF types

## Design

### SDF Evaluation with Type Dispatch
The ray march shader, upon reaching a leaf, evaluates the appropriate SDF based on sdf_type bits (30-31) of color_data:
```wgsl
fn eval_voxel_sdf(p_local: vec3f, half: f32, sdf_type: u32) -> f32 {
    switch sdf_type {
        case 0u: { return sd_box(p_local, vec3f(half)); }
        case 1u: { return length(p_local) - half; }  // sphere
        case 2u: { return sd_box(p_local, vec3f(half)); }  // svo-sampled (placeholder — future: sample from SDF volume)
        case 3u: { return sd_torus(p_local, half * 0.7, half * 0.3); }  // torus/procedural
        default: { return sd_box(p_local, vec3f(half)); }
    }
}
```

### Smooth-Min Neighbor Blending
When evaluating a leaf's SDF, also sample the 6 face-adjacent neighbors:
```wgsl
fn blend_with_neighbors(center_d: f32, ray_pos: vec3f, voxel_center: vec3f,
                        half: f32, blend_k: f32) -> f32 {
    var d = center_d;
    // For each of 6 face neighbors, if occupied, compute smooth_union
    for each neighbor offset in [±x, ±y, ±z]:
        let neighbor_pos = voxel_center + offset * half * 2.0
        if neighbor_is_occupied(neighbor_pos):
            let neighbor_d = eval_voxel_sdf(ray_pos - neighbor_pos, half, neighbor_sdf_type)
            d = smooth_min(d, neighbor_d, blend_k)
    return d;
}

fn smooth_min(a: f32, b: f32, k: f32) -> f32 {
    let h = clamp(0.5 + 0.5 * (b - a) / k, 0.0, 1.0);
    return mix(b, a, h) - k * h * (1.0 - h);
}
```

The `blend_k` parameter (default: `half * 0.25`) controls how aggressively adjacent voxel surfaces merge. This eliminates hard seams between neighboring voxels of the same type.

### Alpha Compositing Along Ray
The ray march already accumulates opacity front-to-back. This ticket ensures correct blending for semi-transparent voxels:
```wgsl
// Inside traversal loop, after SDF eval:
let alpha = (1.0 - smoothstep(0.0, fw, d)) * remaining_alpha;
color += lit_color * alpha;
remaining_alpha -= alpha;
if remaining_alpha < 0.01 { break; }  // early termination
```

For future semi-transparent atoms (from SDF-DAG ticket), the opacity comes from the atom pool. For now, all legacy voxels are opaque (alpha = 1.0 when d <= 0).

### Neighbor Lookup in SVO
To blend with neighbors, the shader needs to look up whether an adjacent voxel exists. This requires a `descend_to(pos)` function in WGSL that walks the SVO from root to the target leaf:
```wgsl
fn svo_lookup(target_svo_pos: vec3f) -> u32 {
    // Walk from root, using bit operations to find the octant at each level
    // Returns color_data of the leaf, or 0 if empty/nonexistent
}
```

This is expensive (O(max_depth) per neighbor), so neighbor blending should be optional and gated behind a quality setting.

## Implementation Plan

1. Add `eval_voxel_sdf()` with type dispatch to `svo_ray_march.wgsl`
2. Add `svo_lookup()` function for neighbor queries
3. Add `smooth_min()` and `blend_with_neighbors()`
4. Add quality toggle in debug overlay: "Neighbor Blend: Off / On"
5. Ensure front-to-back alpha compositing is correct for the ray march loop

## Acceptance Criteria

1. Sphere-type voxels (sdf_type=1) render as smooth spheres, not boxes.
2. Torus-type voxels (sdf_type=3) render as donuts.
3. Adjacent box voxels with neighbor blending enabled show smooth merged surfaces (no hard seam).
4. With blending disabled, rendering matches Phase 1b output (no regression).
5. Semi-transparent SDF evaluation (d slightly positive) produces correct alpha fringe.
6. `svo_lookup()` correctly finds occupied neighbors in the octree.
7. Quality toggle visible in debug overlay.

## Forward-Compatibility Note

`svo_lookup()` uses direct `octree[idx]` array access, which works while the full SVO is resident (Phases 1–3). Phase 4a introduces paged upload with `resolve_node()` for address translation — `svo_lookup()` must be updated to use `resolve_node()` at that point. This is tracked in Phase 4a’s scope.

## Dependencies

- Phase 1b (core ray march shader — base to extend)
