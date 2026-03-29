// Tiled Forward+ Rasterizer — vertex + fragment shaders (T6d Phase 2)
//
// A fullscreen-triangle pass that, for each pixel:
//   1. Looks up the tile (offset, count) from the sorted splat data
//   2. Loops over each splat in the tile
//   3. Evaluates a ray-box SDF for solid voxel edges
//   4. Applies Cook-Torrance/GGX PBR lighting (from T6e)
//   5. Composites front-to-back with alpha blending
//
// Early-out: once remaining_alpha < 0.01 the pixel is saturated.

// ---------------------------------------------------------------------------
// Structs
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
    _pad2:         f32,
}

struct ProjectedSplat {
    screen_min:       vec2f,
    screen_max:       vec2f,
    center_ws:        vec3f,
    half_extent:      f32,
    depth:            f32,
    material_packed:  u32,
    _pad:             vec2u,
}

struct Material {
    base_color: vec3f,
    roughness:  f32,
    metallic:   f32,
}

struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0)       uv:       vec2f,
}

// ---------------------------------------------------------------------------
// Bindings
// ---------------------------------------------------------------------------

@group(0) @binding(0) var<storage, read> sorted_values: array<u32>;
@group(0) @binding(1) var<storage, read> projected:     array<ProjectedSplat>;
@group(0) @binding(2) var<storage, read> tile_data:     array<u32>; // [off, cnt, off, cnt, …]
@group(0) @binding(3) var<uniform>       uniforms:      RasterUniforms;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const PI: f32       = 3.14159265359;
const TILE_SIZE: u32 = 16u;

// ---------------------------------------------------------------------------
// Vertex shader — fullscreen triangle (3 vertices, no VBO)
// ---------------------------------------------------------------------------

@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> VertexOutput {
    // Single triangle covering [-1,1]×[-1,1] clip space
    let positions = array<vec2f, 3>(
        vec2f(-1.0, -1.0),
        vec2f( 3.0, -1.0),
        vec2f(-1.0,  3.0),
    );
    var out: VertexOutput;
    out.position = vec4f(positions[vid], 0.0, 1.0);
    // UV in [0,1] — convert to pixel coords in fragment
    out.uv = positions[vid] * vec2f(0.5, -0.5) + 0.5;
    return out;
}

// ---------------------------------------------------------------------------
// PBR helpers (inlined from pbr_material.wgsl / T6e)
// ---------------------------------------------------------------------------

fn unpack_material(packed: u32) -> Material {
    let r = f32(packed & 0xFFu) / 255.0;
    let g = f32((packed >> 8u) & 0xFFu) / 255.0;
    let b = f32((packed >> 16u) & 0xFFu) / 255.0;
    let roughness = f32((packed >> 24u) & 0x1Fu) / 31.0;
    let metallic  = f32((packed >> 29u) & 1u);
    return Material(vec3f(r, g, b), roughness, metallic);
}

fn ggx_distribution(n_dot_h: f32, alpha2: f32) -> f32 {
    let denom = n_dot_h * n_dot_h * (alpha2 - 1.0) + 1.0;
    return alpha2 / (PI * denom * denom);
}

fn fresnel_schlick(cos_theta: f32, f0: vec3f) -> vec3f {
    return f0 + (vec3f(1.0) - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

fn geometry_schlick_ggx(n_dot: f32, k: f32) -> f32 {
    return n_dot / (n_dot * (1.0 - k) + k);
}

fn geometry_smith(n_dot_v: f32, n_dot_l: f32, alpha: f32) -> f32 {
    let k = (alpha + 1.0) * (alpha + 1.0) / 8.0;
    return geometry_schlick_ggx(n_dot_v, k) * geometry_schlick_ggx(n_dot_l, k);
}

fn evaluate_pbr(
    mat: Material,
    normal: vec3f,
    view_dir: vec3f,
    light_dir: vec3f,
    light_color: vec3f,
) -> vec3f {
    let h       = normalize(view_dir + light_dir);
    let n_dot_l = max(dot(normal, light_dir), 0.0);
    let n_dot_v = max(dot(normal, view_dir), 0.001);
    let n_dot_h = max(dot(normal, h), 0.0);
    let v_dot_h = max(dot(view_dir, h), 0.0);

    let f0     = mix(vec3f(0.04), mat.base_color, mat.metallic);
    let alpha  = mat.roughness * mat.roughness;
    let alpha2 = alpha * alpha;
    let d      = ggx_distribution(n_dot_h, alpha2);
    let f      = fresnel_schlick(v_dot_h, f0);
    let g      = geometry_smith(n_dot_v, n_dot_l, alpha);

    let specular = (d * f * g) / max(4.0 * n_dot_v * n_dot_l, 0.001);
    let k_s      = f;
    let k_d      = (vec3f(1.0) - k_s) * (1.0 - mat.metallic);
    let diffuse  = k_d * mat.base_color / PI;

    return (diffuse + specular) * light_color * n_dot_l;
}

// ---------------------------------------------------------------------------
// SDF helpers
// ---------------------------------------------------------------------------

/// Signed distance to axis-aligned box centered at origin.
fn sd_box(p: vec3f, half_ext: vec3f) -> f32 {
    let q = abs(p) - half_ext;
    return length(max(q, vec3f(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0);
}

/// Closest point on (or inside) an AABB along a ray.
fn ray_box_closest_point(ro: vec3f, rd: vec3f, center: vec3f, half_ext: f32) -> vec3f {
    let he      = vec3f(half_ext);
    let inv_rd  = 1.0 / rd;
    let t1      = (center - he - ro) * inv_rd;
    let t2      = (center + he - ro) * inv_rd;
    let t_min   = max(max(min(t1.x, t2.x), min(t1.y, t2.y)), min(t1.z, t2.z));
    let t_max   = min(min(max(t1.x, t2.x), max(t1.y, t2.y)), max(t1.z, t2.z));
    // Miss → clamp to tmin; inside → t=0
    let t       = select(t_min, 0.0, t_min < 0.0);
    return ro + rd * max(t, 0.0);
}

/// Approximate face-normal from point-on-surface.
fn box_normal(p: vec3f, half_ext: vec3f) -> vec3f {
    let d   = abs(p) - half_ext;
    let eps = vec3f(0.001);
    return normalize(sign(p) * step(d.yzx, d.xyz) * step(d.zxy, d.xyz));
}

// ---------------------------------------------------------------------------
// Fragment shader
// ---------------------------------------------------------------------------

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let px = in.uv * uniforms.resolution;

    let tile_x   = u32(px.x) / TILE_SIZE;
    let tile_y   = u32(px.y) / TILE_SIZE;
    let tile_idx = tile_y * uniforms.grid_width + tile_x;

    let tile_offset = tile_data[tile_idx * 2u];
    let tile_count  = tile_data[tile_idx * 2u + 1u];

    // Empty tile → background
    if tile_count == 0u {
        return vec4f(0.1, 0.1, 0.12, 1.0);
    }

    // Reconstruct world-space ray through this pixel
    let ndc       = in.uv * 2.0 - 1.0;
    let clip_near = vec4f(ndc.x, -ndc.y, 0.0, 1.0);
    let clip_far  = vec4f(ndc.x, -ndc.y, 1.0, 1.0);
    let world_near = uniforms.inv_view_proj * clip_near;
    let world_far  = uniforms.inv_view_proj * clip_far;
    let ray_origin = world_near.xyz / world_near.w;
    let ray_dir    = normalize(world_far.xyz / world_far.w - ray_origin);

    var color           = vec3f(0.0);
    var remaining_alpha = 1.0;

    for (var i = 0u; i < tile_count; i++) {
        let splat_idx = sorted_values[tile_offset + i];
        let s         = projected[splat_idx];

        // Skip if pixel outside splat's screen AABB
        if px.x < s.screen_min.x || px.x > s.screen_max.x ||
           px.y < s.screen_min.y || px.y > s.screen_max.y {
            continue;
        }

        // Ray-box SDF evaluation
        let local_pos = ray_box_closest_point(
            ray_origin, ray_dir, s.center_ws, s.half_extent,
        );
        let d = sd_box(local_pos - s.center_ws, vec3f(s.half_extent));

        // Anti-aliased coverage via screen-space derivative
        let fw    = fwidth(d);
        let alpha = (1.0 - smoothstep(-fw, fw, d)) * remaining_alpha;
        if alpha < 1.0 / 255.0 { continue; }

        // Lighting
        let mat      = unpack_material(s.material_packed);
        let normal   = box_normal(local_pos - s.center_ws, vec3f(s.half_extent));
        let view_dir = normalize(uniforms.camera_pos - local_pos);
        let pbr      = evaluate_pbr(mat, normal, view_dir,
                                    uniforms.light_dir, uniforms.light_color);
        let ambient  = mat.base_color * 0.15;
        let lit      = pbr + ambient;

        color           += lit * alpha;
        remaining_alpha -= alpha;

        // Early-out: pixel saturated
        if remaining_alpha < 0.01 { break; }
    }

    // Fill remaining alpha with background
    color += vec3f(0.1, 0.1, 0.12) * remaining_alpha;

    return vec4f(color, 1.0);
}
