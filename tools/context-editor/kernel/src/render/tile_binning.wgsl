// Tile Binning — compute shader (T6d Phase 1)
//
// Scans sorted keys to find per-tile boundaries.  Each tile gets an offset
// (into the sorted array) and a count.  Sentinel keys (0xFFFFFFFF) are
// rejected since their tile_id exceeds any real tile.
//
// Buffer layout: tile_data is a flat array<atomic<u32>> with manual
// stride 2:  [offset_0, count_0, offset_1, count_1, …]

struct TileBinUniforms {
    num_elements: u32, // max_splats (dispatch covers all entries)
    num_tiles:    u32, // tiles_x * tiles_y — bounds check for tile_id
    _pad0:        u32,
    _pad1:        u32,
}

@group(0) @binding(0) var<storage, read>        sorted_keys: array<u32>;
@group(0) @binding(1) var<storage, read_write>  tile_data:   array<atomic<u32>>;
@group(0) @binding(2) var<uniform>              uniforms:    TileBinUniforms;

@compute @workgroup_size(256)
fn build_tiles(@builtin(global_invocation_id) gid: vec3u) {
    let idx = gid.x;
    if idx >= uniforms.num_elements {
        return;
    }

    let key = sorted_keys[idx];

    // Sentinel keys (0xFFFFFFFF) → skip
    if key == 0xFFFFFFFFu {
        return;
    }

    let tile_id = key >> 12u;

    // Out-of-bounds tile → skip (safety guard)
    if tile_id >= uniforms.num_tiles {
        return;
    }

    // Detect tile start: first element OR different tile from predecessor
    let prev_tile = select(
        sorted_keys[idx - 1u] >> 12u,
        0xFFFFFFFFu,
        idx == 0u,
    );
    if tile_id != prev_tile {
        atomicStore(&tile_data[tile_id * 2u], idx);
    }

    // Every valid element increments its tile's count
    atomicAdd(&tile_data[tile_id * 2u + 1u], 1u);
}
