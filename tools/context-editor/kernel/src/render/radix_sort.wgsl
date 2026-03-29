// Radix Sort — 4-bit / 8-pass GPU radix sort (T6c).
//
// Three entry points per pass:
//   1. radix_histogram  — count digit occurrences per workgroup
//   2. radix_prefix_sum — single-workgroup exclusive scan of global histograms
//   3. radix_scatter    — stable write elements to sorted positions
//
// Histogram layout (digit-major):
//   histograms[digit * num_workgroups + workgroup_id] = count
//
// After prefix sum, the same array holds scatter offsets:
//   histograms[digit * num_workgroups + workgroup_id] = destination base index

struct RadixUniforms {
    bit_shift:           u32, // Current radix digit position (0, 4, 8, …, 28)
    num_elements:        u32, // Total elements to sort (max_splats)
    num_workgroups_sort: u32, // ceil(num_elements / 256)
    _pad:                u32,
}

@group(0) @binding(0) var<storage, read>        keys_src:   array<u32>;
@group(0) @binding(1) var<storage, read>        vals_src:   array<u32>;
@group(0) @binding(2) var<storage, read_write>  keys_dst:   array<u32>;
@group(0) @binding(3) var<storage, read_write>  vals_dst:   array<u32>;
@group(0) @binding(4) var<storage, read_write>  histograms: array<u32>;
@group(0) @binding(5) var<uniform>              uniforms:   RadixUniforms;

// ─────────────────────────────────────────────────────────────────────────────
// Histogram
// ─────────────────────────────────────────────────────────────────────────────

var<workgroup> local_hist: array<atomic<u32>, 16>;

@compute @workgroup_size(256)
fn radix_histogram(
    @builtin(global_invocation_id)  gid: vec3u,
    @builtin(workgroup_id)          wid: vec3u,
    @builtin(local_invocation_id)   lid: vec3u,
) {
    let idx = gid.x;
    let wg  = wid.x;
    let tid = lid.x;

    // Clear local histogram (threads 0-15)
    if tid < 16u {
        atomicStore(&local_hist[tid], 0u);
    }
    workgroupBarrier();

    // Each thread votes for its digit
    if idx < uniforms.num_elements {
        let digit = (keys_src[idx] >> uniforms.bit_shift) & 0xFu;
        atomicAdd(&local_hist[digit], 1u);
    }
    workgroupBarrier();

    // Threads 0-15 write to global histogram (digit-major layout)
    if tid < 16u {
        histograms[tid * uniforms.num_workgroups_sort + wg] =
            atomicLoad(&local_hist[tid]);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Prefix Sum  (single workgroup — handles up to 256 × 256 = 65 536 entries)
// ─────────────────────────────────────────────────────────────────────────────

var<workgroup> block_sums: array<u32, 256>;

@compute @workgroup_size(256)
fn radix_prefix_sum(@builtin(local_invocation_id) lid: vec3u) {
    let tid           = lid.x;
    let total_entries = 16u * uniforms.num_workgroups_sort;
    let chunk_size    = (total_entries + 255u) / 256u;
    let start         = tid * chunk_size;
    let end           = min(start + chunk_size, total_entries);

    // Phase 1 — sequential exclusive prefix sum within this thread's chunk
    var running = 0u;
    for (var i = start; i < end; i++) {
        let val      = histograms[i];
        histograms[i] = running;
        running      += val;
    }

    // Store thread's total into shared memory
    block_sums[tid] = running;
    workgroupBarrier();

    // Phase 2 — inclusive Hillis-Steele scan on block_sums[0..256]
    for (var stride = 1u; stride < 256u; stride <<= 1u) {
        var addend = 0u;
        if tid >= stride {
            addend = block_sums[tid - stride];
        }
        workgroupBarrier();
        block_sums[tid] += addend;
        workgroupBarrier();
    }

    // Convert inclusive → exclusive block offset
    var block_offset = 0u;
    if tid > 0u {
        block_offset = block_sums[tid - 1u];
    }

    // Phase 3 — propagate block offset to every element in this chunk
    for (var i = start; i < end; i++) {
        histograms[i] += block_offset;
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Scatter  (stable — per-element rank computed via shared-memory linear scan)
// ─────────────────────────────────────────────────────────────────────────────

var<workgroup> wg_digits: array<u32, 256>;

@compute @workgroup_size(256)
fn radix_scatter(
    @builtin(global_invocation_id)  gid: vec3u,
    @builtin(workgroup_id)          wid: vec3u,
    @builtin(local_invocation_id)   lid: vec3u,
) {
    let idx = gid.x;
    let wg  = wid.x;
    let tid = lid.x;

    // Load key and compute digit (sentinel for out-of-range threads)
    var digit = 15u;
    if idx < uniforms.num_elements {
        digit = (keys_src[idx] >> uniforms.bit_shift) & 0xFu;
    }

    wg_digits[tid] = digit;
    workgroupBarrier();

    // Compute stable local rank: count threads *before* me with the same digit
    var rank = 0u;
    for (var i = 0u; i < tid; i++) {
        if wg_digits[i] == digit {
            rank += 1u;
        }
    }

    if idx < uniforms.num_elements {
        let base = histograms[digit * uniforms.num_workgroups_sort + wg];
        let dest = base + rank;
        keys_dst[dest] = keys_src[idx];
        vals_dst[dest] = vals_src[idx];
    }
}
