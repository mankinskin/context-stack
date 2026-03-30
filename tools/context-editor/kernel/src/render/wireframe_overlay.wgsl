// Wireframe overlay — draws SVO wireframe lines over the voxel splats.

struct WireframeUniforms {
    view_proj: mat4x4f,
    color:     vec4f,
}

@group(0) @binding(0) var<uniform> u: WireframeUniforms;

@vertex
fn vs_main(@location(0) position: vec3f) -> @builtin(position) vec4f {
    return u.view_proj * vec4f(position, 1.0);
}

@fragment
fn fs_main() -> @location(0) vec4f {
    return u.color;
}
