// Sort Key Build — compute shader (T6b)
//
// Projects each VoxelSplat's AABB to screen-space, computes tile membership,
// and constructs composite sort keys (tile_id << 12 | depth) for the GPU
// radix sort (T6c).

struct VoxelSplat {
    center_ws:       vec3f,
    half_extent:     f32,
    material_packed: u32,
    _pad:            u32,
}

struct ProjectedSplat {
    screen_min:         vec2f,
    screen_max:         vec2f,
    center_and_extent:  vec4f,   // xyz = world center, w = half_extent
    depth:              f32,
    material_packed:    u32,
    _pad:               vec2u,
}

struct CameraUniforms {
    view_proj:   mat4x4f,
    view_mat:    mat4x4f,
    camera_pos:  vec3f,
    _pad0:       f32,
    resolution:  vec2f,
    max_depth:   f32,
    _pad1:       f32,
}

@group(0) @binding(0) var<storage, read>        splats:      array<VoxelSplat>;
@group(0) @binding(1) var<storage, read_write>  projected:   array<ProjectedSplat>;
@group(0) @binding(2) var<storage, read_write>  sort_keys:   array<u32>;
@group(0) @binding(3) var<storage, read_write>  sort_values: array<u32>;
@group(0) @binding(4) var<uniform>              camera:      CameraUniforms;
@group(0) @binding(5) var<storage, read>        splat_count_buf: array<u32>;

const TILE_SIZE: u32 = 16u;

@compute @workgroup_size(256)
fn build_sort_keys(@builtin(global_invocation_id) id: vec3u) {
    let idx = id.x;

    // Bounds check against buffer length (last workgroup may overshoot)
    if idx >= arrayLength(&sort_keys) {
        return;
    }

    // Default: sentinel key sorts to end; rasterizer ignores these entries.
    sort_keys[idx] = 0xFFFFFFFFu;
    sort_values[idx] = idx;

    let count = splat_count_buf[0];
    if idx >= count {
        return;
    }

    let s = splats[idx];

    // Transform the 8 corners of the voxel AABB to clip-space,
    // find the screen-space bounding rectangle.
    let half = vec3f(s.half_extent);
    let corners = array<vec3f, 8>(
        s.center_ws + vec3f(-1.0, -1.0, -1.0) * half,
        s.center_ws + vec3f( 1.0, -1.0, -1.0) * half,
        s.center_ws + vec3f(-1.0,  1.0, -1.0) * half,
        s.center_ws + vec3f( 1.0,  1.0, -1.0) * half,
        s.center_ws + vec3f(-1.0, -1.0,  1.0) * half,
        s.center_ws + vec3f( 1.0, -1.0,  1.0) * half,
        s.center_ws + vec3f(-1.0,  1.0,  1.0) * half,
        s.center_ws + vec3f( 1.0,  1.0,  1.0) * half,
    );

    var screen_min = vec2f(1e9);
    var screen_max = vec2f(-1e9);
    var all_behind = true;

    for (var c = 0u; c < 8u; c++) {
        let clip = camera.view_proj * vec4f(corners[c], 1.0);
        if clip.w <= 0.0 {
            continue;
        }
        all_behind = false;
        let ndc = clip.xyz / clip.w;
        let screen = (ndc.xy * vec2f(0.5, -0.5) + 0.5) * camera.resolution;
        screen_min = min(screen_min, screen);
        screen_max = max(screen_max, screen);
    }

    // Entirely behind camera — discard
    if all_behind {
        return;
    }

    // Frustum cull: AABB entirely off-screen
    if screen_max.x < 0.0 || screen_min.x > camera.resolution.x ||
       screen_max.y < 0.0 || screen_min.y > camera.resolution.y {
        return;
    }

    // Clamp to screen bounds
    screen_min = clamp(screen_min, vec2f(0.0), camera.resolution);
    screen_max = clamp(screen_max, vec2f(0.0), camera.resolution);

    // View-space depth of the voxel center
    let pos_view = camera.view_mat * vec4f(s.center_ws, 1.0);
    let view_depth = -pos_view.z; // Negate: view-space Z is negative for objects in front

    // Store projected splat (center + extent packed into vec4f)
    projected[idx] = ProjectedSplat(
        screen_min,
        screen_max,
        vec4f(s.center_ws, s.half_extent),
        view_depth,
        s.material_packed,
        vec2u(0u),
    );

    // Sort key: tile_id of center (20 bits) | depth (12 bits)
    let center_screen = (screen_min + screen_max) * 0.5;
    let tile_x = u32(center_screen.x) / TILE_SIZE;
    let tile_y = u32(center_screen.y) / TILE_SIZE;
    let grid_width = (u32(camera.resolution.x) + TILE_SIZE - 1u) / TILE_SIZE;
    let tile_id = tile_y * grid_width + tile_x;
    let depth_quantized = u32(clamp(view_depth / camera.max_depth * 4095.0, 0.0, 4095.0));

    sort_keys[idx] = (tile_id << 12u) | depth_quantized;
    sort_values[idx] = idx;
}
