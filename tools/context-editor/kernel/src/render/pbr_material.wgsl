// PBR Material System — shared shader utilities (T6e)
//
// Provides Cook-Torrance/GGX BRDF evaluation, compact u32 material encoding,
// soft shadows, and ambient occlusion for the voxel splatting pipeline.
//
// Included by the tiled rasterizer (T6d) per-pixel.

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const PI: f32 = 3.14159265359;

// ---------------------------------------------------------------------------
// Compact Material Encoding (u32)
// ---------------------------------------------------------------------------
//
// Bit layout of OctreeNode.color_data:
//   Bits  0–7:  R (8 bits, 0–255)
//   Bits  8–15: G (8 bits, 0–255)
//   Bits 16–23: B (8 bits, 0–255)
//   Bits 24–28: Roughness (5 bits, 0–31 → 0.0–1.0)
//   Bit  29:    Metallic (1 bit, 0 = dielectric, 1 = metallic)
//   Bits 30–31: Reserved (emission flag, translucency hint)

struct Material {
    base_color: vec3f,
    roughness:  f32,
    metallic:   f32,
}

fn unpack_material(packed: u32) -> Material {
    let r = f32(packed & 0xFFu) / 255.0;
    let g = f32((packed >> 8u) & 0xFFu) / 255.0;
    let b = f32((packed >> 16u) & 0xFFu) / 255.0;
    let roughness = f32((packed >> 24u) & 0x1Fu) / 31.0;
    let metallic = f32((packed >> 29u) & 1u);
    return Material(vec3f(r, g, b), roughness, metallic);
}

fn pack_material(base_color: vec3f, roughness: f32, metallic: bool) -> u32 {
    let r = u32(clamp(base_color.r * 255.0, 0.0, 255.0));
    let g = u32(clamp(base_color.g * 255.0, 0.0, 255.0));
    let b = u32(clamp(base_color.b * 255.0, 0.0, 255.0));
    let rough_q = u32(clamp(roughness * 31.0, 0.0, 31.0));
    let metal_bit = select(0u, 1u, metallic);
    return r | (g << 8u) | (b << 16u) | (rough_q << 24u) | (metal_bit << 29u);
}

// ---------------------------------------------------------------------------
// Cook-Torrance / GGX BRDF
// ---------------------------------------------------------------------------

/// GGX/Trowbridge-Reitz normal distribution function.
fn ggx_distribution(n_dot_h: f32, alpha2: f32) -> f32 {
    let denom = n_dot_h * n_dot_h * (alpha2 - 1.0) + 1.0;
    return alpha2 / (PI * denom * denom);
}

/// Schlick's Fresnel approximation.
fn fresnel_schlick(cos_theta: f32, f0: vec3f) -> vec3f {
    return f0 + (vec3f(1.0) - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

/// Schlick-GGX geometry attenuation for a single direction.
fn geometry_schlick_ggx(n_dot: f32, k: f32) -> f32 {
    return n_dot / (n_dot * (1.0 - k) + k);
}

/// Smith's geometry function combining view and light directions.
fn geometry_smith(n_dot_v: f32, n_dot_l: f32, alpha: f32) -> f32 {
    let k = (alpha + 1.0) * (alpha + 1.0) / 8.0;  // direct lighting remapping
    return geometry_schlick_ggx(n_dot_v, k) * geometry_schlick_ggx(n_dot_l, k);
}

/// Full Cook-Torrance BRDF evaluation for a single directional light.
///
/// Returns the outgoing radiance contribution from `light_dir`.
fn evaluate_pbr(
    mat: Material,
    normal: vec3f,
    view_dir: vec3f,
    light_dir: vec3f,
    light_color: vec3f,
) -> vec3f {
    let h = normalize(view_dir + light_dir);
    let n_dot_l = max(dot(normal, light_dir), 0.0);
    let n_dot_v = max(dot(normal, view_dir), 0.001);
    let n_dot_h = max(dot(normal, h), 0.0);
    let v_dot_h = max(dot(view_dir, h), 0.0);

    // F0: reflectance at normal incidence
    // Dielectrics: 0.04 (typical), Metals: base_color
    let f0 = mix(vec3f(0.04), mat.base_color, mat.metallic);

    // D: GGX normal distribution
    let alpha = mat.roughness * mat.roughness;
    let alpha2 = alpha * alpha;
    let d = ggx_distribution(n_dot_h, alpha2);

    // F: Fresnel
    let f = fresnel_schlick(v_dot_h, f0);

    // G: Geometry
    let g = geometry_smith(n_dot_v, n_dot_l, alpha);

    // Specular BRDF: DFG / (4 * NdotV * NdotL)
    let specular = (d * f * g) / max(4.0 * n_dot_v * n_dot_l, 0.001);

    // Diffuse: Lambertian with energy conservation
    let k_s = f;
    let k_d = (vec3f(1.0) - k_s) * (1.0 - mat.metallic);
    let diffuse = k_d * mat.base_color / PI;

    return (diffuse + specular) * light_color * n_dot_l;
}

// ---------------------------------------------------------------------------
// Image-Based Lighting (IBL) — split-sum approximation
// ---------------------------------------------------------------------------

// Requires BRDF LUT texture bound at group(1) binding(0,1)
// These are called from the rasterizer when IBL is enabled.

fn evaluate_ibl(
    mat: Material,
    normal: vec3f,
    view_dir: vec3f,
    env_irradiance: vec3f,
    env_prefiltered: vec3f,
    brdf_lut_sample: vec2f,  // textureSample(brdf_lut, sampler, vec2f(NdotV, roughness))
) -> vec3f {
    let n_dot_v = max(dot(normal, view_dir), 0.0);
    let f0 = mix(vec3f(0.04), mat.base_color, mat.metallic);

    let specular = env_prefiltered * (f0 * brdf_lut_sample.x + brdf_lut_sample.y);

    let k_d = (1.0 - mat.metallic);
    let diffuse = k_d * mat.base_color * env_irradiance;

    return diffuse + specular;
}

// ---------------------------------------------------------------------------
// Soft Shadows (SDF ray-march approximation)
// ---------------------------------------------------------------------------

// `scene_sdf` is expected to be defined by the including shader — it queries
// the nearest voxel distance at a world-space point.

fn soft_shadow_factor(
    ray_origin: vec3f,
    light_dir: vec3f,
    max_dist: f32,
    k: f32,  // softness: 8.0 = sharp, 2.0 = very soft
    // scene_sdf: provided externally
) -> f32 {
    var shadow = 1.0;
    var t = 0.02;  // bias to avoid self-intersection
    for (var i = 0u; i < 32u; i++) {
        let p = ray_origin + light_dir * t;
        let d = scene_sdf(p);
        if d < 0.001 {
            return 0.0;  // fully occluded
        }
        shadow = min(shadow, k * d / t);
        t += d;
        if t > max_dist {
            break;
        }
    }
    return clamp(shadow, 0.0, 1.0);
}

// ---------------------------------------------------------------------------
// Ambient Occlusion (short-range SDF probe)
// ---------------------------------------------------------------------------

fn sdf_ao(pos: vec3f, normal: vec3f) -> f32 {
    var ao = 0.0;
    var scale = 1.0;
    for (var i = 1u; i <= 5u; i++) {
        let sample_pos = pos + normal * f32(i) * 0.05;
        let d = scene_sdf(sample_pos);
        ao += (f32(i) * 0.05 - d) * scale;
        scale *= 0.5;
    }
    return clamp(1.0 - ao, 0.0, 1.0);
}

// ---------------------------------------------------------------------------
// Ray-Box SDF (used per-pixel by rasterizer T6d)
// ---------------------------------------------------------------------------

/// Signed distance to an axis-aligned box centered at the origin.
fn sd_box(p: vec3f, half_ext: vec3f) -> f32 {
    let q = abs(p) - half_ext;
    return length(max(q, vec3f(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0);
}
