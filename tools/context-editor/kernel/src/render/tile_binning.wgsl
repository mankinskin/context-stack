// Active-List Tile Binning — compute shader (T6d Phase 2)
//
// Three-pass tile binning using atomics and prefix-sum:
//   1. count_tile_overlaps — count how many sorted splats overlap each tile
//   2. prefix_sum_and_pack — exclusive scan → offsets, pack tile_data, init write heads
//   3. scatter_to_tiles   — write projected-buffer indices into the active_list
//
// The active_list replaces sorted_values for tile lookup in the rasteriser.
// Every entry genuinely overlaps its tile — the rasteriser only needs a
// per-pixel point-in-AABB check for sub-tile precision.

struct ProjectedSplat {
    screen_min:        vec2f,
    screen_max:        vec2f,
    center_and_extent: vec4f,
    depth:             f32,
    material_packed:   u32,
    _pad:              vec2u,
}

struct TileBinUniforms {
    num_elements: u32,  // max_splats — dispatch bound
    num_tiles:    u32,  // tiles_x * tiles_y
    grid_width:   u32,  // tiles_x
    max_active:   u32,  // active_list capacity
}

@group(0) @binding(0) var<storage, read>        sorted_values:    array<u32>;
@group(0) @binding(1) var<storage, read>        projected:        array<ProjectedSplat>;
@group(0) @binding(2) var<storage, read_write>  tile_counts:      array<atomic<u32>>;
@group(0) @binding(3) var<storage, read_write>  tile_write_heads: array<atomic<u32>>;
@group(0) @binding(4) var<storage, read_write>  tile_data:        array<u32>;
@group(0) @binding(5) var<storage, read_write>  active_list:      array<u32>;
@group(0) @binding(6) var<uniform>              uniforms:         TileBinUniforms;
@group(0) @binding(7) var<storage, read>        splat_count_buf:  array<u32>;

const TILE_SIZE: u32 = 16u;

// ---------------------------------------------------------------------------
// Pass 1: Count tile overlaps
// ---------------------------------------------------------------------------

@compute @workgroup_size(256)
fn count_tile_overlaps(@builtin(global_invocation_id) gid: vec3u) {
    let idx = gid.x;
    let count = splat_count_buf[0];
    if idx >= count { return; }

    let proj_idx = sorted_values[idx];
    let s = projected[proj_idx];

    let tx0 = u32(max(s.screen_min.x, 0.0)) / TILE_SIZE;
    let tx1 = u32(max(s.screen_max.x, 0.0)) / TILE_SIZE;
    let ty0 = u32(max(s.screen_min.y, 0.0)) / TILE_SIZE;
    let ty1 = u32(max(s.screen_max.y, 0.0)) / TILE_SIZE;

    for (var ty = ty0; ty <= ty1; ty++) {
        for (var tx = tx0; tx <= tx1; tx++) {
            let tile_idx = ty * uniforms.grid_width + tx;
            if tile_idx < uniforms.num_tiles {
                atomicAdd(&tile_counts[tile_idx], 1u);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Pass 2: Prefix-sum + pack tile_data + init write heads
// ---------------------------------------------------------------------------
//
// Single-thread sequential scan over tiles.  At ~10 K tiles this takes
// microseconds — a parallel scan would be overkill.

@compute @workgroup_size(1)
fn prefix_sum_and_pack() {
    var running_offset = 0u;
    for (var i = 0u; i < uniforms.num_tiles; i++) {
        let count = atomicLoad(&tile_counts[i]);

        // Initialize write head to this tile's start offset
        atomicStore(&tile_write_heads[i], running_offset);

        // Store offset and count as two separate u32s
        tile_data[i * 2u]      = running_offset;
        tile_data[i * 2u + 1u] = count;

        running_offset += count;

        // Prevent offset overflow
        if running_offset >= uniforms.max_active {
            // Remaining tiles get zero count
            for (var j = i + 1u; j < uniforms.num_tiles; j++) {
                tile_data[j * 2u]      = running_offset;
                tile_data[j * 2u + 1u] = 0u;
                atomicStore(&tile_write_heads[j], running_offset);
            }
            break;
        }
    }
}

// ---------------------------------------------------------------------------
// Pass 3: Scatter splat indices into active_list
// ---------------------------------------------------------------------------

@compute @workgroup_size(256)
fn scatter_to_tiles(@builtin(global_invocation_id) gid: vec3u) {
    let idx = gid.x;
    let count = splat_count_buf[0];
    if idx >= count { return; }

    let proj_idx = sorted_values[idx];
    let s = projected[proj_idx];

    let tx0 = u32(max(s.screen_min.x, 0.0)) / TILE_SIZE;
    let tx1 = u32(max(s.screen_max.x, 0.0)) / TILE_SIZE;
    let ty0 = u32(max(s.screen_min.y, 0.0)) / TILE_SIZE;
    let ty1 = u32(max(s.screen_max.y, 0.0)) / TILE_SIZE;

    for (var ty = ty0; ty <= ty1; ty++) {
        for (var tx = tx0; tx <= tx1; tx++) {
            let tile_idx = ty * uniforms.grid_width + tx;
            if tile_idx < uniforms.num_tiles {
                let slot = atomicAdd(&tile_write_heads[tile_idx], 1u);
                if slot < uniforms.max_active {
                    active_list[slot] = proj_idx;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Pass 4: Sort each tile's active_list range by depth (front-to-back)
// ---------------------------------------------------------------------------
//
// One thread per tile.  Insertion sort on the tile's active_list entries
// keyed by projected[].depth.  Tiles typically contain < 100 entries so
// this is fast and avoids the need for a parallel sort.

@compute @workgroup_size(256)
fn sort_tile_active_list(@builtin(global_invocation_id) gid: vec3u) {
    let tile_idx = gid.x;
    if tile_idx >= uniforms.num_tiles { return; }

    let offset = tile_data[tile_idx * 2u];
    let count  = tile_data[tile_idx * 2u + 1u];
    if count <= 1u { return; }

    // Insertion sort — stable, in-place, O(n²) but n is small.
    for (var i = 1u; i < count; i++) {
        let key_idx  = active_list[offset + i];
        let key_depth = projected[key_idx].depth;
        var j = i;
        while j > 0u {
            let prev_idx = active_list[offset + j - 1u];
            if projected[prev_idx].depth <= key_depth { break; }
            active_list[offset + j] = prev_idx;
            j -= 1u;
        }
        active_list[offset + j] = key_idx;
    }
}
