// Voxel Splat Kernel — compute shader (T6a)
//
// Reads the SVO octree buffer and emits one VoxelSplat per occupied leaf node.
// LOD culling discards splats that project below ~1 pixel on screen.

struct OctreeNode {
    child_pointer: u32,  // lower 8 bits = child bitmask; upper 24 = first-child index
    color_data:    u32,  // packed R8 G8 B8 roughness8
}

struct VoxelSplat {
    center_ws:       vec3f,   // world-space voxel center
    half_extent:     f32,     // half-size of axis-aligned box
    material_packed: u32,     // passthrough from OctreeNode.color_data
    _pad:            u32,
}

struct SplatParams {
    camera_pos:  vec3f,
    total_nodes: u32,
    lod_scale:   f32,
    max_depth:   u32,
    world_size:  f32,
    _pad:        f32,
}

@group(0) @binding(0) var<storage, read>       octree:      array<OctreeNode>;
@group(0) @binding(1) var<storage, read_write>  splats:      array<VoxelSplat>;
@group(0) @binding(2) var<storage, read_write>  splat_count: atomic<u32>;
@group(0) @binding(3) var<uniform>              params:      SplatParams;
@group(0) @binding(4) var<storage, read>        node_positions: array<vec4f>;

// ---------------------------------------------------------------------------
// Main kernel
// ---------------------------------------------------------------------------

@compute @workgroup_size(256)
fn generate_splats(@builtin(global_invocation_id) id: vec3u) {
    let node_idx = id.x;

    if node_idx >= params.total_nodes {
        return;
    }

    let node = octree[node_idx];

    // Skip internal nodes — only leaves produce splats
    let child_mask = node.child_pointer & 0xFFu;
    if child_mask != 0u {
        return;
    }

    // Skip empty leaves (color_data == 0 means unoccupied)
    if node.color_data == 0u {
        return;
    }

    // Read precomputed position (xyz) and half_extent (w) from CPU-uploaded buffer
    let pos_data = node_positions[node_idx];
    let pos = pos_data.xyz;
    let half = pos_data.w;

    // LOD culling: skip splats that project below ~1 pixel
    let cam_dist = length(pos - params.camera_pos);
    let screen_size = half / max(cam_dist, 0.001);
    if screen_size < params.lod_scale {
        return;
    }

    let si = atomicAdd(&splat_count, 1u);
    splats[si] = VoxelSplat(pos, half, node.color_data, 0u);
}
