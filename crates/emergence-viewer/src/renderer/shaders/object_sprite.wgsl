// Object sprite shader — used by world objects (resources + structures).
// Simpler variant of being_sprite.wgsl: no skin-tone logic.
// One pipeline handles all objects via the shared atlas.

struct CameraUniform {
    view_proj: mat4x4<f32>,
    pixels_per_unit: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
};
@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(1) @binding(0) var sprite_atlas: texture_2d<f32>;
@group(1) @binding(1) var atlas_sampler: sampler;

struct VertexInput {
    @location(0) vertex_pos: vec2<f32>,
};

struct InstanceInput {
    @location(1) world_pos:   vec2<f32>,
    @location(2) atlas_uv:    vec2<f32>,
    @location(3) atlas_size:  vec2<f32>,
    @location(4) tint:        vec3<f32>,
    @location(5) size:        f32,
    @location(6) alpha:       f32,
    @location(7) _pad:        f32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv:    vec2<f32>,
    @location(1) tint:  vec3<f32>,
    @location(2) alpha: f32,
};

@vertex
fn vs_main(vertex: VertexInput, inst: InstanceInput) -> VertexOutput {
    var out: VertexOutput;
    let screen_size = max(inst.size * camera.pixels_per_unit, 6.0);
    let final_size  = screen_size / camera.pixels_per_unit;
    let world       = inst.world_pos + vertex.vertex_pos * final_size;
    out.clip_position = camera.view_proj * vec4<f32>(world, 0.0, 1.0);
    out.uv    = inst.atlas_uv + (vertex.vertex_pos + 0.5) * inst.atlas_size;
    out.tint  = inst.tint;
    out.alpha = inst.alpha;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let c = textureSample(sprite_atlas, atlas_sampler, in.uv);
    if c.a < 0.1 { discard; }
    return vec4<f32>(c.rgb * in.tint, c.a * in.alpha);
}
