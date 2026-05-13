// UI Composite Shader — blends 2D UI panels over the voxel-splatted scene.
//
// Fullscreen triangle pass. For each pixel:
//   1. Sample the scene colour (from tiled rasteriser output)
//   2. Iterate UI panels and evaluate SDF rounded rectangles
//   3. Alpha-blend panel backgrounds (optionally frosted via scene mipmap)
//   4. Draw borders with anti-aliased SDF edge
//
// Panels with `frosted_glass == 1.0` sample the scene colour at a higher
// mip level to produce a blurred background effect, identical to the in-world
// frosted glass but applied in 2D screen space.

struct UiPanel {
    rect: vec4<f32>,          // x, y, w, h (normalised 0..1)
    bg_color: vec4<f32>,      // premultiplied alpha background
    border_color: vec4<f32>,  // border colour
    params: vec4<f32>,        // border_width, corner_radius, frosted, frost_blur
}

struct CompositeUniforms {
    viewport: vec2<f32>,      // viewport width, height in pixels
    panel_count: u32,
    _pad: u32,
}

@group(0) @binding(0) var scene_tex: texture_2d<f32>;
@group(0) @binding(1) var scene_sampler: sampler;
@group(0) @binding(2) var<uniform> uniforms: CompositeUniforms;
@group(0) @binding(3) var<storage, read> panels: array<UiPanel>;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

// Fullscreen triangle (3 vertices, no vertex buffer).
@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VertexOutput {
    var out: VertexOutput;
    let u = f32((vi << 1u) & 2u);
    let v = f32(vi & 2u);
    out.position = vec4<f32>(u * 2.0 - 1.0, v * -2.0 + 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(u, v);
    return out;
}

// Signed distance for a rounded box centred at the origin.
fn sdf_rounded_rect(p: vec2<f32>, half_size: vec2<f32>, radius: f32) -> f32 {
    let q = abs(p) - half_size + vec2<f32>(radius);
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - radius;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var color = textureSample(scene_tex, scene_sampler, in.uv);

    for (var i = 0u; i < uniforms.panel_count; i = i + 1u) {
        let panel = panels[i];
        let rect = panel.rect;          // x, y, w, h (normalised)
        let border_width = panel.params.x;
        let corner_radius = panel.params.y;
        let frosted = panel.params.z;
        let frost_blur = panel.params.w;

        // Convert pixel position to panel-local coordinates
        let pixel = in.uv;
        let center = rect.xy + rect.zw * 0.5;
        let half_size_ndc = rect.zw * 0.5;

        // Convert corner radius and border width from pixels to NDC
        let r_ndc = corner_radius / uniforms.viewport.x;
        let bw_ndc = border_width / uniforms.viewport.x;

        let p = pixel - center;
        let d = sdf_rounded_rect(p, half_size_ndc, r_ndc);

        // Outside the panel entirely — skip
        if d > bw_ndc {
            continue;
        }

        // Panel background (inside the border)
        if d < 0.0 {
            var bg = panel.bg_color;

            // Frosted glass: sample scene at higher mip for blur
            if frosted > 0.5 {
                let blurred = textureSampleLevel(scene_tex, scene_sampler, in.uv, frost_blur);
                bg = vec4<f32>(
                    mix(blurred.rgb, bg.rgb, bg.a),
                    max(bg.a, 0.5)
                );
            }

            // Anti-aliased edge
            let aa = 1.0 - smoothstep(-0.002, 0.0, d);
            let a = bg.a * aa;
            color = vec4<f32>(mix(color.rgb, bg.rgb, a), max(color.a, a));
        }

        // Border band
        if d >= -bw_ndc && d < bw_ndc {
            let edge = 1.0 - smoothstep(-0.001, 0.001, abs(d) - bw_ndc * 0.5);
            let bc = panel.border_color;
            let a = bc.a * edge;
            color = vec4<f32>(mix(color.rgb, bc.rgb, a), max(color.a, a));
        }
    }

    return color;
}
