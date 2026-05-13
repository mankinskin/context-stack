// Motion-blurred particle injection compute shader.
//
// Reads ParticleSplat structs and emits VoxelSplat entries with velocity-based
// AABB stretch into the main splat buffer at the particle region offset.

struct ParticleSplat {
    position: vec3<f32>,
    scale: f32,
    velocity: vec3<f32>,
    opacity: f32,
    color: vec4<f32>,
}

struct VoxelSplat {
    center_ws: vec3<f32>,
    half_extent: f32,
    material_packed: u32,
    _pad: u32,
}

struct ParticleUniforms {
    particle_count: u32,
    motion_blur_scale: f32,
    _pad0: f32,
    _pad1: f32,
}

@group(0) @binding(0)
var<storage, read> particles: array<ParticleSplat>;

@group(0) @binding(1)
var<storage, read_write> splats: array<VoxelSplat>;

@group(0) @binding(2)
var<uniform> uniforms: ParticleUniforms;

/// Pack normalised RGB + opacity into a single u32.
/// Layout: [0:2 sdf=box][R:6][G:8][B:8][A:8]
///
/// Packs color into the compact material format expected by the tiled rasteriser:
///   bits  0- 7: R (8 bits)
///   bits  8-15: G (8 bits)
///   bits 16-23: B (8 bits)
///   bits 24-28: roughness (5 bits, 0-31)
///   bit     29: metallic (0 = dielectric)
///   bits 30-31: sdf_type (0 = box SDF for particle splats)
fn pack_color(c: vec4<f32>, opacity: f32) -> u32 {
    let r = u32(clamp(c.r, 0.0, 1.0) * 255.0);
    let g = u32(clamp(c.g, 0.0, 1.0) * 255.0);
    let b = u32(clamp(c.b, 0.0, 1.0) * 255.0);
    // Matte, non-metallic, box SDF (sdf_type=0 keeps bits 30-31 clear).
    let roughness: u32 = 9u; // ~0.3 roughness (matte)
    // sdf_type=1 (sphere) in bits 30-31 gives particle-like round appearance.
    return r | (g << 8u) | (b << 16u) | (roughness << 24u) | (1u << 30u);
}

/// Compute the motion-blurred AABB half-extent.
/// The splat is stretched along the velocity axis so faster particles look
/// elongated — a cheap screen-space motion blur approximation.
fn motion_blur_half_extent(base_scale: f32, velocity: vec3<f32>) -> f32 {
    let speed = length(velocity);
    let stretch = speed * uniforms.motion_blur_scale;
    return base_scale + stretch;
}

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    if idx >= uniforms.particle_count {
        return;
    }

    let p = particles[idx];

    // Skip fully transparent particles.
    if p.opacity <= 0.0 {
        return;
    }

    let he = motion_blur_half_extent(p.scale, p.velocity);
    let packed = pack_color(p.color, p.opacity);

    splats[idx] = VoxelSplat(
        p.position,
        he,
        packed,
        0u,
    );
}
