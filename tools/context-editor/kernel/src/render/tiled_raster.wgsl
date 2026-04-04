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
@group(0) @binding(2) var<storage, read> tile_data:     array<u32>; // [offset, count] per tile (2 u32s each)
@group(0) @binding(3) var<uniform>       uniforms:      RasterUniforms;
@group(0) @binding(4) var<storage, read> glass_panels:  array<GlassPanelData>;
@group(0) @binding(5) var<storage, read> octree:        array<vec2u>; // OctreeNode: (child_pointer, color_data)
@group(0) @binding(6) var<storage, read> depth_prepass: array<f32>; // per-pixel opaque depth from ZPrepassNode

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const PI: f32       = 3.14159265359;
const TILE_SIZE: u32 = 16u;
/// Mask for unpacking the projected index from packed active_list entries.
const INDEX_MASK: u32 = 0x1FFFFFu;

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



fn box_normal(p_local: vec3f, half_ext: vec3f) -> vec3f {
    // Ein sehr kleiner Epsilon-Wert für die Gradienten-Berechnung
    let e = vec2f(0.001, 0.0);
    
    // Wir berechnen den Gradienten der sd_box Funktion an Punkt p
    // Das entspricht der Richtung des steilsten Anstiegs der Distanz
    let n = vec3f(
        sd_box(p_local + e.xyy, half_ext) - sd_box(p_local - e.xyy, half_ext),
        sd_box(p_local + e.yxy, half_ext) - sd_box(p_local - e.yxy, half_ext),
        sd_box(p_local + e.yyx, half_ext) - sd_box(p_local - e.yyx, half_ext)
    );
    
    return normalize(n);
}

// ---------------------------------------------------------------------------
// SDF type dispatch — bits 30-31 of material_packed
// 0b00 = Box  (default, fastest)
// 0b01 = Sphere (inscribed in voxel, 90% radius)
// 0b10 = SVO-Sampled (box fallback until LOD splats are added)
// 0b11 = Torus / Procedural
// ---------------------------------------------------------------------------

fn sdf_type_from_mp(material_packed: u32) -> u32 {
    return (material_packed >> 30u) & 3u;
}

fn sd_sphere(p: vec3f, r: f32) -> f32 {
    return length(p) - r;
}

// Torus in xy-plane (vertical ring / wheel orientation) — visible as a ring
// from the default forward-facing camera.
fn sd_torus(p: vec3f, major_r: f32, minor_r: f32) -> f32 {
    let q = vec2f(length(p.xy) - major_r, p.z);
    return length(q) - minor_r;
}

/// Dispatch SDF evaluation to the correct shape function.
fn sdf_eval(p: vec3f, half_ext: f32, sdf_type: u32) -> f32 {
    if sdf_type == 1u {
        return sd_sphere(p, half_ext * 0.9);
    }
    if sdf_type == 3u {
        return sd_torus(p, half_ext * 0.6, half_ext * 0.25);
    }
    // Box (type 0, 2 = svo-sampled fallback, or unknown)
    return sd_box(p, vec3f(half_ext));
}

/// Analytical surface normal for each SDF type.
fn sdf_normal_type(p_local: vec3f, half_ext: f32, sdf_type: u32) -> vec3f {
    if sdf_type == 1u {
        // Sphere: radial direction
        return normalize(p_local);
    }
    if sdf_type == 3u {
        // Torus analytical gradient (ring in XY plane — matches sd_torus).
        let major_r = half_ext * 0.6;
        let xy_len  = length(p_local.xy);
        let xy_hat  = p_local.xy / max(xy_len, 0.0001);
        let q       = vec2f(xy_len - major_r, p_local.z);
        let q_hat   = normalize(q);
        return normalize(vec3f(xy_hat.x * q_hat.x, xy_hat.y * q_hat.x, q_hat.y));
    }
    // Box and all fallback types: use gradient-based box normal
    return box_normal(p_local, vec3f(half_ext));
}

/// Closest point on the surface along the ray, per SDF type.
fn ray_surface_closest_point(
    ro: vec3f, rd: vec3f, center: vec3f, half_ext: f32, sdf_type: u32,
) -> vec3f {
    if sdf_type == 1u {
        // Exact ray-sphere intersection
        let r    = half_ext * 0.9;
        let oc   = ro - center;
        let b    = dot(oc, rd);
        let c    = dot(oc, oc) - r * r;
        let disc = b * b - c;
        if disc >= 0.0 {
            let sq = sqrt(disc);
            let t1 = -b - sq;
            let t2 = -b + sq;
            // Ray starts inside sphere → t=0 (current position is inside)
            if t1 < 0.0 && t2 > 0.0 { return ro; }
            return ro + rd * max(t1, 0.0);
        }
        // Miss → fall through to box closest point
    }
    if sdf_type == 3u {
        // Sphere-trace the torus SDF inside the voxel AABB (rd is normalized).
        // A single box-face sample is always outside the torus tube, so we
        // march along the ray until we reach the surface or exit the box.
        let he     = vec3f(half_ext);
        let inv_rd = 1.0 / rd;
        let t1v    = (center - he - ro) * inv_rd;
        let t2v    = (center + he - ro) * inv_rd;
        let t_enter = max(max(min(t1v.x, t2v.x), min(t1v.y, t2v.y)), min(t1v.z, t2v.z));
        let t_exit  = min(min(max(t1v.x, t2v.x), max(t1v.y, t2v.y)), max(t1v.z, t2v.z));
        let major_r = half_ext * 0.6;
        let minor_r = half_ext * 0.25;
        var t = max(t_enter, 0.0);
        for (var i = 0; i < 8; i++) {
            let d = sd_torus(ro + rd * t - center, major_r, minor_r);
            if d < half_ext * 1e-3 { break; }
            t += d;
            if t >= t_exit { break; }
        }
        return ro + rd * clamp(t, max(t_enter, 0.0), t_exit);
    }
    return ray_box_closest_point(ro, rd, center, half_ext);
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

    let tile_offset = tile_data[tile_idx * 2u];
    let tile_count  = tile_data[tile_idx * 2u + 1u];

    // Empty tile → transparent (let underlying wireframe / MainPass show through)
    if tile_count == 0u {
        return vec4f(0.0);
    }

    var color           = vec3f(0.0);
    var remaining_alpha = 1.0;

    // Depth cull: prepass wrote the nearest opaque hit distance for this pixel.
    // Any splat further than (prepass_depth + epsilon) is behind solid terrain
    // and can be skipped, eliminating z-fighting.
    let px_idx       = u32(px.y) * u32(uniforms.resolution.x) + u32(px.x);
    let prepass_depth = depth_prepass[px_idx];

    for (var i = 0u; i < tile_count; i++) {
        let splat_idx = active_list[tile_offset + i] & INDEX_MASK;
        let s         = projected[splat_idx];

        // Skip if pixel outside splat's screen AABB
        if px.x < s.screen_min.x || px.x > s.screen_max.x ||
           px.y < s.screen_min.y || px.y > s.screen_max.y {
            continue;
        }

        let center_ws  = s.center_and_extent.xyz;
        let half_ext_f = s.center_and_extent.w;

        // SDF type dispatch — unified path for all SDF types.
        let sdf_tp    = sdf_type_from_mp(s.material_packed);
        let local_pos = ray_surface_closest_point(
            ray_origin, ray_dir, center_ws, half_ext_f, sdf_tp,
        );
        let p_local = local_pos - center_ws;
        let d       = sdf_eval(p_local, half_ext_f, sdf_tp);
        let hit_dist = length(local_pos - ray_origin);
        let fw       = hit_dist / max(uniforms.resolution.y, 1.0);
        // AA fringe on the OUTSIDE only: d <= 0 (on/inside surface) → full
        // opacity, 0 < d < fw → smooth silhouette falloff.
        let alpha = (1.0 - smoothstep(0.0, fw, d)) * remaining_alpha;
        if alpha < 1.0 / 255.0 { continue; }
        let normal = sdf_normal_type(p_local, half_ext_f, sdf_tp);

        // Depth cull: skip anything behind the prepass opaque surface.
        let has_prepass_hit = prepass_depth > 0.0 && prepass_depth < 1.0e20;
        if has_prepass_hit && hit_dist > prepass_depth + 0.05 { continue; }

        // Lighting
        let mat      = unpack_material(s.material_packed);
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

    // Apply glass tint to the composited result (premultiplied alpha)
    color *= glass_tint.rgb;

    // Output premultiplied-alpha: (color already weighted by coverage, alpha = total opacity)
    let opacity = 1.0 - remaining_alpha;
    return vec4f(color, opacity);
}
