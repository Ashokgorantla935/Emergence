// Post-process shader: day/night color grading + lightning flash overlay.
// Reads rendered scene texture, applies tint/brightness, outputs to swapchain.

struct PostProcessUniform {
    tint_color:  vec3<f32>,
    brightness:  f32,
    flash_alpha: f32,
    _pad0:       f32,
    _pad1:       f32,
    _pad2:       f32,
};

@group(0) @binding(0) var scene_texture: texture_2d<f32>;
@group(0) @binding(1) var scene_sampler: sampler;
@group(0) @binding(2) var<uniform> pp: PostProcessUniform;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv:       vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = vec4<f32>(in.position, 0.0, 1.0);
    out.uv            = in.uv;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var color = textureSample(scene_texture, scene_sampler, in.uv);

    // Time-of-day color grade
    color = vec4<f32>(color.rgb * pp.tint_color * pp.brightness, color.a);

    // Vignette: darken edges to draw eye toward center
    let center = vec2<f32>(0.5, 0.5);
    let dist = distance(in.uv, center);
    let vignette = smoothstep(0.8, 0.3, dist); // 1.0 at center, 0.0 at corners
    color = vec4<f32>(color.rgb * mix(0.6, 1.0, vignette), color.a);

    // Lightning flash overlay (white)
    let flash_rgb = mix(color.rgb, vec3<f32>(1.0, 1.0, 0.95), pp.flash_alpha);
    return vec4<f32>(flash_rgb, color.a);
}
