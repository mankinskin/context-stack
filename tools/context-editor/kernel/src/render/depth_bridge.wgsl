// depth_bridge.wgsl — Phase 3a: bridge the r32float NDC depth buffer into a
// hardware Depth32Float attachment so downstream passes (wireframe overlay,
// particles, UI) can perform hardware depth testing against ray-marched voxel
// surfaces.
//
// Vertex:   fullscreen triangle (no VBO)
// Fragment: reads per-pixel NDC depth from the ray march storage buffer and
//           writes it to the hardware depth attachment via @builtin(frag_depth).
//           The SvoDepthTexture is then available for downstream depth testing.

struct DepthBridgeUniforms {
    screen_width: u32,
    _pad:         vec3u,
}

@group(0) @binding(0) var<storage, read> depth_buf:   array<f32>;
@group(0) @binding(1) var<uniform>       db_uniforms: DepthBridgeUniforms;

struct VtxOut {
    @builtin(position) clip: vec4f,
}

@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> VtxOut {
    // Fullscreen triangle covering clip space [-1,1]×[-1,1]
    let positions = array<vec2f, 3>(
        vec2f(-1.0, -1.0),
        vec2f( 3.0, -1.0),
        vec2f(-1.0,  3.0),
    );
    return VtxOut(vec4f(positions[vid], 0.0, 1.0));
}

@fragment
fn fs_main(@builtin(position) frag_coord: vec4f) -> @builtin(frag_depth) f32 {
    let px  = u32(frag_coord.x);
    let py  = u32(frag_coord.y);
    let idx = py * db_uniforms.screen_width + px;
    return depth_buf[idx];
}
