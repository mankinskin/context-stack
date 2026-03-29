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

// ---------------------------------------------------------------------------
// Position reconstruction
// ---------------------------------------------------------------------------

// Reconstruct the 3D world-space position of a node by walking the octree
// from root to the node, accumulating the spatial offset at each level.
//
// Since our flat array stores 8 children per internal node, and each child's
// slot encodes which octant it occupies (bit 0=X, bit 1=Y, bit 2=Z), we can
// trace from any node back to the root by finding its parent chain.
//
// However, the flat array does not store parent pointers. Instead we do a
// *forward walk* from root, guided by a pre-computed path. For the kernel
// dispatch where every node is visited, a simpler approach is to encode the
// path bits during the linear scan:
//
// We walk down recursively from the root, but in a compute shader we use an
// iterative stack-free approach: for each node index, determine which octant
// it sits in at each level by comparing against the parent's first_child_index.

// For the initial implementation we use a brute-force approach: walk from root
// for each thread. This is O(max_depth) per thread which is fine for depths
// up to ~10.

fn trace_node_position(node_idx: u32, max_depth: u32, world_size: f32) -> vec3f {
    // Start from root (index 0) and walk towards node_idx.
    // At each level, determine which child slot leads to node_idx.
    var pos = vec3f(0.0);
    var half = world_size * 0.5;
    var current = 0u;  // root

    for (var depth = 0u; depth < max_depth; depth++) {
        let node = octree[current];
        let mask = node.child_pointer & 0xFFu;
        if mask == 0u {
            // Leaf reached — this is the node
            break;
        }
        let first_child = node.child_pointer >> 8u;

        // Check each of the 8 children to find which subtree contains node_idx
        var found = false;
        for (var slot = 0u; slot < 8u; slot++) {
            if (mask & (1u << slot)) == 0u {
                continue;  // slot empty
            }
            let child_idx = first_child + slot;
            if child_idx == node_idx || subtree_contains(child_idx, node_idx, max_depth - depth - 1u) {
                // This octant contains our target
                if (slot & 1u) != 0u { pos.x += half; }
                if (slot & 2u) != 0u { pos.y += half; }
                if (slot & 4u) != 0u { pos.z += half; }
                current = child_idx;
                found = true;
                break;
            }
        }
        if !found {
            break;  // node_idx not reachable (shouldn't happen for valid indices)
        }
        half *= 0.5;
    }

    // Center of the voxel cell
    return pos + vec3f(half);
}

// Check if `target` is within the subtree rooted at `root_idx` by walking
// down at most `remaining_depth` levels. This uses the observation that
// children of `root_idx` occupy indices [first_child, first_child+7], and
// their children are similarly structured.
fn subtree_contains(root_idx: u32, target: u32, remaining_depth: u32) -> bool {
    if root_idx == target {
        return true;
    }
    if remaining_depth == 0u {
        return false;
    }
    let node = octree[root_idx];
    let mask = node.child_pointer & 0xFFu;
    if mask == 0u {
        return false;  // leaf, no children
    }
    let first_child = node.child_pointer >> 8u;
    for (var slot = 0u; slot < 8u; slot++) {
        if (mask & (1u << slot)) == 0u {
            continue;
        }
        let child_idx = first_child + slot;
        if subtree_contains(child_idx, target, remaining_depth - 1u) {
            return true;
        }
    }
    return false;
}

fn voxel_half_extent(max_depth: u32, world_size: f32) -> f32 {
    return world_size / f32(1u << max_depth) * 0.5;
}

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

    let half = voxel_half_extent(params.max_depth, params.world_size);
    let pos = trace_node_position(node_idx, params.max_depth, params.world_size);

    // LOD culling: skip splats that project below ~1 pixel
    let cam_dist = length(pos - params.camera_pos);
    let screen_size = half / max(cam_dist, 0.001);
    if screen_size < params.lod_scale {
        return;
    }

    let si = atomicAdd(&splat_count, 1u);
    splats[si] = VoxelSplat(pos, half, node.color_data, 0u);
}
