// Z-Prepass Compute Shader — Phase 5B (ticket 283c2bc7)
//
// Two entry points:
//
//  clear_depth     — @workgroup_size(256,1,1)
//                    Writes f32 +infinity to every pixel slot in `depth_prepass`.
//                    Dispatch: ceil(width * height / 256) workgroups.
//
//  z_prepass_main  — @workgroup_size(8,8,1)
//                    One thread per pixel. Scans the first MAX_PREPASS_SPLATS
//                    front-to-back sorted splats in the tile. On the first
//                    opaque box hit (alpha > 0.95) writes the view-space depth
//                    to `depth_prepass[px_idx]` and returns.
//                    Dispatch: ceil(width/8) × ceil(height/8) workgroups.
//
// The depth written here is later read by the fragment shader in
// tiled_raster.wgsl to skip splats behind the first opaque surface,
// eliminating z-fighting between adjacent terrain voxels.

// ---------------------------------------------------------------------------
// Structs (must match tiled_raster.wgsl / RasterUniforms)
// ---------------------------------------------------------------------------

struct RasterUniforms {
    inv_view_proj: mat4x4f,
    camera_pos:    vec3f,
    _pad0:         f32,
    resolution:    vec2f,
    grid_width:    u32,
    max_depth:     f32,
    light_dir:     vec3f,
    _pad1:         f32,
    light_color:   vec3f,
    glass_count:   u32,
}

struct ProjectedSplat {
    screen_min:        vec2f,
    screen_max:        vec2f,
    center_and_extent: vec4f,  // xyz = world center, w = half_extent
    depth:             f32,
    material_packed:   u32,
    _pad:              vec2u,
}

// ---------------------------------------------------------------------------
// Bindings (group 0, must match z_prepass_bind_group_layout_descriptor in .rs)
// ---------------------------------------------------------------------------

@group(0) @binding(0) var<storage, read>       active_list:   array<u32>;
@group(0) @binding(1) var<storage, read>       projected:     array<ProjectedSplat>;
@group(0) @binding(2) var<storage, read>       tile_data:     array<u32>; // [offset, count] pairs per tile
@group(0) @binding(3) var<uniform>             uniforms:      RasterUniforms;
@group(0) @binding(4) var<storage, read_write> depth_prepass: array<f32>;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const TILE_SIZE: u32         = 16u;
const MAX_PREPASS_SPLATS: u32 = 8u;
/// Sentinel depth meaning "no opaque hit" — much larger than any real scene depth.
/// Avoids bitcast which is not supported as a naga constant expression.
const DEPTH_INFINITY: f32     = 1.0e20;

// ---------------------------------------------------------------------------
// SDF helpers (duplicated from tiled_raster.wgsl — avoids cross-file import)
// ---------------------------------------------------------------------------

fn sd_box(p: vec3f, half_ext: vec3f) -> f32 {
    let q = abs(p) - half_ext;
    return length(max(q, vec3f(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0);
}

/// Closest point on (or inside) an AABB along a ray.
fn ray_box_closest_point(ro: vec3f, rd: vec3f, center: vec3f, half_ext: f32) -> vec3f {
    let he     = vec3f(half_ext);
    let inv_rd = 1.0 / rd;
    let t1     = (center - he - ro) * inv_rd;
    let t2     = (center + he - ro) * inv_rd;
    let t_min  = max(max(min(t1.x, t2.x), min(t1.y, t2.y)), min(t1.z, t2.z));
    let t      = select(t_min, 0.0, t_min < 0.0);
    return ro + rd * max(t, 0.0);
}

// ---------------------------------------------------------------------------
// Entry point 1: clear_depth
// ---------------------------------------------------------------------------

/// Reset every depth_prepass slot to +infinity before the prepass runs.
/// @workgroup_size(256): one linear thread per pixel.
@compute @workgroup_size(256, 1, 1)
fn clear_depth(@builtin(global_invocation_id) id: vec3<u32>) {
    let total = u32(uniforms.resolution.x) * u32(uniforms.resolution.y);
    if id.x >= total { return; }
    depth_prepass[id.x] = DEPTH_INFINITY;
}

// ---------------------------------------------------------------------------
// Entry point 2: z_prepass_main
// ---------------------------------------------------------------------------

/// Find the nearest opaque box-splat depth per pixel.
/// @workgroup_size(8,8): one thread per pixel tile in an 8×8 block.
@compute @workgroup_size(8, 8, 1)
fn z_prepass_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let res_x = u32(uniforms.resolution.x);
    let res_y = u32(uniforms.resolution.y);
    if gid.x >= res_x || gid.y >= res_y { return; }

    let px = vec2f(f32(gid.x) + 0.5, f32(gid.y) + 0.5);

    // Ray reconstruction — same logic as tiled_raster.wgsl fs_main.
    // Bevy uses infinite reverse-Z; unproject two finite clip-z values.
    let ndc    = px / uniforms.resolution * 2.0 - 1.0;
    let clip_0 = vec4f(ndc.x, -ndc.y, 1.0, 1.0);  // near plane
    let clip_1 = vec4f(ndc.x, -ndc.y, 0.5, 1.0);  // finite midpoint
    let w0     = uniforms.inv_view_proj * clip_0;
    let w1     = uniforms.inv_view_proj * clip_1;
    let p0     = w0.xyz / w0.w;
    let p1     = w1.xyz / w1.w;
    let ray_origin = uniforms.camera_pos;
    let ray_dir    = normalize(p1 - p0);

    // Tile index for this pixel.
    let tile_x   = gid.x / TILE_SIZE;
    let tile_y   = gid.y / TILE_SIZE;
    let tile_idx = tile_y * uniforms.grid_width + tile_x;

    // tile_data layout: [offset at 2i, count at 2i+1] (8 bytes per tile)
    let tile_offset = tile_data[tile_idx * 2u];
    let tile_count  = tile_data[tile_idx * 2u + 1u];

    let px_idx = gid.y * res_x + gid.x;
    let limit  = min(tile_count, MAX_PREPASS_SPLATS);

    for (var i = 0u; i < limit; i++) {
        let splat_idx = active_list[tile_offset + i];
        let s         = projected[splat_idx];

        // Skip if pixel is outside this splat's screen AABB.
        if px.x < s.screen_min.x || px.x > s.screen_max.x ||
           px.y < s.screen_min.y || px.y > s.screen_max.y {
            continue;
        }

        let center_ws = s.center_and_extent.xyz;
        let half_ext  = s.center_and_extent.w;

        // Cheap box SDF regardless of actual sdf_type.
        let local_pos = ray_box_closest_point(ray_origin, ray_dir, center_ws, half_ext);
        let p_local   = local_pos - center_ws;
        let d         = sd_box(p_local, vec3f(half_ext));

        let hit_dist = length(local_pos - ray_origin);
        let fw       = hit_dist / max(uniforms.resolution.y, 1.0);
        let alpha    = 1.0 - smoothstep(-fw, fw, d);

        if alpha > 0.95 {
            depth_prepass[px_idx] = hit_dist;
            return;
        }
    }
    // No opaque hit among the first MAX_PREPASS_SPLATS — leave as F32_INF.
}
