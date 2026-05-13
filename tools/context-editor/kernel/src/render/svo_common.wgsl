// svo_common.wgsl — Shared WGSL helpers for SVO coordinate transforms and
// OctreeNode bit-decoding used by the SVO ray march shader (Phase 1a).
//
// The SVO root covers [origin, origin + world_size]³ in world space.
// Normalised SVO space maps that cube to [0,1]³.
//
// OctreeNode layout (2×u32, matches Rust `OctreeNode`):
//   node[i].x = child_pointer
//     bits  0– 7: child bitmask (which of 8 children exist)
//     bits  8–30: first-child index into the flat nodes array
//     bit  31:    INTERIOR_FLAG (leaf fully surrounded by solid voxels)
//   node[i].y = color_data
//     bits  0– 7: R
//     bits  8–15: G
//     bits 16–23: B
//     bits 24–28: roughness (5 bits, 0–31)
//     bit  29:    metallic flag
//     bits 30–31: SDF type (0=box, 1=sphere, 2=svo-sampled, 3=torus)

// ---------------------------------------------------------------------------
// SVO Transform uniform struct
// ---------------------------------------------------------------------------

struct SvoTransform {
    origin:         vec3f,
    world_size:     f32,
    inv_world_size: f32,
    max_depth:      u32,
    /// Depth at which the octree is split into paged subtrees (Phase 4a).
    /// Nodes at depth < page_depth are in the always-resident root page.
    /// Nodes at depth >= page_depth are in individually-paged leaf pages.
    page_depth:     u32,
    _pad:           u32,
}

// ---------------------------------------------------------------------------
// Coordinate helpers (pure functions — take transform as parameter)
// ---------------------------------------------------------------------------

/// Map a world-space point to normalised SVO space [0,1]³.
fn world_to_svo(p: vec3f, t: ptr<function, SvoTransform>) -> vec3f {
    return (p - (*t).origin) * (*t).inv_world_size;
}

/// Map a normalised SVO-space point back to world space.
fn svo_to_world(p: vec3f, t: ptr<function, SvoTransform>) -> vec3f {
    return (*t).origin + p * (*t).world_size;
}

// ---------------------------------------------------------------------------
// OctreeNode bit-decode helpers
// ---------------------------------------------------------------------------

/// Extract the 8-bit child bitmask from a raw `child_pointer` u32.
fn svo_child_mask(child_pointer: u32) -> u32 {
    return child_pointer & 0xFFu;
}

/// Extract the 23-bit first-child array index from `child_pointer`.
///
/// Bits 8–30 hold the first-child index.  Bit 31 is INTERIOR_FLAG and is
/// masked out to prevent interior-flagged nodes from producing wrong offsets.
fn svo_first_child_index(child_pointer: u32) -> u32 {
    return (child_pointer >> 8u) & 0x7FFFFFu;
}

/// Returns true when the node is a fully-interior leaf (surrounded on all 6
/// faces by solid voxels). The ray march shader skips SDF evaluation for
/// these because their surfaces are always occluded.
fn svo_is_interior(child_pointer: u32) -> bool {
    return (child_pointer & 0x80000000u) != 0u;
}

/// Extract the SDF type from `color_data` bits 30–31.
///   0 = box  1 = sphere  2 = svo-sampled  3 = torus/procedural
fn svo_sdf_type(color_data: u32) -> u32 {
    return (color_data >> 30u) & 0x3u;
}

/// Unpack the base RGB colour from `color_data` bits 0–23, returning [0,1].
fn svo_unpack_base_color(color_data: u32) -> vec3f {
    let r = f32(color_data        & 0xFFu) / 255.0;
    let g = f32((color_data >> 8u) & 0xFFu) / 255.0;
    let b = f32((color_data >> 16u) & 0xFFu) / 255.0;
    return vec3f(r, g, b);
}

/// Unpack roughness from `color_data` bits 24–28, scaled to [0,1].
fn svo_unpack_roughness(color_data: u32) -> f32 {
    return f32((color_data >> 24u) & 0x1Fu) / 31.0;
}

/// Unpack the metallic flag from `color_data` bit 29.
fn svo_unpack_metallic(color_data: u32) -> f32 {
    return f32((color_data >> 29u) & 0x1u);
}
