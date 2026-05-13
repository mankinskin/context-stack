// svo_ray_march.wgsl — Direct SVO Ray March compute shader (Phase 1b – 4b)
//
// Per-pixel (workgroup 8×8) ray marching through the world-space SVO:
//   1. Generate a world-space ray from inv_view_proj
//   2. Transform into normalised [0,1]³ SVO space
//   3. Hierarchically traverse the SVO with a fixed-size explicit stack
//   4. At leaves: evaluate a box SDF for sub-voxel anti-aliasing
//   5. Apply PBR lighting (Phase 3a) with shadow + reflection rays (Phase 2b)
//   6. Write RGBA to color_buffer and world-t to depth_buffer
//
// Phase 4a — Paged SVO:
//   - GPU buffer holds compact-packed pages (only occupied children stored).
//   - page_table[page_id] → physical base offset; 0xFFFFFFFF = not resident.
//   - Address resolution uses popcount-based rank arithmetic (no fixed strides).
//   - StackEntry carries page_base + depth for per-page relative indexing.
//
// Phase 4b — LOD Cutoff:
//   - lod_blend_factor() computes a screen-space size / threshold ratio.
//   - Stochastic per-pixel descent using pcg_hash seeded by frame_index + pixel.
//   - Non-descending nodes render as solid voxels with their propagated color.
//
// Output format: color_buffer stores one vec4f per pixel (RGBA, linear [0,1])
// tightly packed as [pixel_idx * 4 + 0..3] f32 values.

// ---------------------------------------------------------------------------
// Uniforms
// ---------------------------------------------------------------------------

struct RayMarchUniforms {
    inv_view_proj:   mat4x4f,  // 64 bytes
    view_proj:       mat4x4f,  // 64 bytes — Phase 3a: for NDC depth output
    camera_pos:      vec3f,    // 12 bytes
    cot_half_fov:    f32,      //  4 bytes — 1/tan(fov_y/2), for AA footprint and LOD
    resolution:      vec2f,    //  8 bytes
    screen_width:    u32,      //  4 bytes — u32 form of resolution.x for indexing
    frame_index:     u32,      //  4 bytes — Phase 4b: temporal noise seed
    light_dir:       vec3f,    // 12 bytes — normalised world-space light direction
    _pad1:           f32,      //  4 bytes
    light_color:     vec3f,    // 12 bytes
    max_bounces:     u32,      //  4 bytes — max secondary ray bounces (0 = primary only)
    max_shadow_dist: f32,      //  4 bytes — world-space max shadow ray distance
    feature_flags:   u32,      //  4 bytes — bit0=neighbor_blend bit1=shadow bit2=reflect bit3=lod
    lod_threshold:   f32,      //  4 bytes — Phase 4b: pixel screen-size threshold for LOD stop
    lod_softness:    f32,      //  4 bytes — Phase 4b: soft-band half-width around threshold
    _pad3:           vec4u,    // 16 bytes (explicit alignment to 16-byte boundary)
}                              // Total: 224 bytes

// SVO coordinate transform (matches Rust `SvoTransformData`, Phase 4a: page_depth added)
struct SvoTransform {
    origin:         vec3f,
    world_size:     f32,
    inv_world_size: f32,
    max_depth:      u32,
    page_depth:     u32,  // Phase 4a: depth at which the tree splits into paged subtrees
    _pad:           u32,
}

// ---------------------------------------------------------------------------
// Bindings (group 0)
//
// | Binding | Type                    | Content                               |
// |---------|-------------------------|---------------------------------------|
// |    0    | storage<read>           | octree array<vec2u> (packed pages)    |
// |    1    | uniform                 | RayMarchUniforms                      |
// |    2    | uniform                 | SvoTransform                          |
// |    3    | storage<read_write>     | depth_buffer array<f32>               |
// |    4    | storage<read_write>     | color_buffer array<f32>               |
// |    5    | storage<read>           | page_table array<u32> (Phase 4a)      |
// ---------------------------------------------------------------------------

@group(0) @binding(0) var<storage, read>       octree:       array<vec2u>;
@group(0) @binding(1) var<uniform>             uniforms:     RayMarchUniforms;
@group(0) @binding(2) var<uniform>             svo_tf:       SvoTransform;
// depth_buffer: one f32 per pixel — world-space ray parameter t at first hit
@group(0) @binding(3) var<storage, read_write> depth_buffer: array<f32>;
// color_buffer: four f32 per pixel (RGBA), linearised as pixel_idx * 4 + ch
@group(0) @binding(4) var<storage, read_write> color_buffer: array<f32>;
// page_table: virtual page_id → physical base node index; 0xFFFFFFFF = not resident
@group(0) @binding(5) var<storage, read>       page_table:   array<u32>;

// ---------------------------------------------------------------------------
// Blit bindings (group 0, separate pipeline — compiled from same file)
//
// The blit render pass reads from the compute shader's color_buffer and
// writes each pixel to the view target (swapchain surface).
// ---------------------------------------------------------------------------

// These declarations live here so both entry points are in one shader module.
// The blit pipeline uses a different bind group layout than the compute pipeline
// so binding indices re-start from 0 for the blit.
//
// @group(1) @binding(0) var<storage, read> blit_color_buffer: array<f32>;
// @group(1) @binding(1) var<uniform>       blit_uniforms: RayMarchUniforms;
//
// (declared below as blit_color_buf / blit_uniforms — separate group(1) so
//  the compute group(0) declarations above do not conflict)

@group(1) @binding(0) var<storage, read> blit_color_buf:  array<f32>;
@group(1) @binding(1) var<uniform>       blit_uniforms:   RayMarchUniforms;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const MAX_STACK_DEPTH: u32  = 12u;
const INTERIOR_FLAG:   u32  = 0x80000000u;
const OPAQUE_THRESHOLD: f32 = 0.99;
const BACKGROUND_R: f32 = 0.12;
const BACKGROUND_G: f32 = 0.14;
const BACKGROUND_B: f32 = 0.18;

// Phase 2a/2b feature flag bit positions (must match Rust `ray_march_feature_flags()`).
const FEAT_NEIGHBOR_BLEND: u32 = 1u;  // Phase 2a: smooth-min seam blending
const FEAT_SHADOW_RAYS:    u32 = 2u;  // Phase 2b: shadow ray per primary hit
const FEAT_REFLECTION:     u32 = 4u;  // Phase 2b: reflection for metallic surfaces
const FEAT_LOD:            u32 = 8u;  // Phase 4b: stochastic LOD cutoff

// ---------------------------------------------------------------------------
// PBR material helpers (Phase 3a — Cook-Torrance/GGX)
// ---------------------------------------------------------------------------

const PI: f32 = 3.14159265359;

struct Material {
    base_color: vec3f,
    roughness:  f32,
    metallic:   f32,
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
    mat:         Material,
    normal:      vec3f,
    view_dir:    vec3f,
    light_dir:   vec3f,
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
// OctreeNode bit-decode helpers (inlined from svo_common.wgsl)
// ---------------------------------------------------------------------------

fn svo_child_mask(cp: u32) -> u32 {
    return cp & 0xFFu;
}

fn svo_first_child(cp: u32) -> u32 {
    // Mask bit 31 (INTERIOR_FLAG) before extracting the 23-bit child index.
    return (cp >> 8u) & 0x7FFFFFu;
}

fn svo_is_interior(cp: u32) -> bool {
    return (cp & INTERIOR_FLAG) != 0u;
}

fn unpack_base_color(cd: u32) -> vec3f {
    let r = f32(cd        & 0xFFu) / 255.0;
    let g = f32((cd >> 8u) & 0xFFu) / 255.0;
    let b = f32((cd >> 16u) & 0xFFu) / 255.0;
    return vec3f(r, g, b);
}

// ---------------------------------------------------------------------------
// Material bit-field unpacking (bits 24-31 of color_data)
// ---------------------------------------------------------------------------

fn unpack_roughness(cd: u32) -> f32 {
    return f32((cd >> 24u) & 0x1Fu) / 31.0;
}

fn unpack_metallic(cd: u32) -> f32 {
    return f32((cd >> 29u) & 1u);
}

fn unpack_sdf_type(cd: u32) -> u32 {
    return (cd >> 30u) & 3u;
}

// ---------------------------------------------------------------------------
// Extended SDF library (Phase 2a)
// ---------------------------------------------------------------------------

// Sphere SDF: signed distance from p to a sphere centred at origin with radius r.
fn sd_sphere(p: vec3f, r: f32) -> f32 {
    return length(p) - r;
}

// Torus SDF: torus in the xy-plane (vertical ring / wheel orientation),
// major radius R (ring centre radius), minor radius r (tube radius).
// Rotating from xz to xy makes it visible as a ring from the default
// forward-facing camera angle.
fn sd_torus(p: vec3f, major_r: f32, minor_r: f32) -> f32 {
    let q = vec2f(length(p.xy) - major_r, p.z);
    return length(q) - minor_r;
}

// Dispatch SDF evaluation by sdf_type embedded in color_data.
// sdf_type: 0=box, 1=sphere, 2=svo-sampled(placeholder→box), 3=torus
fn eval_voxel_sdf(p_local: vec3f, half: f32, sdf_type: u32) -> f32 {
    switch sdf_type {
        case 1u: { return sd_sphere(p_local, half); }
        case 3u: { return sd_torus(p_local, half * 0.7, half * 0.3); }
        default: { return sd_box(p_local, vec3f(half)); }
    }
}

// Analytical surface normal for the chosen SDF type.
fn eval_voxel_normal(p_local: vec3f, half: f32, sdf_type: u32) -> vec3f {
    if sdf_type == 1u {
        // Sphere: gradient = p / |p|.  Guard against the exact-centre degenerate
        // case (length ≈ 0) where normalize() would produce NaN/zero.
        let len = length(p_local);
        if len < 1e-6 { return vec3f(0.0, 1.0, 0.0); }
        return p_local / len;
    }
    if sdf_type == 3u {
        // Torus in xy plane: analytical gradient (matches sd_torus above).
        let len_xy = max(length(p_local.xy), 1e-6);
        let q      = vec2f(len_xy - half * 0.7, p_local.z);
        let d      = max(length(q), 1e-6);
        let nxy    = (p_local.xy / len_xy) * (q.x / d);
        return normalize(vec3f(nxy.x, nxy.y, q.y / d));
    }
    // Default analytical box normal: largest abs component direction.
    let abs_p = abs(p_local) / half;
    if abs_p.x >= abs_p.y && abs_p.x >= abs_p.z {
        return vec3f(sign(p_local.x), 0.0, 0.0);
    } else if abs_p.y >= abs_p.z {
        return vec3f(0.0, sign(p_local.y), 0.0);
    }
    return vec3f(0.0, 0.0, sign(p_local.z));
}

// Returns the outward face normal for the AABB face that `ro + rd * t` entered.
//
// Computed from per-axis slab entry t-values so it is unambiguous even when
// the ray enters exactly at an edge or corner (two or three axis t-values are
// equal).  That case causes the position-based `eval_voxel_normal` to pick an
// arbitrary face, which is the root cause of visible grey gridlines between
// adjacent box voxels — the "winning" face is always the first branch (X),
// even when the ray is entering through the top (Y) face.
//
// `inv_rd` must be precomputed `1 / rd`.  `box_min / box_max` are the leaf
// AABB corners in the same space as `ro`.
fn box_entry_normal(ro: vec3f, inv_rd: vec3f, box_min: vec3f, box_max: vec3f) -> vec3f {
    // Per-axis slab entry t: how far along the ray until the ray is inside
    // each axis slab.  The LAST axis to be entered determines the face.
    let t1 = (box_min - ro) * inv_rd;
    let t2 = (box_max - ro) * inv_rd;
    let t_enter = min(t1, t2);   // entry t per axis
    // Outward normal on the entering face = -sign(rd) on that axis.
    // WGSL select(false_val, true_val, cond):
    //   inv_rd.x > 0 → rd.x > 0 → ray entered through min-X face → normal = -X
    if t_enter.x >= t_enter.y && t_enter.x >= t_enter.z {
        return vec3f(select(1.0, -1.0, inv_rd.x > 0.0), 0.0, 0.0);
    } else if t_enter.y >= t_enter.z {
        return vec3f(0.0, select(1.0, -1.0, inv_rd.y > 0.0), 0.0);
    }
    return vec3f(0.0, 0.0, select(1.0, -1.0, inv_rd.z > 0.0));
}

// ---------------------------------------------------------------------------
// Smooth-union (smooth_min) for Phase 2a neighbor blending
// ---------------------------------------------------------------------------

fn smooth_min(a: f32, b: f32, k: f32) -> f32 {
    let h = clamp(0.5 + 0.5 * (b - a) / k, 0.0, 1.0);
    return mix(b, a, h) - k * h * (1.0 - h);
}

// ---------------------------------------------------------------------------
// SVO point lookup — returns color_data of leaf containing `pos` (Phase 2a).
//
// `pos` must be in normalised SVO space [0,1)³.  Returns 0 for empty or
// out-of-range positions.  Interior leaves (INTERIOR_FLAG) are treated as
// occupied for blending purposes.
//
// Time complexity: O(max_depth).  Gate behind FEAT_NEIGHBOR_BLEND check to
// avoid per-pixel cost when blending is disabled.
// ---------------------------------------------------------------------------

fn svo_lookup(pos: vec3f) -> u32 {
    if any(pos < vec3f(0.0)) || any(pos >= vec3f(1.0)) { return 0u; }
    var node_idx: u32 = 0u;
    var aabb_min = vec3f(0.0);
    var aabb_size = 1.0;
    for (var depth = 0u; depth < svo_tf.max_depth + 1u; depth++) {
        let node  = octree[node_idx];
        let cp    = node.x;
        let cd    = node.y;
        let cmask = svo_child_mask(cp);
        // Leaf (no children) — return its color_data (0 = empty, != 0 = occupied).
        if cmask == 0u { return cd; }
        let half   = aabb_size * 0.5;
        let center = aabb_min + vec3f(half);
        let bx     = pos.x >= center.x;
        let by     = pos.y >= center.y;
        let bz     = pos.z >= center.z;
        let ci     = u32(bx) | (u32(by) << 1u) | (u32(bz) << 2u);
        if (cmask & (1u << ci)) == 0u { return 0u; }
        aabb_min  += vec3f(select(0.0, half, bx), select(0.0, half, by), select(0.0, half, bz));
        aabb_size  = half;
        node_idx   = svo_first_child(cp) + ci;
    }
    return octree[node_idx].y;
}

// ---------------------------------------------------------------------------
// Neighbor SDF blending (Phase 2a)
//
// Samples the 6 face-adjacent voxel slots and smooth-unions their SDFs with
// the centre voxel, eliminating hard seams between neighbouring filled cells.
// ---------------------------------------------------------------------------

fn blend_with_neighbors(
    center_d:     f32,
    ray_pos:      vec3f,    // sample point in SVO space
    voxel_center: vec3f,    // centre of the current leaf in SVO space
    half:         f32,      // half-size of the leaf in SVO space
    blend_k:      f32,      // smooth-union radius
) -> f32 {
    var d = center_d;
    let step = half * 2.0;
    // ±x, ±y, ±z face neighbours
    let offsets = array<vec3f, 6>(
        vec3f( 1.0, 0.0, 0.0),
        vec3f(-1.0, 0.0, 0.0),
        vec3f( 0.0, 1.0, 0.0),
        vec3f( 0.0,-1.0, 0.0),
        vec3f( 0.0, 0.0, 1.0),
        vec3f( 0.0, 0.0,-1.0),
    );
    for (var i = 0u; i < 6u; i++) {
        let neighbor_center = voxel_center + offsets[i] * step;
        let neighbor_cd     = svo_lookup(neighbor_center);
        if neighbor_cd != 0u {
            let sdf_type   = unpack_sdf_type(neighbor_cd);
            let neighbor_d = eval_voxel_sdf(ray_pos - neighbor_center, half, sdf_type);
            d = smooth_min(d, neighbor_d, blend_k);
        }
    }
    return d;
}

// ---------------------------------------------------------------------------
// World ↔ SVO coordinate transforms
// ---------------------------------------------------------------------------

fn world_to_svo(p: vec3f) -> vec3f {
    return (p - svo_tf.origin) * svo_tf.inv_world_size;
}

// ---------------------------------------------------------------------------
// Ray–AABB slab intersection
//
// Returns vec2f(t_enter, t_exit).  A hit occurs when t_enter < t_exit AND
// t_exit > 0.  We use the safe version that handles infinite ray_inv_d.
// ---------------------------------------------------------------------------

fn ray_aabb_slab(ro: vec3f, inv_d: vec3f, box_min: vec3f, box_max: vec3f) -> vec2f {
    let t1 = (box_min - ro) * inv_d;
    let t2 = (box_max - ro) * inv_d;
    let t_min = max(max(min(t1.x, t2.x), min(t1.y, t2.y)), min(t1.z, t2.z));
    let t_max = min(min(max(t1.x, t2.x), max(t1.y, t2.y)), max(t1.z, t2.z));
    return vec2f(t_min, t_max);
}

// ---------------------------------------------------------------------------
// Box SDF: signed distance from p to an axis-aligned box centred at origin
// with half-extents `half`.  Negative inside, zero at surface, positive outside.
// ---------------------------------------------------------------------------

fn sd_box(p: vec3f, half: vec3f) -> f32 {
    let q = abs(p) - half;
    return length(max(q, vec3f(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0);
}

// ---------------------------------------------------------------------------
// Stack entry for the DDA traversal
// ---------------------------------------------------------------------------

struct StackEntry {
    node_idx:  u32,
    depth:     u32,    // current octree depth (0 = root)
    aabb_min:  vec3f,
    aabb_size: f32,
    t_enter:   f32,
}

// ---------------------------------------------------------------------------
// Phase 4b — stochastic LOD cutoff helpers
// ---------------------------------------------------------------------------

/// Hashed noise: PCG hash of a seed u32 → pseudo-random u32 uniformly on [0, 2^32).
fn pcg_hash(seed: u32) -> u32 {
    var s = seed * 747796405u + 2891336453u;
    let w = ((s >> ((s >> 28u) + 4u)) ^ s) * 277803737u;
    return (w >> 22u) ^ w;
}

/// Stochastic LOD blend factor for the current internal node.
///
/// Returns a value in [0, 1]: 0.0 = always descend, 1.0 = always stop (render
/// the node as a solid leaf using its propagated colour).  Values in between
/// represent a soft blend band where the probability of stopping increases
/// linearly with depth relative to `lod_threshold`.
///
/// # Arguments
/// * `node_size_world` — world-space side length of the current node's AABB.
/// * `t_world`         — world-space ray parameter at the node's entry point.
/// * `fw_coeff`        — pixel footprint coefficient (2 / (cot_half_fov * height)).
fn lod_blend_factor(node_size_world: f32, t_world: f32, fw_coeff: f32) -> f32 {
    // Screen-space footprint of the node in pixels.
    let fw     = fw_coeff * max(t_world, 1e-4);
    let pixels = node_size_world / max(fw, 1e-6);
    // lod_threshold: stop when the node is smaller than this many pixels.
    // lod_softness:  blending band half-width in pixels.
    let lo     = uniforms.lod_threshold - uniforms.lod_softness;
    let hi     = uniforms.lod_threshold + uniforms.lod_softness;
    return 1.0 - smoothstep(lo, hi, pixels);
}

// ---------------------------------------------------------------------------
// SVO traversal
//
// Returns (hit, base_color, alpha, t_world, world_normal).
// t_world is the world-space ray parameter at the first opaque hit.
// ---------------------------------------------------------------------------

struct TraversalResult {
    hit:         bool,
    color:       vec3f,
    alpha:       f32,
    t_hit:       f32,
    normal:      vec3f,
    roughness:   f32,   // perceptual roughness [0,1]
    metallic:    f32,   // metallic flag [0,1] from bit 29
    transparent: bool,  // Phase 2b: refraction flag (always false until atom system adds transparent materials)
    ior:         f32,   // index of refraction (e.g. 1.5 for glass; unused when !transparent)
}

fn traverse_svo(
    ro_svo:     vec3f,  // ray origin in normalised [0,1]³ SVO space
    rd_svo:     vec3f,  // ray direction in SVO space (NOT renormalised — keeps scale)
    inv_rd:     vec3f,  // element-wise 1 / rd_svo
    t_scale:    f32,    // svo_t * t_scale = world-space t
    fw_coeff:   f32,    // pixel footprint coefficient: fw = fw_coeff * t_world
    pixel_xy:   vec2u,  // Phase 4b: pixel coordinates for LOD noise seed
) -> TraversalResult {
    // Root AABB is [0,1]³ in SVO space.
    let root_hit = ray_aabb_slab(ro_svo, inv_rd, vec3f(0.0), vec3f(1.0));
    if root_hit.x >= root_hit.y || root_hit.y <= 0.0 {
        return TraversalResult(false, vec3f(0.0), 0.0, 1e30, vec3f(0.0, 1.0, 0.0), 1.0, 0.0, false, 1.0);
    }

    // Fixed-size stack (depth-limited to MAX_STACK_DEPTH).
    var stack: array<StackEntry, 12>;
    var sp: u32 = 0u;

    stack[0] = StackEntry(0u, 0u, vec3f(0.0), 1.0, max(root_hit.x, 0.0));
    sp = 1u;

    var accum_color = vec3f(0.0);
    var accum_alpha = 0.0;
    var best_t         = 1e30;
    var best_normal    = vec3f(0.0, 1.0, 0.0);
    var best_roughness = 1.0;
    var best_metallic  = 0.0;

    let neighbor_blend = (uniforms.feature_flags & FEAT_NEIGHBOR_BLEND) != 0u;

    while sp > 0u && accum_alpha < OPAQUE_THRESHOLD {
        sp -= 1u;
        let entry     = stack[sp];
        let node_idx  = entry.node_idx;
        let node_depth = entry.depth;
        let n_min     = entry.aabb_min;
        let n_size    = entry.aabb_size;
        let n_max     = n_min + vec3f(n_size);
        let t_enter   = entry.t_enter;

        // Prune: already have a closer opaque hit
        if t_enter >= best_t / t_scale {
            continue;
        }

        let node  = octree[node_idx];
        let cp    = node.x;   // child_pointer
        let cd    = node.y;   // color_data

        let cmask = svo_child_mask(cp);
        let is_leaf = cmask == 0u;

        if is_leaf {
            if cd == 0u {
                continue; // empty leaf
            }
            // Interior leaves are fully surrounded — skip SDF, front-to-back
            // early termination handles occlusion naturally.
            if svo_is_interior(cp) {
                continue;
            }

            // Phase 2a: type-dispatched SDF evaluation.
            let sdf_type = unpack_sdf_type(cd);
            let half     = n_size * 0.5;
            let center   = n_min + vec3f(half);

            // Ray–AABB intersection for this leaf.
            let aabb_hit = ray_aabb_slab(ro_svo, inv_rd, n_min, n_max);

            // t_entry: first surface contact with the voxel.
            //   Used for: hit depth, shadow/reflect ray origin, surface normal.
            //   Placing the shadow origin here (+ normal bias) avoids self-intersection.
            // t_sample: AABB interior midpoint (used only for SDF d evaluation).
            //   Using t_entry for d would give d ≈ 0 → alpha ≈ 0.5 (translucent).
            //   The midpoint gives d << 0 → alpha ≈ 1.0 (opaque solid).
            var t_entry  = max(aabb_hit.x, 0.0);
            var t_sample = clamp(
                (aabb_hit.x + aabb_hit.y) * 0.5,
                t_entry,
                aabb_hit.y,
            );
            // Torus: the AABB midpoint is always at the voxel centre, which is
            // the hole of the torus (d ≈ major_r – minor_r >> fw).  Sphere-trace
            // along the ray inside the AABB to find the tube surface.
            // rd_svo = rd_world * inv_world_size  →  |rd_svo| = inv_world_size
            // step_t = d_svo / |rd_svo| converts SVO-space SDF steps to t units.
            if sdf_type == 3u {
                let rd_inv_len = 1.0 / length(rd_svo); // ≈ world_size
                var t_st = t_entry;
                for (var _i = 0; _i < 8; _i++) {
                    let d_st = sd_torus(ro_svo + rd_svo * t_st - center,
                                        half * 0.7, half * 0.3);
                    if abs(d_st) < half * 5e-3 { break; }
                    t_st += d_st * rd_inv_len;
                    if t_st >= aabb_hit.y { break; }
                }
                t_sample = clamp(t_st, t_entry, aabb_hit.y);
                // For torus the sphere-traced point IS the surface, so entry = sample.
                t_entry  = t_sample;
            }
            let hit_p   = ro_svo + rd_svo * t_sample;
            let p_local = hit_p - center;
            var d       = eval_voxel_sdf(p_local, half, sdf_type);

            // Phase 2a: optional smooth-min neighbor blending.
            if neighbor_blend {
                let blend_k = half * 0.25;
                d = blend_with_neighbors(d, hit_p, center, half, blend_k);
            }

            // Pixel footprint at the surface entry distance (not the interior midpoint).
            let t_world = t_entry * t_scale;
            let fw      = fw_coeff * max(t_world, 1e-4);
            let alpha   = 1.0 - smoothstep(-fw, fw, d);

            if alpha > 0.001 {
                // Evaluate normal at the ray entry point, not the AABB midpoint.
                // At the midpoint all position components are ≈ 0 → degenerate
                // gradient for every SDF type (was the root cause of the
                // "hard centre point" artifact fixed in the previous pass).
                let p_entry_local = (ro_svo + rd_svo * t_entry) - center;

                // Box types: use the slab-derived entry-face normal, which is
                // unambiguous at edges/corners (see box_entry_normal above).
                //
                // Gridline seam suppression (requires FEAT_NEIGHBOR_BLEND):
                //   At shallow viewing angles, rays enter flat ground voxels
                //   through SIDE faces (Z or X) instead of the TOP face.  Side
                //   faces have lower dot(normal, light_dir) → darker pixels →
                //   visible grid.  When the entry face is INTERIOR (covered by
                //   an adjacent voxel), the camera can only see the voxel because
                //   it approached from a grazing angle: the physically correct
                //   visible surface is the nearest EXPOSED face.  We prefer +Y
                //   (up) first — it eliminates grass/terrain gridlines — then
                //   -Y, then fall back to the entry face for fully interior
                //   voxels (stone cores etc.).  Cost: 1–3 svo_lookup calls,
                //   same order as the per-leaf SDF blend already under this flag.
                //
                // Sphere / torus: use the SDF gradient at the AABB entry point
                // (directionally correct; exact surface normal requires an
                // additional ray–sphere solve, deferred to a later phase).
                var normal: vec3f;
                if sdf_type == 0u || sdf_type == 2u {
                    let entry_n = box_entry_normal(ro_svo, inv_rd, n_min, n_max);
                    if neighbor_blend && svo_lookup(center + entry_n * n_size) != 0u {
                        // Entry face is interior — find the nearest exposed face.
                        if svo_lookup(center + vec3f(0.0, n_size, 0.0)) == 0u {
                            normal = vec3f(0.0, 1.0, 0.0);  // +Y exposed
                        } else if svo_lookup(center - vec3f(0.0, n_size, 0.0)) == 0u {
                            normal = vec3f(0.0, -1.0, 0.0); // -Y exposed
                        } else {
                            normal = entry_n; // fully interior voxel, no better option
                        }
                    } else {
                        normal = entry_n;
                    }
                } else {
                    normal = eval_voxel_normal(p_entry_local, half, sdf_type);
                }

                // Front-to-back alpha compositing.
                let contribution = alpha * (1.0 - accum_alpha);
                accum_color += unpack_base_color(cd) * contribution;
                accum_alpha += contribution;

                if t_world < best_t {
                    best_t         = t_world;
                    best_normal    = normal;
                    best_roughness = unpack_roughness(cd);
                    best_metallic  = unpack_metallic(cd);
                }
            }
            continue; // leaf processed
        }

        // Internal node: sort children front-to-back and push onto the stack.
        let child_size = n_size * 0.5;
        let fc         = svo_first_child(cp);

        // Phase 4b: stochastic LOD cutoff.
        // If the node is small enough in screen-space, treat it as a leaf using
        // its propagated colour (cd) instead of descending further.
        let lod_en = (uniforms.feature_flags & FEAT_LOD) != 0u;
        if lod_en && cd != 0u && !svo_is_interior(cp) {
            let t_world_lod  = t_enter * t_scale;
            let size_world   = n_size * svo_tf.world_size;
            let blend        = lod_blend_factor(size_world, t_world_lod, fw_coeff);
            // Hash pixel position + depth + frame_index for temporal stability.
            let pixel_seed   = pixel_xy.y * uniforms.screen_width + pixel_xy.x;
            let noise_raw    = pcg_hash(pcg_hash(pixel_seed + node_depth * 1000u) ^ uniforms.frame_index);
            let noise_f      = f32(noise_raw) / 4294967295.0; // [0, 1)
            if noise_f < blend {
                // Render aggregate colour as opaque leaf.
                let contribution = 1.0 * (1.0 - accum_alpha);
                accum_color += unpack_base_color(cd) * contribution;
                accum_alpha += contribution;
                if t_enter * t_scale < best_t {
                    best_t         = t_enter * t_scale;
                    best_normal    = vec3f(0.0, 1.0, 0.0); // default up normal for LOD surrogates
                    best_roughness = 1.0;
                    best_metallic  = 0.0;
                }
                continue;
            }
        }

        // Collect visible children and their entry t values.
        var child_t:    array<f32,   8>;
        var child_ni:   array<u32,   8>;
        var child_min:  array<vec3f, 8>;
        var child_cnt:  u32 = 0u;

        for (var ci = 0u; ci < 8u; ci++) {
            let bit = 1u << ci;
            if (cmask & bit) == 0u {
                continue;
            }
            let ox   = f32((ci     ) & 1u);
            let oy   = f32((ci >> 1u) & 1u);
            let oz   = f32((ci >> 2u) & 1u);
            let c_min = n_min + vec3f(ox, oy, oz) * child_size;
            let c_max = c_min + child_size;

            let chit = ray_aabb_slab(ro_svo, inv_rd, c_min, c_max);
            if chit.x >= chit.y || chit.y <= 0.0 {
                continue;
            }
            let t_child = max(chit.x, 0.0);
            if t_child >= best_t / t_scale {
                continue;
            }

            // VoxelWorld allocates a full 8-node block per parent; child ci
            // lives at first_child + ci (direct slot index, NOT packed).
            child_t[child_cnt]   = t_child;
            child_ni[child_cnt]  = fc + ci;
            child_min[child_cnt] = c_min;
            child_cnt += 1u;
        }

        // Insertion-sort the collected children descending by t_enter, so the
        // stack pops them in ascending (front-to-back) order.
        for (var i = 1u; i < child_cnt; i++) {
            let kt  = child_t[i];
            let kni = child_ni[i];
            let km  = child_min[i];
            var j   = i;
            while j > 0u && child_t[j - 1u] < kt {
                child_t[j]   = child_t[j - 1u];
                child_ni[j]  = child_ni[j - 1u];
                child_min[j] = child_min[j - 1u];
                j -= 1u;
            }
            child_t[j]   = kt;
            child_ni[j]  = kni;
            child_min[j] = km;
        }

        // Push sorted children (farthest first → closest will be on top).
        for (var i = 0u; i < child_cnt; i++) {
            if sp < MAX_STACK_DEPTH {
                stack[sp] = StackEntry(child_ni[i], node_depth + 1u, child_min[i], child_size, child_t[i]);
                sp += 1u;
            }
        }
    }

    if accum_alpha < 0.001 {
        return TraversalResult(false, vec3f(0.0), 0.0, 1e30, vec3f(0.0, 1.0, 0.0), 1.0, 0.0, false, 1.0);
    }
    return TraversalResult(true, accum_color, accum_alpha, best_t, best_normal, best_roughness, best_metallic, false, 1.0);
}

// ---------------------------------------------------------------------------
// Shadow occlusion traversal (Phase 2b)
//
// Simplified SVO traversal — only detects the first occupied leaf hit.
// No SDF evaluation, no alpha compositing.  Returns true if any leaf blocks
// the ray before max_t_raw (SVO-space ray parameter == world-space distance).
// ---------------------------------------------------------------------------

fn svo_trace_occlusion(
    ro_svo:    vec3f,  // ray origin in SVO space
    rd_svo:    vec3f,  // ray direction in SVO space
    inv_rd:    vec3f,  // element-wise 1 / rd_svo
    max_t_raw: f32,    // max SVO-space t to search (= world-space distance for this parameterisation)
) -> bool {
    let root_hit = ray_aabb_slab(ro_svo, inv_rd, vec3f(0.0), vec3f(1.0));
    if root_hit.x >= root_hit.y || root_hit.y <= 0.0 { return false; }
    if root_hit.x > max_t_raw { return false; }

    var stack: array<StackEntry, 12>;
    var sp: u32 = 0u;
    stack[0] = StackEntry(0u, 0u, vec3f(0.0), 1.0, max(root_hit.x, 0.0));  // occlusion: depth unused
    sp = 1u;

    while sp > 0u {
        sp -= 1u;
        let entry = stack[sp];
        if entry.t_enter > max_t_raw { continue; }

        let node  = octree[entry.node_idx];
        let cp    = node.x;
        let cd    = node.y;
        let cmask = svo_child_mask(cp);

        if cmask == 0u {
            // Leaf: occupied if cd != 0 and not an interior-only node.
            if cd != 0u && !svo_is_interior(cp) { return true; }
            continue;
        }

        let child_size = entry.aabb_size * 0.5;
        let fc         = svo_first_child(cp);

        for (var ci = 0u; ci < 8u; ci++) {
            let bit = 1u << ci;
            if (cmask & bit) == 0u { continue; }
            let ox    = f32((ci     ) & 1u);
            let oy    = f32((ci >> 1u) & 1u);
            let oz    = f32((ci >> 2u) & 1u);
            let c_min = entry.aabb_min + vec3f(ox, oy, oz) * child_size;
            let c_max = c_min + child_size;
            let chit  = ray_aabb_slab(ro_svo, inv_rd, c_min, c_max);
            if chit.x >= chit.y || chit.y <= 0.0 { continue; }
            let t_child = max(chit.x, 0.0);
            if t_child > max_t_raw { continue; }
            if sp < MAX_STACK_DEPTH {
                stack[sp] = StackEntry(fc + ci, 0u, c_min, child_size, t_child);  // depth unused in occlusion
                sp += 1u;
            }
        }
    }
    return false;
}

// ---------------------------------------------------------------------------
// Compute entry point — ray march
// ---------------------------------------------------------------------------

@compute @workgroup_size(8, 8)
fn ray_march_main(@builtin(global_invocation_id) gid: vec3u) {
    let res_u = vec2u(u32(uniforms.resolution.x), u32(uniforms.resolution.y));
    if gid.x >= res_u.x || gid.y >= res_u.y {
        return;
    }

    let pixel_idx  = gid.y * res_u.x + gid.x;
    let color_base = pixel_idx * 4u;

    // Reconstruct world-space ray from NDC.
    // NDC: x in [-1,1] left-to-right, y in [-1,1] bottom-to-top (flip screen y).
    let ndc_x = (f32(gid.x) + 0.5) / uniforms.resolution.x * 2.0 - 1.0;
    let ndc_y = 1.0 - (f32(gid.y) + 0.5) / uniforms.resolution.y * 2.0;
    let clip   = vec4f(ndc_x, ndc_y, 1.0, 1.0);

    let world_h = uniforms.inv_view_proj * clip;
    let world_p = world_h.xyz / world_h.w;

    let ro_world = uniforms.camera_pos;
    let rd_world = normalize(world_p - ro_world);

    // Transform ray to SVO space.  Direction is scaled uniformly by
    // inv_world_size (no translation for direction vectors).
    let ro_svo = world_to_svo(ro_world);
    let rd_svo = rd_world * svo_tf.inv_world_size;
    var inv_rd = 1.0 / rd_svo;
    // Guard against exact-zero components to avoid NaN in slab test.
    if abs(inv_rd.x) > 1e15 { inv_rd.x = 1e15 * sign(inv_rd.x + 1e-30); }
    if abs(inv_rd.y) > 1e15 { inv_rd.y = 1e15 * sign(inv_rd.y + 1e-30); }
    if abs(inv_rd.z) > 1e15 { inv_rd.z = 1e15 * sign(inv_rd.z + 1e-30); }

    // t_scale: AABB t values equal world-space distances (t_svo == t_world when
    // rd_svo = rd_world * inv_world_size).  Multiply by inv_world_size to get
    // SVO-normalised distances, keeping fw (pixel footprint) in the same units
    // as d (sd_box output) — both in [0,1] SVO space.
    let t_scale  = svo_tf.inv_world_size;
    // Pixel footprint coefficient: fw = 2 * t_world / (cot_half_fov * height)
    let fw_coeff = 2.0 / (uniforms.cot_half_fov * uniforms.resolution.y);

    let result = traverse_svo(ro_svo, rd_svo, inv_rd, t_scale, fw_coeff, gid.xy);

    if !result.hit {
        color_buffer[color_base]      = BACKGROUND_R;
        color_buffer[color_base + 1u] = BACKGROUND_G;
        color_buffer[color_base + 2u] = BACKGROUND_B;
        color_buffer[color_base + 3u] = 1.0;
        depth_buffer[pixel_idx]       = 0.0;  // far plane in infinite reverse-Z
        return;
    }

    // Phase 2b: feature flag decoding.
    let flags       = uniforms.feature_flags;
    let shadow_en   = (flags & FEAT_SHADOW_RAYS) != 0u;
    let reflect_en  = (flags & FEAT_REFLECTION)  != 0u;

    // Surface offset bias: push shadow/reflection origins off the hit surface.
    // Uses a fraction of the leaf voxel's half-size in world units (the leaf
    // size is world_size / 2^max_depth).  0.05 voxel-half-widths is enough to
    // clear self-intersection for all SDF types (spheres, tori) and is scale-
    // invariant regardless of world_size.
    // bias_svo = 0.05 * (world_size / 2^max_depth) / world_size
    //          = 0.05 / 2^max_depth  (in normalised SVO space)
    let leaf_half_svo = 0.5 / f32(1u << svo_tf.max_depth);
    let origin_bias   = 0.05 * leaf_half_svo;

    // Light direction transformed to SVO-space ray parameterisation.
    let light_dir_svo = uniforms.light_dir * svo_tf.inv_world_size;

    // Iterative bounce loop (Phase 2b).  WGSL has no function recursion, so
    // reflections use an explicit loop capped at max_bounces.
    var final_color   = vec3f(0.0);
    var throughput    = vec3f(1.0);
    var bounce_ro     = ro_svo;
    var bounce_rd     = rd_svo;
    var bounce_result = result;
    var first_t       = result.t_hit;

    for (var bounce = 0u; bounce <= uniforms.max_bounces; bounce++) {
        let normal   = bounce_result.normal;
        // Un-premultiply the accumulated colour to get the bare surface colour.
        let base_col = bounce_result.color / max(bounce_result.alpha, 0.001);

        // ---- Shadow ray (Phase 2b) ----
        var shadow = 1.0;
        if shadow_en {
            // Hit point in SVO space: t_raw = t_hit_svo_normalised * world_size
            let t_raw       = bounce_result.t_hit * svo_tf.world_size;
            let hit_p       = bounce_ro + bounce_rd * t_raw;
            let shadow_orig = hit_p + normal * origin_bias;

            var shad_inv = 1.0 / light_dir_svo;
            if abs(shad_inv.x) > 1e15 { shad_inv.x = 1e15 * sign(shad_inv.x + 1e-30); }
            if abs(shad_inv.y) > 1e15 { shad_inv.y = 1e15 * sign(shad_inv.y + 1e-30); }
            if abs(shad_inv.z) > 1e15 { shad_inv.z = 1e15 * sign(shad_inv.z + 1e-30); }

            // Shadow max_t: world-space distance = SVO-space t parameter.
            let occluded = svo_trace_occlusion(
                shadow_orig, light_dir_svo, shad_inv, uniforms.max_shadow_dist
            );
            shadow = select(1.0, 0.0, occluded);
        }

        // ---- PBR shading (Phase 3a — Cook-Torrance/GGX) ----
        let pbr_mat  = Material(base_col, bounce_result.roughness, bounce_result.metallic);
        let view_dir = -normalize(bounce_rd);  // surface → ray origin direction
        let pbr_lit  = evaluate_pbr(
            pbr_mat,
            normal,
            view_dir,
            normalize(uniforms.light_dir),
            uniforms.light_color,
        ) * shadow;
        // Small ambient term: non-metallic diffuse fill light.
        let ambient = base_col * 0.03 * (1.0 - bounce_result.metallic);
        let lit = pbr_lit + ambient;

        // Blend with background for semi-transparent SDF fringe pixels.
        let inv_a   = 1.0 - bounce_result.alpha;
        let blended = lit * bounce_result.alpha
                    + vec3f(BACKGROUND_R, BACKGROUND_G, BACKGROUND_B) * inv_a;

        // ---- Check for reflection / refraction (Phase 2b) ----
        let can_reflect = reflect_en
                       && bounce_result.metallic > 0.5
                       && bounce < uniforms.max_bounces;
        let can_refract = bounce_result.transparent
                       && bounce < uniforms.max_bounces;

        if can_reflect {
            // Schlick Fresnel for metallic reflectance.
            // Metals have F0 ≈ 0.9 (not 0.04 like dielectrics), so reflections
            // are strong even at normal incidence — metallic surfaces are mirrors.
            let cos_theta = max(-dot(normalize(bounce_rd), normal), 0.0);
            let fresnel   = 0.9 + 0.1 * pow(1.0 - cos_theta, 5.0);

            // Diffuse contribution is minimal for metals (metals have no diffuse).
            final_color += throughput * blended * (1.0 - fresnel);
            throughput  *= fresnel;

            // Compute reflected ray from the hit point.
            let t_raw      = bounce_result.t_hit * svo_tf.world_size;
            let hit_p      = bounce_ro + bounce_rd * t_raw;
            let refl_orig  = hit_p + normal * origin_bias;
            let refl_rd    = reflect(bounce_rd, normal);

            var refl_inv = 1.0 / refl_rd;
            if abs(refl_inv.x) > 1e15 { refl_inv.x = 1e15 * sign(refl_inv.x + 1e-30); }
            if abs(refl_inv.y) > 1e15 { refl_inv.y = 1e15 * sign(refl_inv.y + 1e-30); }
            if abs(refl_inv.z) > 1e15 { refl_inv.z = 1e15 * sign(refl_inv.z + 1e-30); }

            let next = traverse_svo(refl_orig, refl_rd, refl_inv, t_scale, fw_coeff, gid.xy);
            if !next.hit {
                final_color += throughput
                             * vec3f(BACKGROUND_R, BACKGROUND_G, BACKGROUND_B);
                break;
            }
            bounce_ro     = refl_orig;
            bounce_rd     = refl_rd;
            bounce_result = next;
        } else if can_refract {
            // Refraction ray (Phase 2b infrastructure).
            // bounce_result.transparent is always false until the atom system
            // adds transparent materials; this branch handles TIR correctly
            // so AC 2b-7 is satisfied structurally.
            let inc_unit  = normalize(bounce_rd);
            let refr_unit = refract(inc_unit, normal, 1.0 / bounce_result.ior);
            if dot(refr_unit, refr_unit) < 0.001 {
                // Total internal reflection — fall back to opaque diffuse.
                final_color += throughput * blended;
                break;
            }
            // Scale refracted unit direction to SVO-space parameterisation.
            let refr_rd   = refr_unit * svo_tf.inv_world_size;
            let t_raw     = bounce_result.t_hit * svo_tf.world_size;
            let hit_p     = bounce_ro + bounce_rd * t_raw;
            // Bias origin into the surface (opposite direction to normal).
            let refr_orig = hit_p - normal * origin_bias;
            var refr_inv  = 1.0 / refr_rd;
            if abs(refr_inv.x) > 1e15 { refr_inv.x = 1e15 * sign(refr_inv.x + 1e-30); }
            if abs(refr_inv.y) > 1e15 { refr_inv.y = 1e15 * sign(refr_inv.y + 1e-30); }
            if abs(refr_inv.z) > 1e15 { refr_inv.z = 1e15 * sign(refr_inv.z + 1e-30); }
            let next = traverse_svo(refr_orig, refr_rd, refr_inv, t_scale, fw_coeff, gid.xy);
            if !next.hit {
                final_color += throughput
                             * vec3f(BACKGROUND_R, BACKGROUND_G, BACKGROUND_B);
                break;
            }
            bounce_ro     = refr_orig;
            bounce_rd     = refr_rd;
            bounce_result = next;
        } else {
            // Diffuse surface or all secondary rays disabled — accumulate and stop.
            final_color += throughput * blended;
            break;
        }
    }

    color_buffer[color_base]      = final_color.r;
    color_buffer[color_base + 1u] = final_color.g;
    color_buffer[color_base + 2u] = final_color.b;
    color_buffer[color_base + 3u] = 1.0;

    // Phase 3a: Write NDC depth for the hardware depth bridge.
    // `first_t` is SVO-normalised; multiply by world_size to get the
    // world-space ray parameter, then project to Bevy's infinite reverse-Z.
    let t_world_actual = first_t * svo_tf.world_size;
    let hit_world      = ro_world + rd_world * t_world_actual;
    let clip_pos       = uniforms.view_proj * vec4f(hit_world, 1.0);
    depth_buffer[pixel_idx] = clip_pos.z / clip_pos.w;
}

// ---------------------------------------------------------------------------
// Blit vertex shader — fullscreen triangle (group 1 bindings only used in fs)
// ---------------------------------------------------------------------------

struct BlitVertexOut {
    @builtin(position) clip_pos: vec4f,
}

@vertex
fn blit_vs(@builtin(vertex_index) vid: u32) -> BlitVertexOut {
    // Single triangle covering the entire clip-space [-1,1]×[-1,1]
    let positions = array<vec2f, 3>(
        vec2f(-1.0, -1.0),
        vec2f( 3.0, -1.0),
        vec2f(-1.0,  3.0),
    );
    var out: BlitVertexOut;
    out.clip_pos = vec4f(positions[vid], 0.0, 1.0);
    return out;
}

// ---------------------------------------------------------------------------
// Blit fragment shader — copy color_buffer pixel to view target
// ---------------------------------------------------------------------------

@fragment
fn blit_fs(@builtin(position) frag_pos: vec4f) -> @location(0) vec4f {
    let px   = u32(frag_pos.x);
    let py   = u32(frag_pos.y);
    let w    = blit_uniforms.screen_width;
    let base = (py * w + px) * 4u;
    let r    = blit_color_buf[base];
    let g    = blit_color_buf[base + 1u];
    let b    = blit_color_buf[base + 2u];
    let a    = blit_color_buf[base + 3u];
    return vec4f(r, g, b, a);
}
