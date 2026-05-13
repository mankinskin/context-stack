# Phase 2b: Secondary Rays — Reflections, Refractions, Shadows

## Problem

The old pipeline required special-case code for glass panels (refraction, chromatic aberration, caustics) as a pre-pass before the splat loop. With SVO ray marching, secondary rays (reflections, refractions, shadow rays) become a natural extension of the primary traversal — just cast another ray through the same SVO.

This ticket replaces the removed glass panel system with a uniform secondary ray system that works for ALL voxel materials, not just dedicated glass objects.

## Design

### Material-Driven Secondary Rays
After the primary ray hits a surface, the material properties determine secondary ray behavior:
- **Metallic > 0.5**: cast reflection ray (mirror direction via surface normal)
- **Transparent flag** (from atom system / sdf_type extension): cast refraction ray (Snell's law)
- **Shadow**: always cast a shadow ray toward the light direction

### Shadow Rays
```wgsl
fn trace_shadow_ray(hit_pos: vec3f, light_dir: vec3f, normal: vec3f) -> f32 {
    // Cast ray from hit_pos toward light
    // Return 0.0 if occluded (any SVO hit), 1.0 if clear
    // Use simplified traversal — only need binary hit/miss, no SDF eval
    let shadow_origin = hit_pos + normal * 0.01;  // bias to avoid self-intersection
    return svo_trace_occlusion(shadow_origin, light_dir, max_shadow_dist);
}
```

Shadow rays use a simplified traversal: they only need to detect ANY hit, not the nearest. This is cheaper than primary rays because:
- No SDF evaluation needed (just test leaf occupancy)
- No alpha compositing
- No lighting computation
- Early exit on first hit

### Reflection Rays
```wgsl
fn trace_reflection(hit_pos: vec3f, normal: vec3f, ray_dir: vec3f,
                    roughness: f32, max_bounces: u32) -> vec3f {
    if max_bounces == 0u { return vec3f(0.0); }  // guard: u32 underflow would wrap to 0xFFFFFFFF
    let reflect_dir = reflect(ray_dir, normal);
    // Optionally jitter by roughness for glossy reflections
    return svo_trace_color(hit_pos + normal * 0.01, reflect_dir, max_bounces - 1u);
}
```

Reflections are full secondary traversals that return color. Limit to 1-2 bounces for performance.

### Refraction Rays
```wgsl
fn trace_refraction(hit_pos: vec3f, normal: vec3f, ray_dir: vec3f,
                    ior: f32, max_bounces: u32) -> vec3f {
    if max_bounces == 0u { return vec3f(0.0); }  // guard: u32 underflow would wrap to 0xFFFFFFFF
    let refract_dir = refract(ray_dir, normal, 1.0 / ior);
    if length(refract_dir) < 0.001 { return vec3f(0.0); }  // total internal reflection
    return svo_trace_color(hit_pos - normal * 0.01, refract_dir, max_bounces - 1u);
}
```

### Ray Budget
To prevent unbounded recursion:
- `max_bounces` uniform (default: 2)
- Reflection/refraction rays have bounce_count - 1
- Shadow rays don't decrement (they're always terminal)
- At max_bounces == 0, return ambient/sky color
- **Guard all recursive calls with `if max_bounces > 0u`** before subtracting — WGSL `u32` underflow wraps to `0xFFFFFFFF` and would produce runaway traversal

**WGSL recursion constraint**: WGSL does not allow recursive function calls. The pseudocode above shows `trace_reflection`/`trace_refraction` calling `svo_trace_color`, which in turn would call them again — that's recursion and won't compile. The actual implementation must use an **iterative bounce loop**:
```wgsl
var color = vec3f(0.0);
var throughput = vec3f(1.0);
var ray_pos = primary_hit_pos;
var ray_dir = primary_ray_dir;
for (var bounce = 0u; bounce <= max_bounces; bounce++) {
    let hit = svo_trace(ray_pos, ray_dir);
    if !hit.valid { color += throughput * sky_color; break; }
    // Shadow ray (always terminal — not affected by bounce loop)
    let shadow = trace_shadow_ray(hit.pos, light_dir, hit.normal);
    color += throughput * shade_pbr(hit, shadow);
    // Determine next bounce direction from material
    if hit.metallic > 0.5 {
        ray_dir = reflect(ray_dir, hit.normal);
        throughput *= fresnel;
    } else if hit.transparent {
        ray_dir = refract(ray_dir, hit.normal, 1.0 / hit.ior);
        if length(ray_dir) < 0.001 { break; } // TIR
    } else { break; } // diffuse — no further bounces
    ray_pos = hit.pos + hit.normal * 0.01 * sign;
}
```
The helper functions (`trace_reflection`, `trace_refraction`) shown earlier are **conceptual** — the shipped code uses the iterative loop above.

### Integration into Primary Ray March
After primary hit:
```wgsl
let shadow    = trace_shadow_ray(hit_pos, light_dir, normal);
let diffuse   = max(dot(normal, light_dir), 0.0) * shadow;
var final_color = base_color * (diffuse * light_color + ambient);

if metallic > 0.5 {
    let refl = trace_reflection(hit_pos, normal, ray_dir, roughness, max_bounces);
    final_color = mix(final_color, refl, fresnel);
}
```

## Implementation Plan

1. Factor out `svo_trace_color()` as a reusable function returning `vec3f` color (primary rays already implemented in Phase 1b); this is distinct from `svo_trace_occlusion()` (binary hit/miss, step 2)
2. Add `svo_trace_occlusion()` — simplified binary hit/miss traversal for shadows
3. Add shadow ray to primary hit shading
4. Add reflection ray for metallic surfaces
5. Add refraction ray infrastructure (used when atom system adds transparent materials)
6. Add `max_bounces` and `max_shadow_dist` to `RayMarchUniforms`
7. Add shadow and reflection toggles to debug overlay

## Acceptance Criteria

1. Shadow rays produce correct hard shadows — voxels occluding light cast shadows on terrain.
2. Metallic voxels show reflections of nearby geometry.
3. No self-intersection artifacts (proper bias on secondary ray origins).
4. Shadow ray performance: < 2ms overhead at 1080p for a 128³ world.
5. Reflection/refraction are toggleable in debug overlay.
6. Max bounces = 0 disables reflections and refractions but shadow rays still run (they are always terminal and unaffected by `max_bounces`). Output matches Phase 1b + shadow rays.
7. Total internal reflection handled correctly for refraction.

## Implementation Note: Refraction Exit Rays

The `refract()` call in `trace_refraction()` handles air→material entry (`eta = 1.0 / ior`). For thin surfaces (e.g. glass panels), the refracted ray exits back into air, requiring a second refraction with `eta = ior` and a flipped normal. This two-interface refraction should be handled inside `trace_refraction()` by detecting when the secondary ray exits the material (SDF sign change from negative to positive) and applying a second Snell’s law step with inverted parameters.

## Dependencies

- Phase 1b (core ray march — provides the traversal function to reuse)
