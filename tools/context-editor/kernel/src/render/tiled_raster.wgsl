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
    glass_count:   u32,
}

struct ProjectedSplat {
    screen_min:         vec2f,
    screen_max:         vec2f,
    center_and_extent:  vec4f,   // xyz = world center, w = half_extent
    depth:              f32,
    material_packed:    u32,
    _pad:               vec2u,
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

struct GlassPanelData {
    center:           vec3f,
    corner_radius:    f32,
    half_size:        vec3f,
    ior:              f32,
    tint:             vec4f,
    blur_roughness:   f32,
    caustic_strength: f32,
    chromatic_spread: f32,
    _pad:             f32,
}

// ---------------------------------------------------------------------------
// Bindings
// ---------------------------------------------------------------------------

@group(0) @binding(0) var<storage, read> active_list: array<u32>;
@group(0) @binding(1) var<storage, read> projected:     array<ProjectedSplat>;
@group(0) @binding(2) var<storage, read> tile_data:     array<u32>; // packed: (offset << 12) | count
@group(0) @binding(3) var<uniform>       uniforms:      RasterUniforms;
@group(0) @binding(4) var<storage, read> glass_panels:  array<GlassPanelData>;

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
// Glass SDF helpers
// ---------------------------------------------------------------------------

/// Rounded-box SDF for a glass panel (in panel-local space).
fn sdf_rounded_box_glass(world_p: vec3f, panel: GlassPanelData) -> f32 {
    let p = world_p - panel.center;
    let q = abs(p) - panel.half_size + vec3f(panel.corner_radius);
    return length(max(q, vec3f(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0)
           - panel.corner_radius;
}

/// Numerical normal from SDF gradient (central differences).
fn glass_sdf_normal(p: vec3f, panel: GlassPanelData) -> vec3f {
    let e = 0.001;
    let dx = sdf_rounded_box_glass(p + vec3f(e, 0.0, 0.0), panel)
           - sdf_rounded_box_glass(p - vec3f(e, 0.0, 0.0), panel);
    let dy = sdf_rounded_box_glass(p + vec3f(0.0, e, 0.0), panel)
           - sdf_rounded_box_glass(p - vec3f(0.0, e, 0.0), panel);
    let dz = sdf_rounded_box_glass(p + vec3f(0.0, 0.0, e), panel)
           - sdf_rounded_box_glass(p - vec3f(0.0, 0.0, e), panel);
    return normalize(vec3f(dx, dy, dz));
}

/// Snell's law refraction with total-internal-reflection fallback.
fn refract_ray(incident: vec3f, normal: vec3f, eta: f32) -> vec3f {
    let cos_i  = -dot(incident, normal);
    let sin2_t = eta * eta * (1.0 - cos_i * cos_i);
    // Total internal reflection
    if sin2_t > 1.0 {
        return reflect(incident, normal);
    }
    let cos_t = sqrt(1.0 - sin2_t);
    return eta * incident + (eta * cos_i - cos_t) * normal;
}

/// Ray-AABB intersection for a glass panel; returns hit distance or -1.
fn ray_glass_hit(ro: vec3f, rd: vec3f, panel: GlassPanelData) -> f32 {
    let he     = panel.half_size;
    let inv_rd = 1.0 / rd;
    let t1     = (panel.center - he - ro) * inv_rd;
    let t2     = (panel.center + he - ro) * inv_rd;
    let t_min  = max(max(min(t1.x, t2.x), min(t1.y, t2.y)), min(t1.z, t2.z));
    let t_max  = min(min(max(t1.x, t2.x), max(t1.y, t2.y)), max(t1.z, t2.z));
    if t_max < 0.0 || t_min > t_max { return -1.0; }
    return select(t_min, 0.0, t_min < 0.0);
}

// ---------------------------------------------------------------------------
// Glass VFX helpers
// ---------------------------------------------------------------------------

/// Chromatic aberration — per-channel tint shift from refraction dispersion.
/// Diverges R/G/B slightly based on chromatic_spread and distortion magnitude.
fn apply_chromatic_aberration(
    tint: vec4f,
    distortion: vec2f,
    chromatic_spread: f32,
) -> vec4f {
    let chroma = length(distortion) * chromatic_spread;
    return vec4f(
        tint.r * (1.0 + chroma * 0.3),
        tint.g,
        tint.b * (1.0 - chroma * 0.3),
        tint.a,
    );
}

/// Pseudo-caustic brightness from refraction divergence.
fn compute_caustic(distortion: vec2f, caustic_strength: f32) -> f32 {
    // Analytical approximation — avoids fwidth() which requires uniform control flow.
    return length(distortion) * caustic_strength * 20.0;
}

/// Curvature-adaptive roughness — glass edges appear more diffuse than
/// the flat center. Estimates curvature from SDF gradient instead of
/// using fwidth() which requires uniform control flow.
fn curvature_adaptive_roughness(
    hit: vec3f,
    panel: GlassPanelData,
    base_roughness: f32,
) -> f32 {
    // Estimate curvature via the SDF value near the surface — closer to
    // the rounded edges means higher curvature.
    let d = abs(sdf_rounded_box_glass(hit, panel));
    let curvature = clamp(1.0 - d / max(panel.corner_radius, 0.001), 0.0, 1.0);
    return clamp(base_roughness + curvature * 0.3, 0.0, 1.0);
}

// ---------------------------------------------------------------------------
// Fragment shader
// ---------------------------------------------------------------------------

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    // Ray reconstruction — Bevy uses infinite reverse-Z projection.
    // Use camera_pos as origin; unproject two finite clip-z values for direction.
    let ndc = in.uv * 2.0 - 1.0;
    let clip_0 = vec4f(ndc.x, -ndc.y, 1.0, 1.0);   // near plane (depth = 1.0)
    let clip_1 = vec4f(ndc.x, -ndc.y, 0.5, 1.0);   // finite midpoint
    let world_0 = uniforms.inv_view_proj * clip_0;
    let world_1 = uniforms.inv_view_proj * clip_1;
    let p0 = world_0.xyz / world_0.w;
    let p1 = world_1.xyz / world_1.w;
    let ray_origin = uniforms.camera_pos;
    var ray_dir    = normalize(p1 - p0);

    // ---- Glass refraction pre-pass ----
    var glass_tint  = vec4f(1.0);
    var adjusted_uv = in.uv;
    for (var g = 0u; g < uniforms.glass_count; g++) {
        let panel = glass_panels[g];
        let t = ray_glass_hit(ray_origin, ray_dir, panel);
        if t >= 0.0 {
            let hit = ray_origin + ray_dir * t;
            let d   = sdf_rounded_box_glass(hit, panel);
            if d <= 0.0 {
                let normal    = glass_sdf_normal(hit, panel);
                let eta       = 1.0 / panel.ior;
                let refracted = refract_ray(ray_dir, normal, eta);
                let distortion = (refracted.xy - ray_dir.xy) * 0.05;
                adjusted_uv = adjusted_uv + distortion;
                ray_dir     = refracted;

                // Curvature-adaptive roughness
                let eff_roughness = curvature_adaptive_roughness(
                    hit, panel, panel.blur_roughness);

                if eff_roughness < 0.01 {
                    glass_tint = apply_chromatic_aberration(
                        glass_tint * panel.tint, distortion, panel.chromatic_spread);
                } else {
                    let frost_atten = 1.0 - eff_roughness * 0.3;
                    glass_tint *= panel.tint * vec4f(vec3f(frost_atten), 1.0);
                }

                let caustic = compute_caustic(distortion, panel.caustic_strength);
                glass_tint = vec4f(
                    glass_tint.rgb + caustic * uniforms.light_color, glass_tint.a);
            }
        }
    }

    // ---- Tile lookup (from possibly refracted UV) ----
    let px = adjusted_uv * uniforms.resolution;

    let tile_x   = u32(clamp(px.x, 0.0, uniforms.resolution.x - 1.0)) / TILE_SIZE;
    let tile_y   = u32(clamp(px.y, 0.0, uniforms.resolution.y - 1.0)) / TILE_SIZE;
    let tile_idx = tile_y * uniforms.grid_width + tile_x;

    let packed     = tile_data[tile_idx];
    let tile_count  = packed & 0xFFFu;
    let tile_offset = packed >> 12u;

    // Empty tile → background (tinted by glass if applicable)
    if tile_count == 0u {
        return vec4f(vec3f(0.1, 0.1, 0.12) * glass_tint.rgb, 1.0);
    }

    var color           = vec3f(0.0);
    var remaining_alpha = 1.0;

    for (var i = 0u; i < tile_count; i++) {
        let splat_idx = active_list[tile_offset + i];
        let s         = projected[splat_idx];

        // Skip if pixel outside splat's screen AABB
        if px.x < s.screen_min.x || px.x > s.screen_max.x ||
           px.y < s.screen_min.y || px.y > s.screen_max.y {
            continue;
        }

        let center_ws  = s.center_and_extent.xyz;
        let half_ext_f = s.center_and_extent.w;

        // Ray-box SDF evaluation
        let local_pos = ray_box_closest_point(
            ray_origin, ray_dir, center_ws, half_ext_f,
        );
        let d = sd_box(local_pos - center_ws, vec3f(half_ext_f));

        // Anti-aliased coverage — analytical pixel footprint (avoids fwidth
        // in non-uniform control flow)
        let hit_dist = length(local_pos - ray_origin);
        let fw = hit_dist / max(uniforms.resolution.y, 1.0);
        let alpha = (1.0 - smoothstep(-fw, fw, d)) * remaining_alpha;
        if alpha < 1.0 / 255.0 { continue; }

        // Lighting
        let mat      = unpack_material(s.material_packed);
        let normal   = box_normal(local_pos - center_ws, vec3f(half_ext_f));
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

    // Apply glass tint to the composited result
    color *= glass_tint.rgb;

    return vec4f(color, 1.0);
}
