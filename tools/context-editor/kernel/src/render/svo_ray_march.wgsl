// svo_ray_march.wgsl — Direct SVO Ray March compute shader (Phase 1b)
//
// Per-pixel (workgroup 8×8) ray marching through the world-space SVO:
//   1. Generate a world-space ray from inv_view_proj
//   2. Transform into normalised [0,1]³ SVO space
//   3. Hierarchically traverse the SVO with a fixed-size explicit stack
//   4. At leaves: evaluate a box SDF for sub-voxel anti-aliasing
//   5. Apply simplified Lambertian + ambient lighting
//   6. Write RGBA to color_buffer and world-t to depth_buffer
//
// This shader uses the existing full-SVO upload (SvoDoubleBuffer) — all node
// indices in child_pointer are direct flat-array lookups.  Paging is Phase 4a.
//
// Output format: color_buffer stores one vec4f per pixel (RGBA, linear [0,1])
// tightly packed as [pixel_idx * 4 + 0..3] f32 values.

// ---------------------------------------------------------------------------
// Uniforms
// ---------------------------------------------------------------------------

struct RayMarchUniforms {
    inv_view_proj: mat4x4f,  // 64 bytes
    camera_pos:    vec3f,    // 12 bytes
    cot_half_fov:  f32,      //  4 bytes — 1/tan(fov_y/2), for AA pixel footprint
    resolution:    vec2f,    //  8 bytes
    screen_width:  u32,      //  4 bytes — u32 form of resolution.x for indexing
    _pad0:         u32,      //  4 bytes
    light_dir:     vec3f,    // 12 bytes — normalised world-space light direction
    _pad1:         f32,      //  4 bytes
    light_color:   vec3f,    // 12 bytes
    _pad2:         f32,      //  4 bytes
}                            // Total: 128 bytes

// SVO coordinate transform (matches Rust `SvoTransformData`)
struct SvoTransform {
    origin:         vec3f,
    world_size:     f32,
    inv_world_size: f32,
    max_depth:      u32,
    _pad:           vec2u,
}

// ---------------------------------------------------------------------------
// Bindings (group 0)
//
// | Binding | Type                    | Content                        |
// |---------|-------------------------|--------------------------------|
// |    0    | storage<read>           | octree array<vec2u>            |
// |    1    | uniform                 | RayMarchUniforms               |
// |    2    | uniform                 | SvoTransform                   |
// |    3    | storage<read_write>     | depth_buffer array<f32>        |
// |    4    | storage<read_write>     | color_buffer array<f32>        |
// ---------------------------------------------------------------------------

@group(0) @binding(0) var<storage, read>       octree:       array<vec2u>;
@group(0) @binding(1) var<uniform>             uniforms:     RayMarchUniforms;
@group(0) @binding(2) var<uniform>             svo_tf:       SvoTransform;
// depth_buffer: one f32 per pixel — world-space ray parameter t at first hit
@group(0) @binding(3) var<storage, read_write> depth_buffer: array<f32>;
// color_buffer: four f32 per pixel (RGBA), linearised as pixel_idx * 4 + ch
@group(0) @binding(4) var<storage, read_write> color_buffer: array<f32>;

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
    aabb_min:  vec3f,
    aabb_size: f32,
    t_enter:   f32,
}

// ---------------------------------------------------------------------------
// SVO traversal
//
// Returns (hit, base_color, alpha, t_world, world_normal).
// t_world is the world-space ray parameter at the first opaque hit.
// ---------------------------------------------------------------------------

struct TraversalResult {
    hit:    bool,
    color:  vec3f,
    alpha:  f32,
    t_hit:  f32,
    normal: vec3f,
}

fn traverse_svo(
    ro_svo:     vec3f,  // ray origin in normalised [0,1]³ SVO space
    rd_svo:     vec3f,  // ray direction in SVO space (NOT renormalised — keeps scale)
    inv_rd:     vec3f,  // element-wise 1 / rd_svo
    t_scale:    f32,    // svo_t * t_scale = world-space t
    fw_coeff:   f32,    // pixel footprint coefficient: fw = fw_coeff * t_world
) -> TraversalResult {
    // Root AABB is [0,1]³ in SVO space.
    let root_hit = ray_aabb_slab(ro_svo, inv_rd, vec3f(0.0), vec3f(1.0));
    if root_hit.x >= root_hit.y || root_hit.y <= 0.0 {
        return TraversalResult(false, vec3f(0.0), 0.0, 1e30, vec3f(0.0, 1.0, 0.0));
    }

    // Fixed-size stack (depth-limited to MAX_STACK_DEPTH).
    var stack: array<StackEntry, 12>;
    var sp: u32 = 0u;

    stack[0] = StackEntry(0u, vec3f(0.0), 1.0, max(root_hit.x, 0.0));
    sp = 1u;

    var accum_color = vec3f(0.0);
    var accum_alpha = 0.0;
    var best_t      = 1e30;
    var best_normal = vec3f(0.0, 1.0, 0.0);

    while sp > 0u && accum_alpha < OPAQUE_THRESHOLD {
        sp -= 1u;
        let entry     = stack[sp];
        let node_idx  = entry.node_idx;
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

            // SDF evaluation for sub-voxel anti-aliasing.
            let half     = vec3f(n_size * 0.5);
            let center   = n_min + half;

            // Ray–AABB to find a good sample point inside the voxel.
            let aabb_hit = ray_aabb_slab(ro_svo, inv_rd, n_min, n_max);
            let t_mid    = clamp(
                (aabb_hit.x + aabb_hit.y) * 0.5,
                max(aabb_hit.x, 0.0),
                aabb_hit.y,
            );
            let hit_p    = ro_svo + rd_svo * t_mid;
            let p_local  = hit_p - center;
            let d        = sd_box(p_local, half);

            // FOV-correct pixel footprint at this distance.
            let t_world = t_mid * t_scale;
            let fw      = fw_coeff * max(t_world, 1e-4);
            let alpha   = 1.0 - smoothstep(-fw, fw, d);

            if alpha > 0.001 {
                // Analytical box normal: largest component of abs(p_local / half).
                let abs_p  = abs(p_local) / half;
                var normal: vec3f;
                if abs_p.x >= abs_p.y && abs_p.x >= abs_p.z {
                    normal = vec3f(sign(p_local.x), 0.0, 0.0);
                } else if abs_p.y >= abs_p.z {
                    normal = vec3f(0.0, sign(p_local.y), 0.0);
                } else {
                    normal = vec3f(0.0, 0.0, sign(p_local.z));
                }

                // Front-to-back alpha compositing.
                let contribution = alpha * (1.0 - accum_alpha);
                accum_color += unpack_base_color(cd) * contribution;
                accum_alpha += contribution;

                if t_world < best_t {
                    best_t      = t_world;
                    best_normal = normal;
                }
            }
            continue; // leaf processed
        }

        // Internal node: sort children front-to-back and push onto the stack.
        let child_size = n_size * 0.5;
        let fc         = svo_first_child(cp);

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
                stack[sp] = StackEntry(child_ni[i], child_min[i], child_size, child_t[i]);
                sp += 1u;
            }
        }
    }

    if accum_alpha < 0.001 {
        return TraversalResult(false, vec3f(0.0), 0.0, 1e30, vec3f(0.0, 1.0, 0.0));
    }
    return TraversalResult(true, accum_color, accum_alpha, best_t, best_normal);
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

    let result = traverse_svo(ro_svo, rd_svo, inv_rd, t_scale, fw_coeff);

    if !result.hit {
        color_buffer[color_base]     = BACKGROUND_R;
        color_buffer[color_base + 1u] = BACKGROUND_G;
        color_buffer[color_base + 2u] = BACKGROUND_B;
        color_buffer[color_base + 3u] = 1.0;
        depth_buffer[pixel_idx]      = 1e30;
        return;
    }

    // Simplified Lambertian + ambient lighting (Cook-Torrance is Phase 3a).
    let normal  = result.normal;
    let diffuse = max(dot(normal, normalize(uniforms.light_dir)), 0.0)
                  * uniforms.light_color * result.color;
    let ambient = result.color * 0.15;
    var lit     = diffuse + ambient;

    // Blend with background for semi-transparent (SDF fringe) pixels.
    let inv_a = 1.0 - result.alpha;
    lit = lit * result.alpha + vec3f(BACKGROUND_R, BACKGROUND_G, BACKGROUND_B) * inv_a;

    color_buffer[color_base]      = lit.r;
    color_buffer[color_base + 1u] = lit.g;
    color_buffer[color_base + 2u] = lit.b;
    color_buffer[color_base + 3u] = 1.0;
    depth_buffer[pixel_idx]       = result.t_hit;
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
